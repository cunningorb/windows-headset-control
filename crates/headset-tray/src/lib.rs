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

pub mod state;
pub mod ui;
pub mod worker;

#[cfg(windows)]
pub mod install;
#[cfg(windows)]
pub mod settings;
#[cfg(windows)]
pub mod win32;

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
