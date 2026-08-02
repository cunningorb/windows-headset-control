//! The only `unsafe` in this crate.
//!
//! Holds the shell notification icon, the popup menu, the message loop, and the
//! Core Audio calls for microphone mute. This module exists instead of a
//! tray-icon dependency: the phase's footprint rule is zero new crates, and a
//! second confined `unsafe` module was judged the better trade. Every other
//! module in `headset-tray` is safe Rust.
//!
//! Mute lives here rather than in the device layer because it is a USB Audio
//! Class control, not a vendor command — see `docs/device-research.md`. There is
//! no vendor set-mute to call.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use windows::core::{w, GUID, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, BOOL, FALSE, HWND, LPARAM, LRESULT, POINT, TRUE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, HBITMAP};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eMultimedia, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DispatchMessageW, GetCursorPos, GetMessageW, PostMessageW, PostQuitMessage,
    RegisterClassW, SetForegroundWindow, TrackPopupMenu, TranslateMessage, HICON, HMENU, ICONINFO,
    MF_CHECKED, MF_DISABLED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, TPM_BOTTOMALIGN,
    TPM_RIGHTALIGN, WINDOW_EX_STYLE, WM_APP, WM_COMMAND, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP,
    WNDCLASSW, WS_OVERLAPPED,
};

use crate::state::HeadsetState;
use crate::worker::Command;

/// Tray icon callback.
const WM_TRAY: u32 = WM_APP + 1;
/// Posted by the worker thread when state changed.
pub const WM_STATE: u32 = WM_APP + 2;

const ID_EXIT: usize = 1;
const ID_MUTE: usize = 2;
const ID_REFRESH: usize = 3;
const ID_STARTUP: usize = 4;
const ID_SYNAPSE_WARNING: usize = 5;
const ID_SIDETONE_BASE: usize = 100;
const ID_GAMECHAT_BASE: usize = 200;

const SIDETONE_MAX: u8 = 15;
const GAMECHAT_MAX: u8 = 20;

struct Ctx {
    icon: HICON,
    commands: Sender<Command>,
    state: Arc<Mutex<HeadsetState>>,
    nid: NOTIFYICONDATAW,
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

/// Runs the tray UI on the calling thread until the user exits.
///
/// Blocks. `state` is shared with the worker thread, which posts [`WM_STATE`]
/// after updating it. `on_window` is called once the window exists, so the
/// caller can hand the handle to that worker.
pub fn run_ui_with<F: FnOnce(isize)>(
    commands: Sender<Command>,
    state: Arc<Mutex<HeadsetState>>,
    on_window: F,
) -> windows::core::Result<()> {
    unsafe {
        // Apartment-threaded because this thread also pumps messages, which is
        // what COM's STA contract expects.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let instance = GetModuleHandleW(None)?;
        let class = w!("HeadsetTrayWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_win32());
        }

        // A real (never-shown) window rather than a message-only one:
        // TrackPopupMenu needs a foreground-capable owner to dismiss cleanly.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("Headset Tray"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            instance,
            None,
        )?;

        let icon = build_icon()?;
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: icon,
            ..Default::default()
        };
        write_tip(&mut nid, "BlackShark V3 Pro - starting");
        Shell_NotifyIconW(NIM_ADD, &nid).ok()?;

        CTX.with(|c| {
            *c.borrow_mut() = Some(Ctx {
                icon,
                commands,
                state,
                nid,
            })
        });

        on_window(hwnd.0 as isize);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Ok(())
    }
}

/// Copies text into the fixed 128-wchar tooltip buffer, truncating rather than
/// overflowing. Windows silently drops an unterminated tip, so the last slot is
/// always left as the NUL.
fn write_tip(nid: &mut NOTIFYICONDATAW, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().take(nid.szTip.len() - 1).collect();
    nid.szTip = [0; 128];
    nid.szTip[..wide.len()].copy_from_slice(&wide);
}

fn refresh_tray(ctx: &mut Ctx) {
    let tip = ctx
        .state
        .lock()
        .map(|s| s.tooltip())
        .unwrap_or_else(|_| "BlackShark V3 Pro".into());
    write_tip(&mut ctx.nid, &tip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &ctx.nid);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            let click = lp.0 as u32;
            if click == WM_RBUTTONUP || click == WM_LBUTTONUP {
                CTX.with(|c| {
                    if let Some(ctx) = c.borrow().as_ref() {
                        show_menu(hwnd, ctx);
                    }
                });
            }
            LRESULT(0)
        }
        WM_STATE => {
            CTX.with(|c| {
                if let Some(ctx) = c.borrow_mut().as_mut() {
                    refresh_tray(ctx);
                }
            });
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wp.0 & 0xFFFF;
            CTX.with(|c| {
                if let Some(ctx) = c.borrow().as_ref() {
                    on_command(id, ctx);
                }
            });
            LRESULT(0)
        }
        WM_DESTROY => {
            CTX.with(|c| {
                if let Some(ctx) = c.borrow_mut().take() {
                    let _ = Shell_NotifyIconW(NIM_DELETE, &ctx.nid);
                    let _ = DestroyIcon(ctx.icon);
                    let _ = ctx.commands.send(Command::Shutdown);
                }
            });
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn on_command(id: usize, ctx: &Ctx) {
    match id {
        ID_EXIT => unsafe {
            let _ = PostMessageW(ctx.nid.hWnd, WM_DESTROY, WPARAM(0), LPARAM(0));
        },
        ID_REFRESH => {
            let _ = ctx.commands.send(Command::Refresh);
        }
        ID_STARTUP => {
            // Point the Run entry at the installed copy when there is one, so
            // enabling startup from a freshly built binary still survives the
            // build directory being cleaned.
            let target = crate::install::installed_exe()
                .filter(|p| p.exists())
                .or_else(|| std::env::current_exe().ok());
            if let Some(exe) = target {
                let now = crate::settings::run_on_startup();
                let _ = crate::settings::set_run_on_startup(!now, &exe);
            }
        }
        ID_SYNAPSE_WARNING => {
            let now = crate::settings::show_synapse_warning();
            if crate::settings::set_show_synapse_warning(!now) {
                if let Ok(mut s) = ctx.state.lock() {
                    // Recompute rather than just clearing: turning the warning
                    // back on must restore it only if Synapse is actually there.
                    s.warn_vendor_software = crate::warn_vendor_software();
                }
                unsafe {
                    let _ = PostMessageW(ctx.nid.hWnd, WM_STATE, WPARAM(0), LPARAM(0));
                }
            }
        }
        ID_MUTE => {
            // Only the OS endpoint can be toggled. The headset's hardware switch
            // is not software-writable and no vendor set-mute command exists.
            let current = get_mic_mute().unwrap_or(false);
            if set_mic_mute(!current).is_ok() {
                if let Ok(mut s) = ctx.state.lock() {
                    s.mic_mute_os = Some(!current);
                }
                unsafe {
                    let _ = PostMessageW(ctx.nid.hWnd, WM_STATE, WPARAM(0), LPARAM(0));
                }
            }
        }
        i if (ID_SIDETONE_BASE..=ID_SIDETONE_BASE + SIDETONE_MAX as usize).contains(&i) => {
            let _ = ctx
                .commands
                .send(Command::SetSidetone((i - ID_SIDETONE_BASE) as u8));
        }
        i if (ID_GAMECHAT_BASE..=ID_GAMECHAT_BASE + GAMECHAT_MAX as usize).contains(&i) => {
            let _ = ctx
                .commands
                .send(Command::SetGameChat((i - ID_GAMECHAT_BASE) as u8));
        }
        _ => {}
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn show_menu(hwnd: HWND, ctx: &Ctx) {
    let state = ctx.state.lock().map(|s| s.clone()).unwrap_or_default();
    let Ok(menu) = CreatePopupMenu() else { return };

    // Status lines are disabled entries: informational, not actionable.
    let battery = match (state.connected, state.battery) {
        (Some(false), _) => "Headset off".to_string(),
        (_, Some(b)) => format!("Battery: {b}%"),
        (_, None) => "Battery: unknown".to_string(),
    };
    let _ = AppendMenuW(
        menu,
        MF_STRING | MF_DISABLED | MF_GRAYED,
        0,
        PCWSTR(wide(&battery).as_ptr()),
    );

    let mute_label = match state.effectively_muted() {
        Some(true) => "Mic: muted",
        Some(false) => "Mic: live",
        None => "Mic: unknown",
    };
    let mute_flags = if state.effectively_muted() == Some(true) {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    let _ = AppendMenuW(menu, mute_flags, ID_MUTE, PCWSTR(wide(mute_label).as_ptr()));

    // A hardware-muted mic cannot be un-muted from here; say so rather than
    // offering a control that will appear not to work.
    if state.mic_mute_hardware == Some(true) {
        let _ = AppendMenuW(
            menu,
            MF_STRING | MF_DISABLED | MF_GRAYED,
            0,
            PCWSTR(wide("  (headset switch - toggle on the headset)").as_ptr()),
        );
    }

    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

    let connected = state.connected != Some(false);
    let sidetone = build_value_menu("Sidetone", ID_SIDETONE_BASE, SIDETONE_MAX, state.sidetone);
    let gamechat = build_value_menu(
        "Game / Chat balance",
        ID_GAMECHAT_BASE,
        GAMECHAT_MAX,
        state.game_chat,
    );
    let sub_flags = if connected {
        MF_STRING | MF_POPUP
    } else {
        MF_STRING | MF_POPUP | MF_DISABLED | MF_GRAYED
    };
    if let Some((h, label)) = sidetone {
        let _ = AppendMenuW(menu, sub_flags, h.0 as usize, PCWSTR(wide(&label).as_ptr()));
    }
    if let Some((h, label)) = gamechat {
        let _ = AppendMenuW(menu, sub_flags, h.0 as usize, PCWSTR(wide(&label).as_ptr()));
    }

    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        ID_REFRESH,
        PCWSTR(wide("Refresh").as_ptr()),
    );
    if let Some(settings_menu) = build_settings_menu() {
        let _ = AppendMenuW(
            menu,
            MF_STRING | MF_POPUP,
            settings_menu.0 as usize,
            PCWSTR(wide("Settings").as_ptr()),
        );
    }

    if state.warn_vendor_software {
        let _ = AppendMenuW(
            menu,
            MF_STRING | MF_DISABLED | MF_GRAYED,
            0,
            PCWSTR(wide("Synapse is running and may override settings").as_ptr()),
        );
    }
    let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, PCWSTR(wide("Exit").as_ptr()));

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    // Required so the menu dismisses when the user clicks elsewhere; without it
    // the popup can stick around after focus moves.
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(
        menu,
        TPM_RIGHTALIGN | TPM_BOTTOMALIGN,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );
    let _ = DestroyMenu(menu);
}

/// Builds the Settings submenu.
///
/// Both entries read their state live rather than from a cached copy. The
/// startup checkbox in particular reflects the same registry value Windows
/// reads at logon, so disabling the entry from Task Manager's Startup tab shows
/// up here rather than being contradicted by a stale setting.
unsafe fn build_settings_menu() -> Option<HMENU> {
    let sub = CreatePopupMenu().ok()?;

    let startup_on = crate::settings::run_on_startup();
    let startup_flags = if startup_on {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    let _ = AppendMenuW(
        sub,
        startup_flags,
        ID_STARTUP,
        PCWSTR(wide("Run on Windows startup").as_ptr()),
    );

    let warn_on = crate::settings::show_synapse_warning();
    let warn_flags = if warn_on {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    let _ = AppendMenuW(
        sub,
        warn_flags,
        ID_SYNAPSE_WARNING,
        PCWSTR(wide("Warn when Synapse is running").as_ptr()),
    );

    // Where the executable actually lives matters when startup is on: a Run
    // entry pointing at a build output in a source tree will break the moment
    // that tree is cleaned. Show it rather than let it fail silently later.
    if startup_on && !crate::install::running_from_install_dir() {
        let _ = AppendMenuW(sub, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            sub,
            MF_STRING | MF_DISABLED | MF_GRAYED,
            0,
            PCWSTR(wide("Running from outside the install folder").as_ptr()),
        );
    }
    Some(sub)
}

/// Builds a submenu of discrete values, checking the current one.
unsafe fn build_value_menu(
    label: &str,
    base: usize,
    max: u8,
    current: Option<u8>,
) -> Option<(HMENU, String)> {
    let sub = CreatePopupMenu().ok()?;
    for v in 0..=max {
        let flags = if current == Some(v) {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        let _ = AppendMenuW(
            sub,
            flags,
            base + v as usize,
            PCWSTR(wide(&v.to_string()).as_ptr()),
        );
    }
    let title = match current {
        Some(v) => format!("{label}: {v}"),
        None => format!("{label}: unknown"),
    };
    Some((sub, title))
}

/// Draws a 32x32 headset glyph.
///
/// Procedural rather than an embedded resource: adding an `.ico` would mean a
/// build script and a resource-compiler dependency, and the footprint rule for
/// this phase is zero new crates. Light fill with a dark outline so it stays
/// legible on both light and dark taskbars.
unsafe fn build_icon() -> windows::core::Result<HICON> {
    const N: usize = 32;
    let mut shape = [false; N * N];

    let put = |shape: &mut [bool; N * N], x: i32, y: i32| {
        if (0..N as i32).contains(&x) && (0..N as i32).contains(&y) {
            shape[y as usize * N + x as usize] = true;
        }
    };

    // Headband: an arc centred low so the band sits across the top.
    let (cx, cy) = (16.0f32, 19.0f32);
    for y in 0..N as i32 {
        for x in 0..N as i32 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let r = (dx * dx + dy * dy).sqrt();
            if (10.5..=13.5).contains(&r) && dy < 0.0 {
                put(&mut shape, x, y);
            }
        }
    }
    // Ear cups.
    for y in 15..27 {
        for x in 0..N as i32 {
            let left = (3..=8).contains(&x);
            let right = (23..=28).contains(&x);
            // Round the vertical ends slightly.
            let end = y == 15 || y == 26;
            if (left || right) && !(end && (x == 3 || x == 8 || x == 23 || x == 28)) {
                put(&mut shape, x, y);
            }
        }
    }

    // BGRA, straight alpha. Outline any transparent pixel that touches the
    // shape, so the glyph reads on a light taskbar as well as a dark one.
    let fill: u32 = 0xFF_F0F0F0;
    let outline: u32 = 0xFF_1A1A1A;
    let mut pixels = vec![0u32; N * N];
    for y in 0..N as i32 {
        for x in 0..N as i32 {
            let i = y as usize * N + x as usize;
            if shape[i] {
                pixels[i] = fill;
                continue;
            }
            let touches = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..N as i32).contains(&nx)
                        && (0..N as i32).contains(&ny)
                        && shape[ny as usize * N + nx as usize]
                })
            });
            if touches {
                pixels[i] = outline;
            }
        }
    }

    let color: HBITMAP = CreateBitmap(
        N as i32,
        N as i32,
        1,
        32,
        Some(pixels.as_ptr() as *const std::ffi::c_void),
    );
    // An all-zero mask means "use the colour bitmap's alpha everywhere".
    let mask: HBITMAP = CreateBitmap(N as i32, N as i32, 1, 1, None);
    let info = ICONINFO {
        fIcon: TRUE,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask,
        hbmColor: color,
    };
    let icon = CreateIconIndirect(&info)?;
    let _ = DeleteObject(color);
    let _ = DeleteObject(mask);
    Ok(icon)
}

fn endpoint_volume() -> windows::core::Result<IAudioEndpointVolume> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eCapture, eMultimedia)?;
        device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
    }
}

/// Reads the default capture endpoint's mute.
///
/// This is the Windows-side mute, which is independent of the headset's
/// hardware switch: toggling one was observed to produce no traffic for the
/// other.
pub fn get_mic_mute() -> windows::core::Result<bool> {
    unsafe { Ok(endpoint_volume()?.GetMute()?.as_bool()) }
}

pub fn set_mic_mute(muted: bool) -> windows::core::Result<()> {
    unsafe {
        endpoint_volume()?.SetMute(BOOL::from(muted), std::ptr::null::<GUID>())?;
        Ok(())
    }
}

/// Posts a state-changed message to the UI thread. Safe to call from the worker.
pub fn post_state(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let _ = PostMessageW(
            HWND(hwnd as *mut std::ffi::c_void),
            WM_STATE,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

/// Whether a process with this executable name is running.
///
/// Used to warn that the vendor engine will contend for settings. Advisory
/// only: a false negative costs a missing tooltip line, nothing more.
pub fn process_running(exe_name: &str) -> bool {
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return false;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found = false;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case(exe_name) {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        found
    }
}

/// `FALSE` exists to keep the unused-import lint quiet in builds where the
/// Core Audio path is compiled but never called.
#[allow(dead_code)]
const _UNUSED: BOOL = FALSE;
