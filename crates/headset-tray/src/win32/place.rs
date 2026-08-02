//! Pure placement geometry. No OS calls, no `unsafe`, no window handles.
//!
//! This exists because the panel's position was being computed inside the same
//! `unsafe` block that queried the OS for it, which made a real multi-monitor
//! bug untestable. The OS lookups stay in `panel.rs`; the arithmetic lives here
//! where it can be pinned down.

use windows::Win32::Foundation::RECT;

/// A rectangle in physical screen pixels.
///
/// Screen coordinates are signed: a monitor to the left of the primary has
/// negative `left`, which is exactly the case a work-area clamp gets wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Bounds {
    pub fn from_rect(r: RECT) -> Bounds {
        Bounds {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

/// Top-left for a `w` x `h` panel sitting just above `icon`, clamped to `work`.
pub fn above_icon(icon: Bounds, work: Bounds, w: i32, h: i32, margin: i32) -> (i32, i32) {
    let centre_x = (icon.left + icon.right) / 2;
    let mut x = centre_x - w / 2;
    if x + w > work.right {
        x = work.right - w;
    }
    if x < work.left {
        x = work.left;
    }

    let mut y = icon.top - h - margin;
    if y < work.top {
        // Taskbar at the top of the screen: there is no room above the icon,
        // so drop below it rather than off-screen.
        y = icon.bottom + margin;
    }
    if y + h > work.bottom {
        y = work.bottom - h;
    }
    if y < work.top {
        // A panel taller than the work area cannot satisfy both clamps. Prefer
        // the top edge: the header identifies the device, and losing the footer
        // is less confusing than losing the title.
        y = work.top;
    }
    (x, y)
}

/// Top-left for a panel whose height changed to `h`, holding its bottom edge.
pub fn hold_bottom(current: Bounds, work: Bounds, h: i32) -> (i32, i32) {
    let mut y = current.bottom - h;
    if y + h > work.bottom {
        y = work.bottom - h;
    }
    if y < work.top {
        y = work.top;
    }
    (current.left, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1920x1080 primary, taskbar 48 px tall at the bottom.
    fn primary() -> Bounds {
        Bounds {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1032,
        }
    }

    /// A tray icon near the right-hand end of that taskbar.
    fn icon() -> Bounds {
        Bounds {
            left: 1700,
            top: 1032,
            right: 1724,
            bottom: 1056,
        }
    }

    #[test]
    fn the_panel_sits_above_the_icon_and_centred_on_it() {
        let (x, y) = above_icon(icon(), primary(), 342, 500, 8);
        assert_eq!(x, 1712 - 342 / 2, "centred on the icon");
        assert_eq!(y, 1032 - 500 - 8, "bottom edge one margin above the icon");
    }

    #[test]
    fn a_panel_running_off_the_right_edge_is_pulled_back_in() {
        let narrow = Bounds {
            left: 1890,
            top: 1032,
            right: 1914,
            bottom: 1056,
        };
        let (x, _) = above_icon(narrow, primary(), 342, 500, 8);
        assert_eq!(x, 1920 - 342, "flush with the work-area right edge");
    }

    #[test]
    fn a_secondary_monitor_to_the_left_keeps_its_negative_coordinates() {
        // The bug this whole module exists for: SPI_GETWORKAREA reports the
        // primary monitor, so a panel on a left-hand secondary was clamped to
        // x >= 0 and jumped across to the primary display.
        let left_monitor = Bounds {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1032,
        };
        let left_icon = Bounds {
            left: -220,
            top: 1032,
            right: -196,
            bottom: 1056,
        };
        let (x, y) = above_icon(left_icon, left_monitor, 342, 500, 8);
        assert!(
            x < 0,
            "panel must stay on the left-hand monitor, got x = {x}"
        );
        assert!(x >= -1920, "and inside it, got x = {x}");
        assert_eq!(y, 1032 - 500 - 8);
    }

    #[test]
    fn a_taskbar_at_the_top_puts_the_panel_below_the_icon() {
        let work = Bounds {
            left: 0,
            top: 48,
            right: 1920,
            bottom: 1080,
        };
        let top_icon = Bounds {
            left: 1700,
            top: 24,
            right: 1724,
            bottom: 48,
        };
        let (_, y) = above_icon(top_icon, work, 342, 500, 8);
        assert_eq!(y, 48 + 8, "dropped below the icon rather than off-screen");
    }

    #[test]
    fn a_panel_taller_than_the_work_area_starts_at_its_top() {
        let (_, y) = above_icon(icon(), primary(), 342, 4000, 8);
        assert_eq!(y, 0, "clamped to the work-area top, never above it");
    }

    #[test]
    fn holding_the_bottom_moves_the_top_when_the_height_changes() {
        let current = Bounds {
            left: 1541,
            top: 524,
            right: 1883,
            bottom: 1024,
        };
        let (x, y) = hold_bottom(current, primary(), 620);
        assert_eq!(x, 1541, "horizontal position is untouched");
        assert_eq!(y, 1024 - 620, "grew upward, bottom edge still at 1024");
    }

    #[test]
    fn holding_the_bottom_still_respects_the_work_area() {
        let current = Bounds {
            left: 100,
            top: 900,
            right: 442,
            bottom: 1024,
        };
        let (_, y) = hold_bottom(current, primary(), 4000);
        assert_eq!(y, 0, "a panel taller than the screen starts at the top");
    }
}
