//! Windows tray application for supported wireless headsets.
//!
//! Three parts: a safe state model (`state`), a device thread that owns the one
//! control session (`worker`), and a confined `unsafe` Win32 module for the tray
//! icon, menus, and Core Audio (`win32`).
//!
//! The crate is deliberately dependency-free beyond the workspace's own crates
//! and the already-pinned `windows`. Replacing a heavyweight vendor application
//! with another heavyweight application would defeat the point.

#![cfg_attr(not(windows), forbid(unsafe_code))]

pub mod output;
pub mod state;
pub mod ui;
pub mod worker;

#[cfg(windows)]
pub mod install;
#[cfg(windows)]
pub mod settings;
#[cfg(windows)]
pub mod win32;

pub use output::Slot;
pub use state::HeadsetState;
pub use worker::Command;

/// Whether Razer's engine is running.
///
/// It re-applies settings in response to device events — writing sidetone on
/// every mic-mute transition, for one — so it will contend with this tray. The
/// tray warns rather than refusing to run, because trying this out before
/// uninstalling Synapse is the normal way someone will arrive here.
#[cfg(windows)]
pub fn warn_vendor_software() -> bool {
    settings::show_synapse_warning() && win32::process_running("RazerAppEngine.exe")
}

/// Settings readers the layout uses to render toggle positions.
///
/// Wrapped here so `ui::layout` stays free of `#[cfg(windows)]` and can be
/// compiled and tested on its own terms; off Windows they report the defaults.
#[cfg(windows)]
pub fn settings_run_on_startup() -> bool {
    settings::run_on_startup()
}

#[cfg(windows)]
pub fn settings_show_warning() -> bool {
    settings::show_synapse_warning()
}

#[cfg(windows)]
pub fn settings_switch_output() -> bool {
    settings::switch_output_when_off()
}

/// Whether the headset's game and chat channels are set apart when it returns.
#[cfg(windows)]
pub fn settings_split_game_and_chat() -> bool {
    settings::split_game_and_chat()
}

/// What stopped the output switch or the split working last time, if anything.
///
/// The panel reads this rather than being told: the failure it describes
/// happened while the panel was closed, which is the whole reason it is
/// written down.
#[cfg(windows)]
pub fn settings_output_problem() -> Option<output::Problem> {
    settings::output_problem()
}

/// A chosen device as `(name, is it plugged in right now)`.
///
/// Presence is resolved here rather than in `layout` because it takes a Core
/// Audio enumeration; the wording built from it stays pure and testable in
/// `layout::output_choice_subtitle`.
#[cfg(windows)]
pub fn settings_output_choice(slot: Slot) -> Option<(String, bool)> {
    let (id, name) = settings::output_choice(slot)?;
    Some((name, win32::audio::is_present(&id)))
}

#[cfg(windows)]
pub fn settings_output_choice_id(slot: Slot) -> Option<String> {
    settings::output_choice(slot).map(|(id, _)| id)
}

/// Every playback device, as `(endpoint id, name)`.
#[cfg(windows)]
pub fn output_devices() -> Vec<(String, String)> {
    win32::audio::render_outputs()
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect()
}

#[cfg(not(windows))]
pub fn settings_run_on_startup() -> bool {
    false
}

#[cfg(not(windows))]
pub fn settings_show_warning() -> bool {
    true
}

#[cfg(not(windows))]
pub fn warn_vendor_software() -> bool {
    false
}

#[cfg(not(windows))]
pub fn settings_switch_output() -> bool {
    false
}

#[cfg(not(windows))]
pub fn settings_split_game_and_chat() -> bool {
    false
}

#[cfg(not(windows))]
pub fn settings_output_problem() -> Option<output::Problem> {
    None
}

#[cfg(not(windows))]
pub fn settings_output_choice(_slot: Slot) -> Option<(String, bool)> {
    None
}

#[cfg(not(windows))]
pub fn settings_output_choice_id(_slot: Slot) -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn output_devices() -> Vec<(String, String)> {
    Vec::new()
}
