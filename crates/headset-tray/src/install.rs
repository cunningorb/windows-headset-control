//! Self-installation.
//!
//! The binary installs itself rather than shipping a separate installer: the
//! phase's footprint rule is zero new dependencies, and an MSI would mean a WiX
//! build toolchain. `--install` copies the running executable into
//! `%LOCALAPPDATA%\Programs\HeadsetTray`, enables logon startup, and registers
//! an Add/Remove Programs entry so it uninstalls like any other application.
//!
//! Per-user throughout. Nothing here writes to `HKEY_LOCAL_MACHINE`, installs a
//! service or driver, or requires elevation — consistent with the `asInvoker`
//! row in `docs/threat-model.md`.

use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

use crate::settings::{self, APP_NAME};

const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\HeadsetTray";
const DISPLAY_NAME: &str = "Headset Tray";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

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
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::NoLocalAppData => write!(f, "LOCALAPPDATA is not set"),
            InstallError::Io(e) => write!(f, "{e}"),
            InstallError::Registry(what) => write!(f, "could not write {what} to the registry"),
        }
    }
}

/// Copies this executable into the install directory, enables startup, and
/// registers for Add/Remove Programs.
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
    register_uninstall(&target)?;
    Ok(target)
}

/// Removes the startup entry, the Add/Remove Programs entry, the running tray,
/// and the install directory.
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
    let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wide(UNINSTALL_KEY).as_ptr())) };
    // Already absent is success: uninstalling twice must not be an error.
    if status != ERROR_SUCCESS {
        tracing::debug!("uninstall key absent or not removable");
    }

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

fn set_uninstall_string(key: HKEY, name: &str, data: &str) -> Result<(), InstallError> {
    let w = wide(data);
    let bytes = unsafe { std::slice::from_raw_parts(w.as_ptr() as *const u8, w.len() * 2) };
    let status =
        unsafe { RegSetValueExW(key, PCWSTR(wide(name).as_ptr()), 0, REG_SZ, Some(bytes)) };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(InstallError::Registry("an uninstall entry value"))
    }
}

fn register_uninstall(exe: &std::path::Path) -> Result<(), InstallError> {
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide(UNINSTALL_KEY).as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_WRITE,
            None,
            &mut key,
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(InstallError::Registry("the uninstall key"));
    }
    let result = (|| {
        set_uninstall_string(key, "DisplayName", DISPLAY_NAME)?;
        set_uninstall_string(key, "DisplayVersion", env!("CARGO_PKG_VERSION"))?;
        set_uninstall_string(key, "Publisher", "windows-headset-control")?;
        set_uninstall_string(
            key,
            "UninstallString",
            &format!("\"{}\" --uninstall", exe.display()),
        )?;
        set_uninstall_string(key, "DisplayIcon", &exe.display().to_string())?;
        set_uninstall_string(
            key,
            "InstallLocation",
            &exe.parent().unwrap_or(exe).display().to_string(),
        )?;
        Ok(())
    })();
    unsafe {
        let _ = RegCloseKey(key);
    }
    result
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
    fn the_uninstall_key_is_per_user() {
        // HKLM would need elevation and would leave an entry other users see.
        assert!(UNINSTALL_KEY.starts_with(r"Software\Microsoft\Windows"));
        assert!(!UNINSTALL_KEY.to_lowercase().contains("wow6432"));
    }
}
