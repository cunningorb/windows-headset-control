//! Palette and metrics.
//!
//! Every value here was sampled by pixel from the mockups in
//! `Documents\ShareX\Screenshots\2026-08\opera_*.png`, not estimated. Text
//! colours are peak luminance within a glyph run, because averaging over
//! anti-aliased text reads far too dark.
//!
//! This is the only place appearance is decided. `layout` derives every position
//! from these constants and `render` reads colours from nowhere else, so a visual
//! change is an edit here rather than a hunt through drawing code.

/// Straight (non-premultiplied) RGBA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl Color {
    pub const fn rgb(hex: u32) -> Color {
        Color(
            ((hex >> 16) & 0xFF) as u8,
            ((hex >> 8) & 0xFF) as u8,
            (hex & 0xFF) as u8,
            0xFF,
        )
    }

    pub const fn with_alpha(self, a: u8) -> Color {
        Color(self.0, self.1, self.2, a)
    }

    pub fn as_f32(self) -> (f32, f32, f32, f32) {
        (
            self.0 as f32 / 255.0,
            self.1 as f32 / 255.0,
            self.2 as f32 / 255.0,
            self.3 as f32 / 255.0,
        )
    }
}

// ---------------------------------------------------------------- palette ---

pub const BG_PANEL: Color = Color::rgb(0x131623);
pub const BG_CARD: Color = Color::rgb(0x1A1D29);
pub const BG_BANNER: Color = Color::rgb(0x1D1E31);
pub const BG_BUTTON: Color = Color::rgb(0x292B37);

pub const BORDER_CARD: Color = Color::rgb(0x272935);
pub const BORDER_BANNER: Color = Color::rgb(0x2D2E41);

pub const TRACK_INACTIVE: Color = Color::rgb(0x2D2F3B);

pub const ACCENT: Color = Color::rgb(0x9184D9);
pub const ACCENT_TEXT: Color = Color::rgb(0xD2CEFD);
pub const ACCENT_LABEL: Color = Color::rgb(0xCECAF9);

pub const TEXT_PRIMARY: Color = Color::rgb(0xE9E9ED);
pub const TEXT_SECONDARY: Color = Color::rgb(0x8E91A6);
pub const TEXT_MUTED: Color = Color::rgb(0xB0B3C7);

pub const STATE_LIVE: Color = Color::rgb(0x4ECB89);
pub const STATE_MUTED: Color = Color::rgb(0xE0555F);

/// Everything is drawn dimmed at this opacity when the headset is unreachable.
pub const DISABLED_ALPHA: u8 = 0x66;

// ---------------------------------------------------------------- metrics ---

/// Panel width, measured from the mockup (342 px of panel inside an 8 px shadow).
pub const PANEL_W: f32 = 342.0;
/// Shadow margin on every side. The window is this much larger than the panel.
pub const SHADOW: f32 = 8.0;
/// Inset from the panel edge to card content.
pub const MARGIN: f32 = 17.0;
/// Card width: PANEL_W - 2 * MARGIN.
pub const CONTENT_W: f32 = PANEL_W - 2.0 * MARGIN;

pub const PANEL_RADIUS: f32 = 14.0;
pub const CARD_RADIUS: f32 = 10.0;
pub const BUTTON_RADIUS: f32 = 8.0;

/// Status card height, measured y 87..156.
pub const CARD_H: f32 = 70.0;
/// Warning banner height, measured y 397..445.
pub const BANNER_H: f32 = 49.0;
/// Switcher button, measured 125 x 36.
pub const SWITCHER_W: f32 = 125.0;
pub const SWITCHER_H: f32 = 36.0;

/// Noise-mode segment row. Not in the mockups — they predate the parameter
/// being identified — so this matches the switcher's height rather than being
/// sampled, which is the only honest thing to say about it.
pub const SEGMENT_H: f32 = 36.0;

/// Slider dots, measured ~5 px across.
pub const DOT_R: f32 = 2.5;
/// Knob radius, measured ~5 px, plus a soft glow beyond it.
pub const KNOB_R: f32 = 6.5;
pub const KNOB_GLOW_R: f32 = 12.0;
pub const TRACK_LINE_W: f32 = 2.0;

// Vertical rhythm, derived from the mockup's measured bands.
pub const HEADER_TOP: f32 = 22.0;
pub const HEADER_H: f32 = 58.0;
pub const GAP: f32 = 16.0;
pub const FOOTER_H: f32 = 44.0;

// ------------------------------------------------------------------ fonts ---

pub const FONT_FAMILY: &str = "Segoe UI";

/// Point sizes and weights, in the order they appear in the panel.
pub const FS_TITLE: f32 = 15.0;
pub const FS_SUBTITLE: f32 = 10.0;
pub const FS_BATTERY: f32 = 20.0;
pub const FS_CAPTION: f32 = 9.5;
pub const FS_BODY: f32 = 12.0;
pub const FS_PILL: f32 = 11.0;
pub const FS_END_LABEL: f32 = 9.5;
/// Settings row descriptions. Sized so the longer of the two fits on one line;
/// they are vertically centred, so a wrap grows upward into the title.
pub const FS_DESCRIPTION: f32 = 8.5;

pub const W_REGULAR: u32 = 400;
pub const W_SEMIBOLD: u32 = 600;
pub const W_BOLD: u32 = 700;

/// Not measured. Nothing in the vendor protocol reports the wireless link type,
/// so this is a stated assumption rather than a reading. See
/// `docs/device-research.md`.
pub const LINK_TYPE_LABEL: &str = "2.4 GHZ";

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the palette to the values sampled from the mockups. An accidental
    /// edit fails here; an intentional one updates both sides together.
    #[test]
    fn palette_matches_the_sampled_mockups() {
        let pairs: [(Color, u32, &str); 15] = [
            (BG_PANEL, 0x131623, "panel"),
            (BG_CARD, 0x1A1D29, "card"),
            (BG_BANNER, 0x1D1E31, "banner"),
            (BG_BUTTON, 0x292B37, "button"),
            (BORDER_CARD, 0x272935, "card border"),
            (BORDER_BANNER, 0x2D2E41, "banner border"),
            (TRACK_INACTIVE, 0x2D2F3B, "track"),
            (ACCENT, 0x9184D9, "accent"),
            (ACCENT_TEXT, 0xD2CEFD, "accent text"),
            (ACCENT_LABEL, 0xCECAF9, "accent label"),
            (TEXT_PRIMARY, 0xE9E9ED, "text primary"),
            (TEXT_SECONDARY, 0x8E91A6, "text secondary"),
            (TEXT_MUTED, 0xB0B3C7, "text muted"),
            (STATE_LIVE, 0x4ECB89, "live"),
            (STATE_MUTED, 0xE0555F, "muted"),
        ];
        for (c, hex, name) in pairs {
            assert_eq!(c, Color::rgb(hex), "{name} drifted from the mockup");
            assert_eq!(c.3, 0xFF, "{name} must be opaque");
        }
    }

    #[test]
    fn content_width_matches_the_measured_card_width() {
        // Cards measured x 25..332 inside a panel starting at x 8: 308 px wide.
        assert_eq!(CONTENT_W, 308.0);
    }

    #[test]
    fn colour_conversion_is_normalised() {
        let (r, g, b, a) = Color::rgb(0xFFFFFF).as_f32();
        assert!((r - 1.0).abs() < f32::EPSILON);
        assert!((g - 1.0).abs() < f32::EPSILON);
        assert!((b - 1.0).abs() < f32::EPSILON);
        assert!((a - 1.0).abs() < f32::EPSILON);
    }
}
