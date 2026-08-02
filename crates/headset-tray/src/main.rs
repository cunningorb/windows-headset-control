//! Tray application entry point.
//!
//! `windows_subsystem = "windows"` so launching it does not flash a console.
//! `--install` and `--uninstall` still need to say something, so they attach to
//! the parent console when there is one and fall back to a message box when the
//! user double-clicked the exe.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
compile_error!("headset-tray targets Windows only");

#[cfg(windows)]
fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "--install" => run_install(),
        "--uninstall" => run_uninstall(),
        "--help" | "-h" | "/?" => report(
            "Headset Tray",
            "Usage:\n  headset-tray.exe              run the tray\n  \
             headset-tray.exe --install    install and run at logon\n  \
             headset-tray.exe --uninstall  remove startup and the uninstall entry",
        ),
        _ => run_tray(),
    }
}

#[cfg(windows)]
fn run_install() {
    use headset_tray::install;
    match install::install() {
        Ok(path) => {
            // Launch the installed copy, not this one: if --install was run from
            // a build directory, the running image is the wrong binary to leave
            // resident. Report a spawn failure rather than discarding it -- a
            // silent failure here looks exactly like a successful install that
            // mysteriously did not start.
            // Detach the child's stdio explicitly. Inheriting it makes the spawn
            // depend on console state: `report` may call FreeConsole, which
            // invalidates inherited console handles and makes CreateProcess fail
            // with a confusing error. Null stdio removes the coupling entirely
            // rather than relying on the order these two run in.
            let started = std::process::Command::new(&path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            let note = match started {
                Ok(_) => "Starting it now.".to_string(),
                Err(e) => format!("Could not start it now ({e}); it will start at next sign-in."),
            };
            report(
                "Headset Tray installed",
                &format!(
                    "Installed to:\n{}\n\nIt will start automatically when you sign in.\n{note}",
                    path.display()
                ),
            );
        }
        Err(e) => report(
            "Headset Tray install failed",
            &format!("Could not install: {e}"),
        ),
    }
}

#[cfg(windows)]
fn run_uninstall() {
    use headset_tray::install;
    match install::uninstall() {
        Ok(exe) => {
            let where_ = exe
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "the install folder".into());
            report(
                "Headset Tray uninstalled",
                &format!(
                    "Startup entry and uninstall registration removed.\n\n\
                     The program file itself is still in:\n{where_}\n\n\
                     Windows will not let a running program delete itself, so \
                     remove that folder manually if you want it gone.",
                ),
            );
        }
        Err(e) => report(
            "Headset Tray uninstall failed",
            &format!("Could not uninstall: {e}"),
        ),
    }
}

#[cfg(windows)]
fn run_tray() {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::{mpsc, Arc, Mutex};

    use headset_tray::{install, state::HeadsetState, win32, worker};

    tracing_subscriber_init();
    // A previous --install may have parked the old image alongside the new one;
    // by now it is no longer running, so it can go.
    install::tidy_previous_upgrade();

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
        report("Headset Tray failed to start", &format!("{e}"));
    }

    let _ = tx.send(worker::Command::Shutdown);
    let _ = device_thread.join();
}

/// Prints to the parent console if there is one, otherwise shows a message box.
///
/// A `windows_subsystem = "windows"` binary has no console of its own, so
/// running `--install` from a terminal would otherwise produce silence.
///
/// `AttachConsole` alone is not enough: it gives the process a console, but the
/// standard-output handle was already resolved (to nothing) at startup, so
/// `println!` still goes nowhere. Writing to `CONOUT$` directly is what actually
/// reaches the terminal.
#[cfg(windows)]
fn report(title: &str, body: &str) {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, WriteFile, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows::Win32::System::Console::{
        AttachConsole, FreeConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

    let text = format!("{title}\n\n{body}\n");

    // Case 1: stdout is already a real handle -- the caller redirected us to a
    // file or a pipe. Ordinary printing works and is what they asked for.
    let inherited = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if matches!(inherited, Ok(h) if !h.is_invalid() && h.0 as usize != 0) {
        use std::io::Write as _;
        print!("{text}");
        let _ = std::io::stdout().flush();
        return;
    }

    // Case 2: launched from a terminal that gave us no stdout of our own.
    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_ok() {
        let name: Vec<u16> = "CONOUT$".encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe {
            CreateFileW(
                PCWSTR(name.as_ptr()),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };
        if let Ok(h) = handle {
            let bytes = text.as_bytes();
            let mut written = 0u32;
            unsafe {
                let _ = WriteFile(h, Some(bytes), Some(&mut written), None);
                let _ = CloseHandle(h);
            }
        }
        unsafe {
            let _ = FreeConsole();
        }
        return;
    }

    // No parent console: the user double-clicked, so a dialog is the only way
    // to say anything at all.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(body),
            &HSTRING::from(title),
            MB_OK | MB_ICONINFORMATION,
        );
    }
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
