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
pub mod worker;

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
pub fn vendor_software_running() -> bool {
    win32::process_running("RazerAppEngine.exe")
}

#[cfg(not(windows))]
pub fn vendor_software_running() -> bool {
    false
}
