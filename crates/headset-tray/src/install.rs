//! Self-installation.
//!
//! `--install` copies the running executable into
//! `%LOCALAPPDATA%\Programs\HeadsetTray`, enables logon startup, and creates a
//! Start menu shortcut. It is a developer shortcut for putting a local build in
//! place; the Inno setup executable is what users run, and it alone owns the
//! Add/Remove Programs entry.
//!
//! Per-user throughout. Nothing here writes to `HKEY_LOCAL_MACHINE`, installs a
//! service or driver, or requires elevation — consistent with the `asInvoker`
//! row in `docs/threat-model.md`.

use std::path::PathBuf;

use windows::core::PCWSTR;

use crate::settings::{self, APP_NAME};

/// `%LOCALAPPDATA%\Programs\HeadsetTray`.
pub fn install_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("Programs").join(APP_NAME))
}

pub fn installed_exe() -> Option<PathBuf> {
    Some(install_dir()?.join("headset-tray.exe"))
}

/// Whether the currently running image is the installed copy.
pub fn running_from_install_dir() -> bool {
    match (std::env::current_exe().ok(), installed_exe()) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

#[derive(Debug)]
pub enum InstallError {
    NoLocalAppData,
    Io(std::io::Error),
    Registry(&'static str),
    Shortcut(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::NoLocalAppData => write!(f, "LOCALAPPDATA is not set"),
            InstallError::Io(e) => write!(f, "{e}"),
            InstallError::Registry(what) => write!(f, "could not write {what} to the registry"),
            InstallError::Shortcut(e) => write!(f, "could not create the Start menu shortcut: {e}"),
        }
    }
}

/// Where the Start menu entry goes. Per-user: the all-users Start menu needs
/// administrator rights, which this project never asks for.
pub fn start_menu_shortcut() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Headset Tray.lnk"),
    )
}

/// Creates the Start menu shortcut pointing at `exe`.
///
/// No icon is specified: the shortcut inherits the executable's own icon
/// resource, so there is one icon to keep current rather than two.
fn create_shortcut(exe: &std::path::Path) -> Result<(), InstallError> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    let Some(dest) = start_menu_shortcut() else {
        return Ok(());
    };
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    unsafe {
        // --install runs before any window exists, so this thread has no
        // apartment yet. Harmless if one is already initialised.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
        link.SetPath(&HSTRING::from(exe.as_os_str()))
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
        link.SetDescription(&HSTRING::from("Headset settings in the notification area"))
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
        if let Some(dir) = exe.parent() {
            let _ = link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()));
        }
        let file: IPersistFile = link
            .cast()
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
        file.Save(&HSTRING::from(dest.as_os_str()), true)
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
    }
    Ok(())
}

/// Copies this executable into the install directory, enables startup, and adds
/// a Start menu shortcut.
///
/// A developer shortcut for putting a local build in place. It deliberately
/// does **not** register in Add/Remove Programs: that entry belongs to the Inno
/// installer, and two things writing the same key is how a stale entry pointing
/// at a deleted file happens.
///
/// Returns the installed path. Safe to run again over an existing install: it
/// is how an upgrade is performed.
pub fn install() -> Result<PathBuf, InstallError> {
    let dir = install_dir().ok_or(InstallError::NoLocalAppData)?;
    let target = dir.join("headset-tray.exe");
    let source = std::env::current_exe().map_err(InstallError::Io)?;

    std::fs::create_dir_all(&dir).map_err(InstallError::Io)?;

    // Re-running --install from the installed copy would otherwise be a
    // copy-onto-itself, which fails on Windows.
    if source != target {
        // A running instance holds its image locked, so a plain copy over a
        // live install fails. Move the old one aside first; Windows permits
        // renaming a running executable even though it forbids overwriting it.
        if target.exists() {
            let stale = dir.join("headset-tray.exe.old");
            let _ = std::fs::remove_file(&stale);
            std::fs::rename(&target, &stale).map_err(InstallError::Io)?;
        }
        std::fs::copy(&source, &target).map_err(InstallError::Io)?;
    }

    if !settings::set_run_on_startup(true, &target) {
        return Err(InstallError::Registry("the startup entry"));
    }
    create_shortcut(&target)?;
    // Deliberately no Add/Remove Programs registration. That entry belongs to
    // the Inno installer, which has its own uninstaller; two things writing the
    // same key is how a stale entry pointing at a deleted file happens. See
    // docs/history/specs/2026-08-02-installer-and-icon-design.md.
    Ok(target)
}

/// Removes the startup entry, the Start menu shortcut, the running tray, and
/// the install directory — exactly what `install()` creates, and nothing else.
///
/// Windows will not let a process delete its own image, and `--uninstall` is
/// normally run *from* the installed copy. Rather than leave the folder behind
/// and tell the user to clear it up, this asks any running tray to exit and
/// hands the directory to a detached helper that waits for this process to go
/// and then removes it.
pub fn uninstall() -> Result<Option<PathBuf>, InstallError> {
    let dir = install_dir();
    if !settings::set_run_on_startup(false, &PathBuf::new()) {
        return Err(InstallError::Registry("the startup entry"));
    }
    if let Some(lnk) = start_menu_shortcut() {
        let _ = std::fs::remove_file(lnk);
    }
    // The Add/Remove Programs entry is deliberately NOT removed here. It belongs
    // to the Inno installer, which has its own uninstaller; deleting it would
    // strand an installation this path did not create. `--uninstall` is the
    // inverse of `--install`, not a general uninstaller — an installation made
    // by the setup executable is removed through Settings > Installed apps.

    close_running_tray();
    if let Some(d) = dir.as_ref() {
        schedule_directory_removal(d);
    }
    Ok(dir)
}

/// Asks a running tray to shut down, so it stops holding its own image open.
///
/// Posts to the window rather than terminating the process: the tray's
/// `WM_DESTROY` handler removes its notification icon, and killing it outright
/// leaves a ghost icon in the tray until the user hovers over it.
fn close_running_tray() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_CLOSE};
    unsafe {
        let class: Vec<u16> = "HeadsetTrayWindow"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let hwnd = FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null());
        if let Ok(h) = hwnd {
            if !h.is_invalid() {
                let _ = PostMessageW(h, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

/// Spawns a detached helper that deletes `dir` once this process has exited.
///
/// Retries, because the exact moment our image is released is not ours to know:
/// the tray may still be shutting down. Bounded so a directory that genuinely
/// cannot be removed leaves a stopped script rather than a spinning one.
fn schedule_directory_removal(dir: &std::path::Path) {
    let script = std::env::temp_dir().join("headset-tray-cleanup.cmd");
    let body = format!(
        "@echo off\r\n\
         set n=0\r\n\
         :retry\r\n\
         set /a n+=1\r\n\
         ping -n 2 127.0.0.1 >nul\r\n\
         rd /s /q \"{dir}\" 2>nul\r\n\
         if not exist \"{dir}\" goto done\r\n\
         if %n% lss 15 goto retry\r\n\
         :done\r\n\
         del \"%~f0\"\r\n",
        dir = dir.display()
    );
    if std::fs::write(&script, body).is_err() {
        return;
    }
    // Detached and windowless: the user asked to uninstall, not to watch a
    // console flash past.
    let _ = std::process::Command::new("cmd.exe")
        .arg("/c")
        .arg(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Cleans up the `.old` image a previous upgrade left behind.
///
/// Called on normal startup, by which point the previous image is no longer
/// running and can finally be deleted.
pub fn tidy_previous_upgrade() {
    if let Some(dir) = install_dir() {
        let stale = dir.join("headset-tray.exe.old");
        if stale.exists() {
            let _ = std::fs::remove_file(stale);
        }
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;

    #[test]
    fn the_shortcut_goes_in_the_per_user_start_menu() {
        let p = start_menu_shortcut().expect("APPDATA is set on Windows");
        let s = p.to_string_lossy().to_lowercase();
        assert!(s.ends_with("headset tray.lnk"), "{}", p.display());
        assert!(
            s.contains(r"\microsoft\windows\start menu\programs"),
            "{}",
            p.display()
        );
        // Per-user throughout: nothing goes in the all-users Start menu, which
        // would need administrator rights this project does not ask for.
        assert!(!s.contains("programdata"), "{}", p.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_dir_is_per_user_and_never_program_files() {
        let Some(dir) = install_dir() else { return };
        let s = dir.display().to_string();
        assert!(
            !s.to_lowercase().contains("program files"),
            "installing to Program Files would require elevation: {s}"
        );
        assert!(s.ends_with(APP_NAME), "{s}");
    }

    #[test]
    fn installed_exe_sits_inside_the_install_dir() {
        let (Some(dir), Some(exe)) = (install_dir(), installed_exe()) else {
            return;
        };
        assert_eq!(exe.parent(), Some(dir.as_path()));
    }

    #[test]
    fn nothing_here_registers_in_add_remove_programs() {
        // That entry belongs to the Inno installer. This path used to write it
        // too, which is how an installation removed by one tool could leave the
        // other's entry behind, pointing at a deleted executable.
        let src = include_str!("install.rs");
        assert!(
            !src.contains(r"CurrentVersion\Uninstall"),
            "install.rs writes an Add/Remove Programs key again"
        );
    }
}
