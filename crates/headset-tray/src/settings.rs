//! Persisted settings, stored in the registry under `HKEY_CURRENT_USER`.
//!
//! Two settings, deliberately stored in two different places:
//!
//! - **Run on Windows startup** lives in `...\CurrentVersion\Run`, where Windows
//!   itself reads it. There is no separate copy of this setting, because a copy
//!   could disagree with reality — disabling the entry from Task Manager's
//!   Startup tab would leave a config file still claiming it was on. Reading the
//!   same value Windows reads means the menu cannot lie.
//! - **Show the Synapse warning** is ours alone, so it lives under
//!   `HKCU\Software\HeadsetTray`.
//!
//! Registry rather than a config file only to avoid inventing a file format and
//! taking a parser dependency for two booleans.

use std::path::{Path, PathBuf};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_SUCCESS, MAX_PATH};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
    RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};

/// Where Windows looks for per-user startup programs.
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// The value name under `RUN_KEY`. Also the app's identity in Task Manager.
pub const APP_NAME: &str = "HeadsetTray";
/// Our own settings key.
const APP_KEY: &str = r"Software\HeadsetTray";
const SYNAPSE_WARNING_VALUE: &str = "ShowSynapseWarning";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Reads a string value, or `None` if absent.
fn read_string(root: HKEY, subkey: &str, value: &str) -> Option<String> {
    let mut buf = [0u16; MAX_PATH as usize * 2];
    let mut len = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(wide(subkey).as_ptr()),
            PCWSTR(wide(value).as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
            Some(&mut len),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let chars = (len as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..chars]))
}

fn read_dword(root: HKEY, subkey: &str, value: &str) -> Option<u32> {
    let mut out: u32 = 0;
    let mut len = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(wide(subkey).as_ptr()),
            PCWSTR(wide(value).as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut out as *mut u32 as *mut std::ffi::c_void),
            Some(&mut len),
        )
    };
    (status == ERROR_SUCCESS).then_some(out)
}

/// Opens (creating if needed) a writable subkey of HKCU.
fn open_write(subkey: &str) -> Option<HKEY> {
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide(subkey).as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_WRITE,
            None,
            &mut key,
            None,
        )
    };
    (status == ERROR_SUCCESS).then_some(key)
}

fn set_string(subkey: &str, value: &str, data: &str) -> bool {
    let Some(key) = open_write(subkey) else {
        return false;
    };
    let wide_data = wide(data);
    let bytes =
        unsafe { std::slice::from_raw_parts(wide_data.as_ptr() as *const u8, wide_data.len() * 2) };
    let status =
        unsafe { RegSetValueExW(key, PCWSTR(wide(value).as_ptr()), 0, REG_SZ, Some(bytes)) };
    unsafe {
        let _ = RegCloseKey(key);
    }
    status == ERROR_SUCCESS
}

fn set_dword(subkey: &str, value: &str, data: u32) -> bool {
    let Some(key) = open_write(subkey) else {
        return false;
    };
    let bytes = data.to_ne_bytes();
    let status = unsafe {
        RegSetValueExW(
            key,
            PCWSTR(wide(value).as_ptr()),
            0,
            REG_DWORD,
            Some(&bytes),
        )
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    status == ERROR_SUCCESS
}

fn delete_value(subkey: &str, value: &str) -> bool {
    let mut key = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide(subkey).as_ptr()),
            0,
            KEY_WRITE,
            &mut key,
        )
    };
    if status != ERROR_SUCCESS {
        return false;
    }
    let status = unsafe { RegDeleteValueW(key, PCWSTR(wide(value).as_ptr())) };
    unsafe {
        let _ = RegCloseKey(key);
    }
    status == ERROR_SUCCESS
}

/// The command line Windows should run at logon, for a given executable.
///
/// Quoted because the install path contains spaces on most machines
/// (`C:\Users\Some Name\AppData\...`), and an unquoted `Run` value would be
/// parsed as a different program plus arguments.
fn run_command(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Whether the app is registered to run at logon.
///
/// Reads the same value Windows reads, so it cannot disagree with what actually
/// happens at logon.
pub fn run_on_startup() -> bool {
    read_string(HKEY_CURRENT_USER, RUN_KEY, APP_NAME).is_some()
}

/// Whether the registered startup command points at this specific executable.
///
/// Distinguishes "startup is on" from "startup is on, but pointing at a copy
/// somewhere else" — which is what an uninstalled-then-rebuilt binary looks
/// like, and is worth not silently ignoring.
pub fn startup_target() -> Option<PathBuf> {
    let raw = read_string(HKEY_CURRENT_USER, RUN_KEY, APP_NAME)?;
    let trimmed = raw.trim().trim_matches('"');
    Some(PathBuf::from(trimmed))
}

/// Registers or unregisters the app for logon.
pub fn set_run_on_startup(enabled: bool, exe: &Path) -> bool {
    if enabled {
        set_string(RUN_KEY, APP_NAME, &run_command(exe))
    } else {
        // Absent is the "off" state, so a missing value is success, not failure.
        delete_value(RUN_KEY, APP_NAME) || !run_on_startup()
    }
}

/// Whether to show the "Synapse is running" warning. Defaults to on: a user who
/// has never touched the setting is better served by knowing something else is
/// changing their settings underneath them.
pub fn show_synapse_warning() -> bool {
    read_dword(HKEY_CURRENT_USER, APP_KEY, SYNAPSE_WARNING_VALUE) != Some(0)
}

pub fn set_show_synapse_warning(enabled: bool) -> bool {
    set_dword(APP_KEY, SYNAPSE_WARNING_VALUE, u32::from(enabled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_startup_command_is_quoted() {
        // %LOCALAPPDATA% contains a space on most real machines. An unquoted
        // Run value would be read as a different program plus arguments.
        let p = PathBuf::from(r"C:\Users\Some Name\AppData\Local\Programs\HeadsetTray\x.exe");
        let cmd = run_command(&p);
        assert!(cmd.starts_with('"') && cmd.ends_with('"'), "{cmd}");
        assert!(cmd.contains("Some Name"));
    }

    #[test]
    fn an_absent_setting_is_not_read_as_disabled() {
        // The default has to be the informative one: someone who has never
        // touched the setting should still be told when Synapse is fighting
        // them. `show_synapse_warning` expresses that as `!= Some(0)`, so this
        // pins the property that makes it work -- an absent value reads as
        // None, and None is not Some(0).
        let absent = read_dword(
            HKEY_CURRENT_USER,
            r"Software\HeadsetTray\NoSuchKey",
            "NoSuchValue",
        );
        assert_eq!(absent, None);
        assert!(absent != Some(0), "absent must not mean disabled");
    }

    /// Round-trips against the real registry under a scratch key, then cleans up.
    /// Uses HKCU only, so it needs no elevation and touches nothing shared.
    #[test]
    fn dword_round_trips_through_the_registry() {
        const SCRATCH: &str = r"Software\HeadsetTray\TestScratch";
        assert!(set_dword(SCRATCH, "Flag", 1));
        assert_eq!(read_dword(HKEY_CURRENT_USER, SCRATCH, "Flag"), Some(1));
        assert!(set_dword(SCRATCH, "Flag", 0));
        assert_eq!(read_dword(HKEY_CURRENT_USER, SCRATCH, "Flag"), Some(0));
        assert!(delete_value(SCRATCH, "Flag"));
        assert_eq!(read_dword(HKEY_CURRENT_USER, SCRATCH, "Flag"), None);
    }

    #[test]
    fn string_round_trips_through_the_registry() {
        const SCRATCH: &str = r"Software\HeadsetTray\TestScratch";
        let path = r#""C:\Program Files\With Spaces\app.exe""#;
        assert!(set_string(SCRATCH, "Cmd", path));
        assert_eq!(
            read_string(HKEY_CURRENT_USER, SCRATCH, "Cmd").as_deref(),
            Some(path)
        );
        assert!(delete_value(SCRATCH, "Cmd"));
        assert_eq!(read_string(HKEY_CURRENT_USER, SCRATCH, "Cmd"), None);
    }
}
