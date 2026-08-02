//! Tray application entry point.
//!
//! `windows_subsystem = "windows"` so launching it does not flash a console.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
compile_error!("headset-tray targets Windows only");

#[cfg(windows)]
fn main() {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::{mpsc, Arc, Mutex};

    use headset_tray::{state::HeadsetState, win32, worker};

    tracing_subscriber_init();

    let (tx, rx) = mpsc::channel();
    let state = Arc::new(Mutex::new(HeadsetState::default()));

    // The worker starts before the window exists, so it needs somewhere to
    // learn the handle once there is one. Until then its notifications are
    // dropped, which is correct: there is no UI to repaint yet.
    let hwnd = Arc::new(AtomicIsize::new(0));

    let worker_state = state.clone();
    let worker_hwnd = hwnd.clone();
    let device_thread = std::thread::spawn(move || {
        let backend = headset_device::WindowsHidBackend::new();
        worker::run(&backend, rx, |s| {
            if let Ok(mut guard) = worker_state.lock() {
                *guard = s;
            }
            win32::post_state(worker_hwnd.load(Ordering::Relaxed));
        });
    });

    // Seed the OS-side mute before the first paint so the menu does not open
    // showing "unknown" for something we can read synchronously.
    if let Ok(m) = win32::get_mic_mute() {
        if let Ok(mut guard) = state.lock() {
            guard.mic_mute_os = Some(m);
        }
    }

    if let Err(e) = win32::run_ui_with(tx.clone(), state, |h| {
        hwnd.store(h, Ordering::Relaxed);
    }) {
        eprintln!("tray failed to start: {e}");
    }

    let _ = tx.send(worker::Command::Shutdown);
    let _ = device_thread.join();
}

#[cfg(windows)]
fn tracing_subscriber_init() {
    // Logging is opt-in via HEADSET_TRAY_LOG; a tray app has no console by
    // default, so the default is to stay silent rather than write nowhere.
    if std::env::var("HEADSET_TRAY_LOG").is_ok() {
        tracing::subscriber::set_global_default(
            tracing_subscriber::FmtSubscriber::builder()
                .with_writer(std::io::stderr)
                .finish(),
        )
        .ok();
    }
}
