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

use crate::output::{Problem, Slot};

/// Where Windows looks for per-user startup programs.
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// The value name under `RUN_KEY`. Also the app's identity in Task Manager.
pub const APP_NAME: &str = "HeadsetTray";
/// Our own settings key.
const APP_KEY: &str = r"Software\HeadsetTray";
const SYNAPSE_WARNING_VALUE: &str = "ShowSynapseWarning";
const APPEARANCE_VALUE: &str = "Appearance";
const OUTPUT_SWITCH_VALUE: &str = "SwitchOutputWhenOff";
const SPLIT_VALUE: &str = "SplitGameAndChat";
const RESTORE_OUTPUT_VALUE: &str = "RestoreOutputId";
const OUTPUT_ISSUE_VALUE: &str = "LastOutputIssue";

/// Where Windows keeps the user's light-or-dark preference for applications.
const PERSONALIZE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

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
/// The user's appearance choice.
///
/// Absent means follow Windows, which is what a fresh install gets. An
/// unrecognised value is treated the same way rather than being an error: a
/// hand-edited registry should not stop the tray drawing.
pub fn appearance() -> crate::ui::theme::Appearance {
    use crate::ui::theme::Appearance;
    match read_string(HKEY_CURRENT_USER, APP_KEY, APPEARANCE_VALUE).as_deref() {
        Some("light") => Appearance::Light,
        Some("dark") => Appearance::Dark,
        _ => Appearance::System,
    }
}

pub fn set_appearance(a: crate::ui::theme::Appearance) -> bool {
    use crate::ui::theme::Appearance;
    set_string(
        APP_KEY,
        APPEARANCE_VALUE,
        match a {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        },
    )
}

/// Whether Windows is set to light for applications.
///
/// Absent is dark, which is what Windows itself does with this value.
pub fn windows_prefers_light() -> bool {
    read_dword(HKEY_CURRENT_USER, PERSONALIZE_KEY, "AppsUseLightTheme") == Some(1)
}

pub fn show_synapse_warning() -> bool {
    read_dword(HKEY_CURRENT_USER, APP_KEY, SYNAPSE_WARNING_VALUE) != Some(0)
}

pub fn set_show_synapse_warning(enabled: bool) -> bool {
    set_dword(APP_KEY, SYNAPSE_WARNING_VALUE, u32::from(enabled))
}

/// Whether to move Windows' sound elsewhere while the headset is powered down.
///
/// Defaults to **off**, unlike the Synapse warning. Changing which device the
/// whole machine plays through is not something to start doing to somebody who
/// never asked for it.
pub fn switch_output_when_off() -> bool {
    read_dword(HKEY_CURRENT_USER, APP_KEY, OUTPUT_SWITCH_VALUE) == Some(1)
}

pub fn set_switch_output_when_off(enabled: bool) -> bool {
    set_dword(APP_KEY, OUTPUT_SWITCH_VALUE, u32::from(enabled))
}

/// Whether to point the headset's two channels at different Windows roles when
/// it comes back on: the game channel at ordinary playback, the chat channel at
/// calls.
///
/// Defaults to **off**, for the same reason the switch above it does. Moving
/// somebody's calls to a different endpoint than their music is a choice, not a
/// correction, and plenty of people use one channel for everything.
pub fn split_game_and_chat() -> bool {
    read_dword(HKEY_CURRENT_USER, APP_KEY, SPLIT_VALUE) == Some(1)
}

pub fn set_split_game_and_chat(enabled: bool) -> bool {
    set_dword(APP_KEY, SPLIT_VALUE, u32::from(enabled))
}

/// The registry value names holding one slot's `(endpoint id, name)`.
///
/// The fallback's two keep their original names so an existing installation
/// does not silently lose the device it was already switching to.
fn slot_values(slot: Slot) -> (&'static str, &'static str) {
    match slot {
        Slot::Fallback => ("FallbackOutputId", "FallbackOutputName"),
        Slot::Game => ("GameOutputId", "GameOutputName"),
        Slot::Chat => ("ChatOutputId", "ChatOutputName"),
    }
}

/// A chosen device, as `(endpoint id, name at the time it was chosen)`.
///
/// Both are stored because they answer different questions. The id is what the
/// switch acts on and is stable across reboots and renames; the name is only so
/// the settings panel can say what was chosen without enumerating devices, and
/// can still name it when that device is currently absent.
pub fn output_choice(slot: Slot) -> Option<(String, String)> {
    let (id_value, name_value) = slot_values(slot);
    let id = read_string(HKEY_CURRENT_USER, APP_KEY, id_value)?;
    if id.is_empty() {
        return None;
    }
    let name = read_string(HKEY_CURRENT_USER, APP_KEY, name_value).unwrap_or_else(|| id.clone());
    Some((id, name))
}

pub fn set_output_choice(slot: Slot, id: &str, name: &str) -> bool {
    let (id_value, name_value) = slot_values(slot);
    set_string(APP_KEY, id_value, id) && set_string(APP_KEY, name_value, name)
}

/// The endpoint to put back when the headset returns, or `None` when nothing
/// has been switched away from.
///
/// Persisted rather than held in memory because presence *is* the state: this
/// value existing means "we moved the sound and owe the user a move back". A
/// tray that is closed, updated, or killed while the headset is off would
/// otherwise forget its debt and strand the sound on the speakers.
pub fn restore_output() -> Option<String> {
    read_string(HKEY_CURRENT_USER, APP_KEY, RESTORE_OUTPUT_VALUE).filter(|s| !s.is_empty())
}

/// Records what to restore, or clears the record once it has been restored.
pub fn set_restore_output(id: Option<&str>) -> bool {
    match id {
        Some(id) => set_string(APP_KEY, RESTORE_OUTPUT_VALUE, id),
        // Absent is the "nothing owed" state, so a missing value is success.
        None => delete_value(APP_KEY, RESTORE_OUTPUT_VALUE) || restore_output().is_none(),
    }
}

/// The last thing that stopped the output switch or the split working, if
/// anything.
///
/// Persisted for the same reason the restore record is: the tray is not
/// running when the user reads it. Somebody who powers their headset off,
/// hears nothing move, and opens the panel an hour later needs the reason to
/// still be there. Cleared the moment a switch succeeds, so a stale message
/// cannot outlive the condition it describes.
///
/// Stored as [`Problem::key`] rather than as the sentence shown to the user, so
/// the panel can tell which of the two settings rows the complaint belongs on
/// — and so re-wording a message does not move it. Anything unrecognised, which
/// is what an older build's record looks like, reads as no complaint.
pub fn output_problem() -> Option<Problem> {
    let key = read_string(HKEY_CURRENT_USER, APP_KEY, OUTPUT_ISSUE_VALUE)?;
    Problem::from_key(&key)
}

/// Records a problem, or clears the record once things work again.
///
/// Returns whether this changed what was stored. Callers use that to decide
/// whether to interrupt the user with a balloon: the same problem recurring on
/// every power-off is one notification, not one per cycle.
pub fn set_output_problem(problem: Option<Problem>) -> bool {
    if output_problem() == problem {
        return false;
    }
    match problem {
        Some(p) => set_string(APP_KEY, OUTPUT_ISSUE_VALUE, p.key()),
        // Absent is the "nothing wrong" state, so a missing value is success.
        None => delete_value(APP_KEY, OUTPUT_ISSUE_VALUE) || output_problem().is_none(),
    }
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

    /// Round-trips the real setting, restoring whatever the machine had. The
    /// property under test is that *absence* is the "nothing owed" state, since
    /// the whole restore mechanism is built on presence meaning a debt.
    #[test]
    fn clearing_the_restore_record_leaves_nothing_behind() {
        let previous = restore_output();

        assert!(set_restore_output(Some("{0.0.0.00000000}.{test}")));
        assert_eq!(restore_output().as_deref(), Some("{0.0.0.00000000}.{test}"));
        assert!(set_restore_output(None));
        assert_eq!(restore_output(), None, "a cleared debt must not linger");
        // An empty string is a value, but not a debt: reading it back as one
        // would make the tray try to restore an endpoint that cannot exist.
        assert!(set_string(APP_KEY, RESTORE_OUTPUT_VALUE, ""));
        assert_eq!(restore_output(), None);
        assert!(set_restore_output(None));

        if let Some(p) = previous {
            assert!(set_restore_output(Some(&p)));
        }
    }

    /// The issue record answers two questions with one value: what went wrong,
    /// and whether it is *news*. The second is what keeps a problem that
    /// recurs on every power-off from producing a balloon on every power-off.
    #[test]
    fn recording_the_same_issue_twice_is_not_news() {
        let previous = output_problem();

        assert!(
            set_output_problem(Some(Problem::SwitchFailed)),
            "a new problem is news"
        );
        assert_eq!(output_problem(), Some(Problem::SwitchFailed));
        assert!(
            !set_output_problem(Some(Problem::SwitchFailed)),
            "the same problem again is not"
        );
        assert!(
            set_output_problem(Some(Problem::ChannelsAbsent)),
            "a different one is"
        );
        assert!(set_output_problem(None), "clearing a problem is news");
        assert_eq!(output_problem(), None);
        assert!(
            !set_output_problem(None),
            "clearing nothing must not announce anything"
        );

        let _ = set_output_problem(previous);
    }

    /// The value used to hold the sentence shown to the user. An installation
    /// upgrading across that change must not read the old text back as a
    /// problem it cannot place on any row.
    #[test]
    fn a_record_written_by_an_older_build_reads_as_nothing() {
        let previous = output_problem();

        assert!(set_string(
            APP_KEY,
            OUTPUT_ISSUE_VALUE,
            "Windows refused the change."
        ));
        assert_eq!(output_problem(), None);

        let _ = set_output_problem(previous);
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
