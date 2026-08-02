//! The popup panel window.
//!
//! A layered window rather than an ordinary one: the mockup's drop shadow,
//! rounded corners and knob glow all need per-pixel alpha to composite over
//! whatever happens to be behind the tray — taskbar, wallpaper, another window.
//! `UpdateLayeredWindow` takes the premultiplied bitmap `ui::render` produced and
//! does exactly that.

#![allow(unsafe_code)]

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS,
    HBITMAP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    UpdateLayeredWindow, GWLP_USERDATA, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SW_HIDE,
    SW_SHOWNOACTIVATE, ULW_ALPHA, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::ui::render::RenderedPanel;
use crate::win32::place;

pub const CLASS_NAME: PCWSTR = w!("HeadsetTrayPanel");

/// Creates the panel window. Hidden until first shown.
///
/// `WS_EX_TOOLWINDOW` keeps it out of the taskbar and Alt-Tab, which is what a
/// tray popup should do.
pub unsafe fn create(instance: windows::Win32::Foundation::HMODULE) -> windows::core::Result<HWND> {
    CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        CLASS_NAME,
        w!(""),
        WS_POPUP,
        0,
        0,
        0,
        0,
        None,
        None,
        instance,
        None,
    )
}

/// Pushes a rendered bitmap into the window and shows it at `(x, y)`.
///
/// Position is the window's top-left in screen coordinates; the caller is
/// responsible for having clamped it to a monitor's work area.
/// `activate` should be true only when opening. Re-activating on every repaint
/// would steal focus each time the worker reports a new battery reading.
pub unsafe fn show(
    hwnd: HWND,
    x: i32,
    y: i32,
    img: &RenderedPanel,
    activate: bool,
) -> windows::core::Result<()> {
    let screen = GetDC(None);
    let mem = CreateCompatibleDC(screen);

    let header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: img.width as i32,
            // Negative height: top-down rows, matching how the renderer wrote
            // them. Bottom-up would show the panel upside down.
            biHeight: -(img.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let dib: HBITMAP = CreateDIBSection(mem, &header, DIB_RGB_COLORS, &mut bits, None, 0)?;
    if !bits.is_null() {
        std::ptr::copy_nonoverlapping(img.bgra.as_ptr(), bits as *mut u8, img.bgra.len());
    }
    let old = SelectObject(mem, dib);

    let size = SIZE {
        cx: img.width as i32,
        cy: img.height as i32,
    };
    let src = POINT { x: 0, y: 0 };
    let dst = POINT { x, y };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };

    let result = UpdateLayeredWindow(
        hwnd,
        screen,
        Some(&dst),
        Some(&size),
        mem,
        Some(&src),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );

    SelectObject(mem, old);
    let _ = DeleteObject(dib);
    let _ = DeleteDC(mem);
    ReleaseDC(None, screen);
    result?;

    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
    if activate {
        // Foreground so that clicking elsewhere deactivates us and the panel
        // can close itself.
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
    }
    Ok(())
}

pub unsafe fn hide(hwnd: HWND) {
    let _ = ShowWindow(hwnd, SW_HIDE);
}

/// The work area of the monitor `r` sits on.
///
/// **Not** `SystemParametersInfoW(SPI_GETWORKAREA)`, which is documented as
/// reporting the primary display and therefore clamps a panel on any other
/// monitor against the wrong rectangle.
unsafe fn work_area_for(r: RECT) -> place::Bounds {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let monitor = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(monitor, &mut info).as_bool() {
        place::Bounds::from_rect(info.rcWork)
    } else {
        // No monitor information at all. The icon's own rectangle is a poor
        // work area but a finite one, and it keeps the panel on screen.
        place::Bounds::from_rect(r)
    }
}

/// Places the panel just above the tray icon, clamped to the work area of the
/// monitor the icon is on.
///
/// Asks the shell where the icon actually is rather than assuming the pointer is
/// over it. The cursor is only a fallback: it happens to be right when the user
/// clicked the icon, and wrong for every other way the panel can be opened —
/// which is exactly how a panel ends up in the middle of the screen.
pub unsafe fn anchor(owner: HWND, icon_id: u32, w: i32, h: i32) -> (i32, i32) {
    use windows::Win32::UI::Shell::{Shell_NotifyIconGetRect, NOTIFYICONIDENTIFIER};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let ident = NOTIFYICONIDENTIFIER {
        cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
        hWnd: owner,
        uID: icon_id,
        ..Default::default()
    };
    let icon = match Shell_NotifyIconGetRect(&ident) {
        Ok(r) => r,
        Err(_) => {
            // The icon may be hidden in the overflow flyout, where the shell
            // reports no rectangle. The cursor is the best remaining guess, as
            // a zero-sized rectangle at that point.
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            RECT {
                left: pt.x,
                top: pt.y,
                right: pt.x,
                bottom: pt.y,
            }
        }
    };
    let work = work_area_for(icon);
    place::above_icon(place::Bounds::from_rect(icon), work, w, h, 8)
}

/// Repositions an already-placed panel for a new height, holding its **bottom**
/// edge still.
///
/// The panel is anchored above the tray icon, so its bottom edge is the one the
/// user's eye is on and the one the taskbar constrains. Holding the top instead
/// makes a panel that changes height — switching to Settings, the Synapse
/// banner appearing, the noise section — grow downward over the taskbar and
/// shrink away from it, which reads as the panel sliding around on its own.
///
/// Clamped to the work area so a panel taller than the space above the icon
/// runs off neither end.
pub unsafe fn reanchor_bottom(hwnd: HWND, h: i32) -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut r = RECT::default();
    let _ = GetWindowRect(hwnd, &mut r);
    let work = work_area_for(r);
    place::hold_bottom(place::Bounds::from_rect(r), work, h)
}

/// Stashes a pointer on the window so the shared wndproc can tell which window
/// a message belongs to without a second thread-local.
pub unsafe fn set_tag(hwnd: HWND, tag: isize) {
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, tag);
}

pub unsafe fn tag(hwnd: HWND) -> isize {
    GetWindowLongPtrW(hwnd, GWLP_USERDATA)
}

/// Marker stored in `GWLP_USERDATA` for the panel window.
pub const TAG_PANEL: isize = 0x5041_4E4C;

/// Extracts a click position from an `LPARAM`, in client coordinates.
pub fn point_from_lparam(lp: LPARAM) -> (f32, f32) {
    let x = (lp.0 & 0xFFFF) as i16 as f32;
    let y = ((lp.0 >> 16) & 0xFFFF) as i16 as f32;
    (x, y)
}

/// Whether a `WPARAM` from `WM_MOUSEMOVE` has the left button held.
pub fn left_button_down(wp: WPARAM) -> bool {
    wp.0 & 0x0001 != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lparam_decodes_negative_coordinates() {
        // Dragging above or left of the window yields negatives, which a naive
        // mask reads as ~65000 and sends the knob to the wrong end.
        let lp = LPARAM(((-5i32 as u32 as isize) << 16) | (10u32 as isize & 0xFFFF));
        let (x, y) = point_from_lparam(lp);
        assert_eq!(x, 10.0);
        assert_eq!(y, -5.0);
    }

    #[test]
    fn left_button_flag_is_read_from_the_low_bit() {
        assert!(left_button_down(WPARAM(0x0001)));
        assert!(!left_button_down(WPARAM(0x0002)));
    }
}
