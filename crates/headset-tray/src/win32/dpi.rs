//! Process DPI awareness, and the one piece of arithmetic that follows from it.
//!
//! The panel is a layered window built from a bitmap, so DPI is not something
//! Windows can scale for us without making it blurry: the bitmap has to be
//! rendered at the display's real pixel density. `ui::render` has always
//! accepted a scale; until this module existed, nothing ever passed one.

use windows::Win32::Foundation::HWND;

/// Windows' reference density. Every DPI is expressed relative to this.
const BASE_DPI: f32 = 96.0;

/// Render scale for a display DPI. 96 is 100%, 144 is 150%.
pub fn scale_for_dpi(dpi: u32) -> f32 {
    if dpi == 0 {
        return 1.0;
    }
    dpi as f32 / BASE_DPI
}

/// Opts the process into per-monitor DPI awareness.
///
/// Must run before any window exists: the awareness context is fixed for the
/// process at first use, and a window created beforehand keeps the old one.
///
/// Failure is not fatal. An older Windows without this entry point leaves the
/// process DPI-unaware, which is exactly the behaviour that shipped before, so
/// the tray degrades to a blurry panel rather than not starting.
pub unsafe fn make_process_per_monitor_aware() {
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    if let Err(e) = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
        tracing::warn!("per-monitor DPI awareness unavailable, panel may be blurry: {e}");
    }
}

/// The render scale for the display `hwnd` is currently on.
pub unsafe fn window_scale(hwnd: HWND) -> f32 {
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    scale_for_dpi(GetDpiForWindow(hwnd))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_windows_scaling_steps_convert_exactly() {
        // The percentages Windows offers in Display settings.
        assert_eq!(scale_for_dpi(96), 1.0, "100%");
        assert_eq!(scale_for_dpi(120), 1.25, "125%");
        assert_eq!(scale_for_dpi(144), 1.5, "150%");
        assert_eq!(scale_for_dpi(168), 1.75, "175%");
        assert_eq!(scale_for_dpi(192), 2.0, "200%");
    }

    #[test]
    fn an_implausible_dpi_never_produces_a_zero_or_negative_scale() {
        // GetDpiForWindow returns 0 on failure. A zero scale would render a
        // zero-sized bitmap, and UpdateLayeredWindow would fail on every
        // repaint -- an invisible panel rather than a wrong-sized one.
        assert_eq!(scale_for_dpi(0), 1.0, "a failed query falls back to 100%");
    }
}
