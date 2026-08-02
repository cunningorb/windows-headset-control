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
    DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, LoadCursorW,
    PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
    TrackPopupMenu, TranslateMessage, HICON, ICONINFO, IDC_ARROW, MF_SEPARATOR, MF_STRING, MSG,
    TPM_BOTTOMALIGN, TPM_RIGHTALIGN, WINDOW_EX_STYLE, WM_ACTIVATE, WM_APP, WM_CLOSE, WM_COMMAND,
    WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};

// Not part of any public API: these are the tray's own window plumbing, and
// several are `unsafe` in ways only this module's call sites can uphold.
pub(crate) mod panel;
pub(crate) mod place;

use crate::state::HeadsetState;
use crate::ui::{self, HitTarget, SliderParam, View};
use crate::worker::Command;
use headset_protocol::{NoiseControl, NoiseMode};

/// Tray icon callback.
const WM_TRAY: u32 = WM_APP + 1;
/// Posted by the worker thread when state changed.
pub const WM_STATE: u32 = WM_APP + 2;

/// The shell's "I have restarted, re-add your icon" broadcast.
///
/// Registered at runtime rather than being a constant: `RegisterWindowMessageW`
/// allocates the value, and every process that registers the same string gets
/// the same number. Zero means registration failed, which must never match an
/// incoming message.
static TASKBAR_CREATED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

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
    /// The popup. Created once and shown/hidden, so its Direct2D resources and
    /// window handle survive across openings.
    panel_hwnd: HWND,
    renderer: Option<crate::ui::render::Renderer>,
    view: crate::ui::View,
    param: crate::ui::SliderParam,
    /// Hit regions from the last render, so a click can be resolved against
    /// what is actually on screen rather than a freshly computed guess.
    hits: Vec<(crate::ui::layout::Rect, crate::ui::HitTarget)>,
    track: Option<crate::ui::layout::TrackGeometry>,
    /// ANC level track from the last render. `None` in every mode but ANC.
    level_track: Option<crate::ui::layout::LevelTrack>,
    /// Value being dragged. Shown instead of the device's value until release,
    /// which is what makes one write per adjustment rather than twenty.
    drag: Option<u8>,
    panel_visible: bool,
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

/// Runs `f` with the tray context, or does nothing if we are already inside a
/// handler that holds it.
///
/// This guard is not optional. `ShowWindow`, `SetForegroundWindow` and
/// `UpdateLayeredWindow` dispatch messages **synchronously**, so a handler that
/// calls them re-enters this window procedure before returning. Holding a
/// `RefCell` borrow across that is a double-borrow panic, and with
/// `panic = "abort"` the process dies on the spot — which looks from outside
/// like the panel opening and then freezing, because a layered window's pixels
/// outlive the process that drew them.
///
/// Skipping the nested call is the correct behaviour, not a workaround: the
/// outer call is already mutating the same state, and the message that provoked
/// the re-entry was caused by us rather than by the user.
fn with_ctx<R>(f: impl FnOnce(&mut Ctx) -> R) -> Option<R> {
    CTX.with(|c| match c.try_borrow_mut() {
        Ok(mut guard) => guard.as_mut().map(f),
        Err(_) => {
            tracing::debug!("ignoring a re-entrant window message");
            None
        }
    })
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
        // A class with a null hCursor never sets the pointer shape, so the
        // cursor keeps whatever it was when it entered -- which is the
        // app-starting hourglass for a freshly launched process. That is the
        // "loading cursor that never goes away" over the panel.
        let arrow = LoadCursorW(None, IDC_ARROW)?;

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: arrow,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_win32());
        }
        let panel_class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: panel::CLASS_NAME,
            hCursor: arrow,
            ..Default::default()
        };
        if RegisterClassW(&panel_class) == 0 {
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

        // Registered before the icon is added: if the shell restarts between the
        // two, the broadcast still has somewhere to land.
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(w!("TaskbarCreated")),
            std::sync::atomic::Ordering::Relaxed,
        );

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

        let panel_hwnd = panel::create(instance)?;
        panel::set_tag(panel_hwnd, panel::TAG_PANEL);

        // A failed renderer must not take the tray down with it: the icon,
        // tooltip and right-click menu still work without the panel.
        let renderer = match ui::render::Renderer::new() {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::error!("panel renderer unavailable: {e}");
                None
            }
        };

        CTX.with(|c| {
            *c.borrow_mut() = Some(Ctx {
                icon,
                commands,
                state,
                nid,
                panel_hwnd,
                renderer,
                view: View::Main,
                param: SliderParam::GameChat,
                hits: Vec::new(),
                track: None,
                level_track: None,
                drag: None,
                panel_visible: false,
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

/// Re-registers the notification icon after the shell restarted.
///
/// The previous icon was destroyed with the old taskbar, so this is `NIM_ADD`
/// and not `NIM_MODIFY`; modifying an icon the shell no longer knows about
/// fails silently and leaves the tray empty.
fn readd_icon(ctx: &mut Ctx) {
    unsafe {
        // Delete first, ignoring failure. If the shell somehow does still hold
        // the icon, adding a second one would leave a duplicate that no message
        // ever reaches.
        let _ = Shell_NotifyIconW(NIM_DELETE, &ctx.nid);
        if Shell_NotifyIconW(NIM_ADD, &ctx.nid).as_bool() {
            tracing::info!("re-added the tray icon after a shell restart");
        } else {
            tracing::error!("could not re-add the tray icon after a shell restart");
        }
    }
    refresh_tray(ctx);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            let click = lp.0 as u32;
            if click == WM_LBUTTONUP {
                with_ctx(toggle_panel);
            } else if click == WM_RBUTTONUP {
                // Right-click keeps a minimal classic menu, which is where Exit
                // lives: the panel design has no room for it.
                with_ctx(|ctx| show_menu(hwnd, ctx));
            }
            LRESULT(0)
        }
        WM_STATE => {
            CTX.with(|c| {
                if let Some(ctx) = c.borrow_mut().as_mut() {
                    refresh_tray(ctx);
                    if ctx.panel_visible {
                        redraw_panel(ctx);
                    }
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONDOWN if panel::tag(hwnd) == panel::TAG_PANEL => {
            let (x, y) = panel::point_from_lparam(lp);
            with_ctx(|ctx| on_panel_press(ctx, x, y));
            LRESULT(0)
        }
        WM_MOUSEMOVE if panel::tag(hwnd) == panel::TAG_PANEL => {
            if panel::left_button_down(wp) {
                let (x, _) = panel::point_from_lparam(lp);
                with_ctx(|ctx| on_panel_drag(ctx, x));
            }
            LRESULT(0)
        }
        WM_LBUTTONUP if panel::tag(hwnd) == panel::TAG_PANEL => {
            with_ctx(on_panel_release);
            LRESULT(0)
        }
        // Clicking away closes the panel, which is what a tray popup should do.
        WM_ACTIVATE if panel::tag(hwnd) == panel::TAG_PANEL && wp.0 == 0 => {
            with_ctx(hide_panel);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wp.0 & 0xFFFF;
            with_ctx(|ctx| on_command(id, ctx));
            LRESULT(0)
        }
        // Only the owner window's destruction ends the application. Both windows
        // share this procedure, and tearing down on the panel's WM_DESTROY would
        // take the context away while leaving the panel on screen — a live
        // process with an orphaned window that ignores every click.
        WM_DESTROY if panel::tag(hwnd) != panel::TAG_PANEL => {
            CTX.with(|c| {
                if let Some(ctx) = c.borrow_mut().take() {
                    let _ = Shell_NotifyIconW(NIM_DELETE, &ctx.nid);
                    let _ = DestroyIcon(ctx.icon);
                    let _ = ctx.commands.send(Command::Shutdown);
                    // Take the panel with us; otherwise its pixels outlive the
                    // message loop and look like a frozen window.
                    if !ctx.panel_hwnd.is_invalid() {
                        let _ = DestroyWindow(ctx.panel_hwnd);
                    }
                }
            });
            PostQuitMessage(0);
            LRESULT(0)
        }
        // The shell restarted and took every tray icon with it. Re-register, and
        // drop the panel: it was anchored to an icon that no longer exists.
        m if m != 0 && m == TASKBAR_CREATED.load(std::sync::atomic::Ordering::Relaxed) => {
            with_ctx(|ctx| {
                if ctx.panel_visible {
                    hide_panel(ctx);
                }
                readd_icon(ctx);
            });
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Renders the current state and pushes it into the layered window.
fn redraw_panel(ctx: &mut Ctx) {
    let Some(renderer) = ctx.renderer.as_ref() else {
        return;
    };
    let state = ctx.state.lock().map(|s| s.clone()).unwrap_or_default();
    let panel = ui::build(&state, ctx.view, ctx.param, ctx.drag);
    let img = match renderer.render(&panel, 1.0) {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("panel render failed: {e}");
            return;
        }
    };
    // Keep the regions that match what is now on screen, so a click resolves
    // against the drawn layout rather than a recomputed one.
    ctx.hits = panel.hits.clone();
    ctx.track = panel.track;
    ctx.level_track = panel.level_track;

    let first_show = !ctx.panel_visible;
    unsafe {
        let (x, y) = if first_show {
            panel::anchor(
                ctx.nid.hWnd,
                ctx.nid.uID,
                img.width as i32,
                img.height as i32,
            )
        } else {
            // Keep the panel where it is across repaints; re-anchoring to the
            // cursor would make it walk around the screen as values update.
            // Held by the bottom edge, because that is the edge sitting just
            // above the tray icon and a repaint can change the height.
            panel::reanchor_bottom(ctx.panel_hwnd, img.height as i32)
        };
        if let Err(e) = panel::show(ctx.panel_hwnd, x, y, &img, first_show) {
            tracing::error!("showing the panel failed: {e}");
            return;
        }
    }
    ctx.panel_visible = true;
}

fn hide_panel(ctx: &mut Ctx) {
    unsafe { panel::hide(ctx.panel_hwnd) };
    ctx.panel_visible = false;
    ctx.drag = None;
    // Always reopen on the main view; landing back in Settings is disorienting.
    ctx.view = View::Main;
}

fn toggle_panel(ctx: &mut Ctx) {
    if ctx.panel_visible {
        hide_panel(ctx);
    } else {
        // Ask for fresh values on open rather than showing whatever was last
        // pushed: the device is the source of truth.
        let _ = ctx.commands.send(Command::Refresh);
        // Re-evaluate the warning here rather than trusting a cached answer:
        // Synapse may have started or stopped since the panel was last open.
        if let Ok(mut s) = ctx.state.lock() {
            s.warn_vendor_software = crate::warn_vendor_software();
        }
        redraw_panel(ctx);
    }
}

fn on_panel_press(ctx: &mut Ctx, x: f32, y: f32) {
    // Panel-local coordinates: the window includes the shadow margin.
    let (lx, ly) = (x - crate::ui::theme::SHADOW, y - crate::ui::theme::SHADOW);
    let Some(target) = ctx
        .hits
        .iter()
        .find(|(r, _)| r.contains(lx, ly))
        .map(|(_, t)| *t)
    else {
        return;
    };

    match target {
        HitTarget::Gear => {
            ctx.view = View::Settings;
            redraw_panel(ctx);
        }
        HitTarget::Back => {
            ctx.view = View::Main;
            redraw_panel(ctx);
        }
        HitTarget::Refresh => {
            let _ = ctx.commands.send(Command::Refresh);
            // Refresh means "re-read everything", including whether Synapse is
            // still running. Only the device half comes back from the worker.
            if let Ok(mut s) = ctx.state.lock() {
                s.warn_vendor_software = crate::warn_vendor_software();
            }
            redraw_panel(ctx);
        }
        HitTarget::Switcher => {
            ctx.param = ctx.param.other();
            redraw_panel(ctx);
        }
        HitTarget::MutePill => {
            let current = get_mic_mute().unwrap_or(false);
            if set_mic_mute(!current).is_ok() {
                if let Ok(mut s) = ctx.state.lock() {
                    s.mic_mute_os = Some(!current);
                }
                redraw_panel(ctx);
            }
        }
        HitTarget::SliderTrack => {
            if let Some(g) = ctx.track {
                ctx.drag = Some(g.value_at(lx));
                redraw_panel(ctx);
            }
        }
        HitTarget::NoiseOff => set_noise_mode(ctx, NoiseMode::Off),
        HitTarget::NoiseAnc => set_noise_mode(ctx, NoiseMode::Anc),
        HitTarget::NoiseAmbient => set_noise_mode(ctx, NoiseMode::Ambient),
        HitTarget::NoiseLevel => {
            if let Some(t) = ctx.level_track {
                send_noise(ctx, |n| NoiseControl {
                    anc_level: t.level_at(lx),
                    ..n
                });
            }
        }
        HitTarget::ToggleStartup => {
            let target_exe = crate::install::installed_exe()
                .filter(|p| p.exists())
                .or_else(|| std::env::current_exe().ok());
            if let Some(exe) = target_exe {
                let now = crate::settings::run_on_startup();
                let _ = crate::settings::set_run_on_startup(!now, &exe);
                redraw_panel(ctx);
            }
        }
        HitTarget::ToggleWarning => {
            let now = crate::settings::show_synapse_warning();
            if crate::settings::set_show_synapse_warning(!now) {
                if let Ok(mut s) = ctx.state.lock() {
                    s.warn_vendor_software = crate::warn_vendor_software();
                }
                redraw_panel(ctx);
            }
        }
    }
}

fn set_noise_mode(ctx: &mut Ctx, mode: NoiseMode) {
    send_noise(ctx, |n| NoiseControl { mode, ..n });
}

/// Read-modify-write, on the UI side of the wire.
///
/// The device holds mode and level in one two-byte parameter, so a change to
/// either has to carry the other. `f` is handed the state the panel is
/// currently showing and returns the whole thing.
///
/// Nothing is sent when the current state is unknown: composing a write would
/// mean inventing the byte we did not read, and the panel does not hit-test the
/// noise row while the headset is unreachable anyway.
fn send_noise(ctx: &mut Ctx, f: impl FnOnce(NoiseControl) -> NoiseControl) {
    let current = ctx.state.lock().ok().and_then(|s| s.noise);
    let Some(current) = current else { return };
    let _ = ctx.commands.send(Command::SetNoise(f(current)));
}

fn on_panel_drag(ctx: &mut Ctx, x: f32) {
    if ctx.drag.is_none() {
        return;
    }
    let Some(g) = ctx.track else { return };
    let v = g.value_at(x - crate::ui::theme::SHADOW);
    if ctx.drag != Some(v) {
        ctx.drag = Some(v);
        redraw_panel(ctx);
    }
}

/// Commits the dragged value.
///
/// One write per adjustment: requests are paced at 250 ms and a sidetone write
/// costs two exchanges, so writing every step of a drag would put the device
/// seconds behind the knob.
fn on_panel_release(ctx: &mut Ctx) {
    let Some(v) = ctx.drag.take() else { return };
    let cmd = match ctx.param {
        SliderParam::Sidetone => Command::SetSidetone(v),
        SliderParam::GameChat => Command::SetGameChat(v),
    };
    let _ = ctx.commands.send(cmd);
    redraw_panel(ctx);
}

fn on_command(id: usize, ctx: &Ctx) {
    match id {
        ID_EXIT => unsafe {
            // WM_CLOSE, not a synthetic WM_DESTROY: posting WM_DESTROY runs the
            // teardown handler without the window actually being destroyed,
            // which leaves a live process holding windows nothing can reach.
            let _ = PostMessageW(ctx.nid.hWnd, WM_CLOSE, WPARAM(0), LPARAM(0));
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

/// The right-click menu.
///
/// Deliberately minimal: everything about the headset lives in the panel now.
/// This exists because the panel design has nowhere sensible to put Exit, and a
/// tray icon with no way to quit is hostile.
unsafe fn show_menu(hwnd: HWND, ctx: &Ctx) {
    let Ok(menu) = CreatePopupMenu() else { return };
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        ID_REFRESH,
        PCWSTR(wide("Refresh").as_ptr()),
    );
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, PCWSTR(wide("Exit").as_ptr()));

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    // Required so the menu dismisses when focus moves elsewhere.
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
    let _ = ctx;
}

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
