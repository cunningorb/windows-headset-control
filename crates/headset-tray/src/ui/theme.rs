//! Palette and metrics.
//!
//! Every value here was sampled by pixel from the design mockups (not
//! committed to this repository), not estimated. Text
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

// --------------------------------------------------------------- palettes ---

/// Every colour the panel draws with, chosen for the current accessibility
/// settings. `layout` reads colours through the accessors below rather than
/// from the constants, so a second palette needs no changes at the call sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg_panel: Color,
    pub bg_card: Color,
    pub bg_banner: Color,
    pub bg_button: Color,
    pub border_card: Color,
    pub border_banner: Color,
    pub track_inactive: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub accent_label: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub state_live: Color,
    pub state_muted: Color,
    /// The toggle knob. Its own role because it must read against an accent
    /// track in every palette; it used to borrow `text_primary`, which is
    /// near-white in the dark palette by coincidence and near-black in the
    /// light one, where the knob vanished into the track.
    pub toggle_knob: Color,
}

/// What the user asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    /// Follow Windows. The default a fresh install gets.
    System,
    Light,
    Dark,
}

/// What will actually be drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Light,
    Dark,
    HighContrast,
}

/// Decides the palette from the three inputs that can influence it.
///
/// Pure, so the precedence is testable without a registry or a window. High
/// contrast wins over everything: a user who turned it on did not mean "unless
/// the app has a light theme".
pub fn resolve(appearance: Appearance, windows_prefers_light: bool, high_contrast: bool) -> Which {
    if high_contrast {
        return Which::HighContrast;
    }
    match appearance {
        Appearance::Light => Which::Light,
        Appearance::Dark => Which::Dark,
        Appearance::System => {
            if windows_prefers_light {
                Which::Light
            } else {
                Which::Dark
            }
        }
    }
}

/// The resolved palette. Set at startup and whenever Windows reports a settings
/// change. An atomic rather than a lock: read on every primitive during a paint
/// and written approximately never.
static RESOLVED: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(DARK);

const LIGHT: u8 = 0;
const DARK: u8 = 1;
const HIGH_CONTRAST: u8 = 2;

pub fn set_palette(which: Which) {
    let v = match which {
        Which::Light => LIGHT,
        Which::Dark => DARK,
        Which::HighContrast => HIGH_CONTRAST,
    };
    RESOLVED.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// Kept so the existing call sites and tests read naturally.
pub fn set_high_contrast(on: bool) {
    set_palette(if on { Which::HighContrast } else { Which::Dark });
}

pub fn palette() -> Palette {
    match RESOLVED.load(std::sync::atomic::Ordering::Relaxed) {
        LIGHT => light_palette(),
        HIGH_CONTRAST => high_contrast_palette(),
        _ => sampled_palette(),
    }
}

/// The mockup palette, unchanged. Every value is the constant above it.
fn sampled_palette() -> Palette {
    Palette {
        bg_panel: BG_PANEL,
        bg_card: BG_CARD,
        bg_banner: BG_BANNER,
        bg_button: BG_BUTTON,
        border_card: BORDER_CARD,
        border_banner: BORDER_BANNER,
        track_inactive: TRACK_INACTIVE,
        accent: ACCENT,
        accent_text: ACCENT_TEXT,
        accent_label: ACCENT_LABEL,
        text_primary: TEXT_PRIMARY,
        text_secondary: TEXT_SECONDARY,
        text_muted: TEXT_MUTED,
        state_live: STATE_LIVE,
        state_muted: STATE_MUTED,
        toggle_knob: TEXT_PRIMARY,
    }
}

// --------------------------------------------------------- light palette ---
//
// Sampled from the light mockups by locating each feature and reading it, not
// by reading fixed coordinates: those images are 363x457 while the dark ones
// are 358x521, so positions do not carry over. Guessed coordinates were tried
// first and returned background for four of six values.
//
// **Text is sampled as the darkest pixel within the glyph run**, which is the
// inverse of the rule at the top of this file. That rule takes peak luminance
// because light ink anti-aliases toward a dark background; here dark ink
// anti-aliases toward a light one, and averaging reads too light. Following the
// original rule unchanged would have sampled near-white and produced invisible
// text that still passes a naive contrast check.

const L_BG_PANEL: Color = Color::rgb(0xF0F0F5);
const L_BG_CARD: Color = Color::rgb(0xE8E8EE);
const L_BORDER_CARD: Color = Color::rgb(0xD2D2D8);
const L_BG_BUTTON: Color = Color::rgb(0xDFDDED);
const L_TRACK_INACTIVE: Color = Color::rgb(0x9B9CA6);
const L_TEXT_PRIMARY: Color = Color::rgb(0x23252F);
const L_TEXT_SECONDARY: Color = Color::rgb(0x6E7283);
const L_TEXT_MUTED: Color = Color::rgb(0x727687);
const L_ACCENT: Color = Color::rgb(0x6153B8);
/// The value text is the same purple as the accent here. In the dark palette it
/// is a lighter tint; on a light background it is not.
const L_ACCENT_TEXT: Color = Color::rgb(0x6153B8);
const L_STATE_LIVE: Color = Color::rgb(0x389669);

// DERIVED, not measured — these three appear in neither mockup.
//
// No end label is highlighted in the mockup (sidetone sits at 14 of 15), so the
// active-label colour could not be read. The dark palette keeps it within a few
// units of the value text, so this follows.
const L_ACCENT_LABEL: Color = Color::rgb(0x6153B8);
// Neither mockup shows the Synapse warning or a muted microphone. The banner is
// deliberately almost indistinguishable from a card, because in the sampled dark
// palette it is: #1D1E31 against #1A1D29. The warning is carried by the glyph and
// the wording, not by colour, so inventing an amber here would be inventing a
// design decision nobody made.
const L_BG_BANNER: Color = Color::rgb(0xE6E5EF);
const L_BORDER_BANNER: Color = Color::rgb(0xCFCEDD);
const L_STATE_MUTED: Color = Color::rgb(0xC62B36);

/// The light palette. See above for which values were sampled and which were
/// derived; the distinction matters because this file opens by claiming
/// everything in it was measured.
pub fn light_palette() -> Palette {
    Palette {
        bg_panel: L_BG_PANEL,
        bg_card: L_BG_CARD,
        bg_banner: L_BG_BANNER,
        bg_button: L_BG_BUTTON,
        border_card: L_BORDER_CARD,
        border_banner: L_BORDER_BANNER,
        track_inactive: L_TRACK_INACTIVE,
        accent: L_ACCENT,
        accent_text: L_ACCENT_TEXT,
        accent_label: L_ACCENT_LABEL,
        text_primary: L_TEXT_PRIMARY,
        text_secondary: L_TEXT_SECONDARY,
        text_muted: L_TEXT_MUTED,
        state_live: L_STATE_LIVE,
        state_muted: L_STATE_MUTED,
        toggle_knob: Color::rgb(0xFFFFFF),
    }
}

/// Maximum-separation palette for Windows high contrast.
///
/// Deliberately not a tinted version of the mockup palette: high contrast is
/// chosen by users who cannot read low-contrast greys, so every surface is
/// black and every foreground is at full luminance. The distinctions the
/// mockup palette draws with subtle shade differences are drawn with hue here.
pub fn high_contrast_palette() -> Palette {
    const BLACK: Color = Color::rgb(0x000000);
    const WHITE: Color = Color::rgb(0xFFFFFF);
    const YELLOW: Color = Color::rgb(0xFFFF00);
    const CYAN: Color = Color::rgb(0x00FFFF);
    Palette {
        bg_panel: BLACK,
        bg_card: BLACK,
        bg_banner: BLACK,
        bg_button: BLACK,
        border_card: WHITE,
        border_banner: WHITE,
        track_inactive: Color::rgb(0x3F3F3F),
        accent: YELLOW,
        accent_text: YELLOW,
        accent_label: YELLOW,
        text_primary: WHITE,
        text_secondary: WHITE,
        text_muted: WHITE,
        state_live: CYAN,
        state_muted: YELLOW,
        toggle_knob: WHITE,
    }
}

// Accessors. `layout` calls these rather than naming the constants, so the
// palette in force is decided in one place. The constants stay as the record
// of what was sampled from the mockups.
macro_rules! palette_accessors {
    ($($name:ident),* $(,)?) => {
        $(pub fn $name() -> Color { palette().$name })*
    };
}
palette_accessors!(
    bg_panel,
    bg_card,
    bg_banner,
    bg_button,
    border_card,
    border_banner,
    track_inactive,
    accent,
    accent_text,
    accent_label,
    text_primary,
    text_secondary,
    text_muted,
    state_live,
    state_muted,
    toggle_knob,
);

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
    fn the_default_palette_is_the_sampled_one() {
        set_high_contrast(false);
        let p = palette();
        assert_eq!(p.bg_panel, BG_PANEL);
        assert_eq!(p.accent, ACCENT);
        assert_eq!(p.text_primary, TEXT_PRIMARY);
    }

    #[test]
    fn high_contrast_separates_every_pair_that_sits_on_top_of_another() {
        // The failure being prevented is a palette that "supports" high
        // contrast by swapping a few colours and leaving text on a background
        // it cannot be read against. Every foreground must clear a real
        // luminance gap from the surface it is drawn on.
        set_high_contrast(true);
        let p = palette();

        let luminance =
            |c: Color| 0.2126 * (c.0 as f32) + 0.7152 * (c.1 as f32) + 0.0722 * (c.2 as f32);
        let pairs: [(Color, Color, &str); 5] = [
            (p.text_primary, p.bg_panel, "title on panel"),
            (p.text_secondary, p.bg_panel, "caption on panel"),
            (p.text_primary, p.bg_card, "battery on card"),
            (p.accent_text, p.bg_panel, "value on panel"),
            (p.accent, p.track_inactive, "filled track on unfilled"),
        ];
        for (fg, bg, what) in pairs {
            let gap = (luminance(fg) - luminance(bg)).abs();
            assert!(gap > 128.0, "{what}: luminance gap only {gap:.0}");
        }
        set_high_contrast(false);
    }

    #[test]
    fn high_contrast_beats_every_appearance_choice() {
        for a in [Appearance::System, Appearance::Light, Appearance::Dark] {
            for win_light in [true, false] {
                assert_eq!(
                    resolve(a, win_light, true),
                    Which::HighContrast,
                    "{a:?} with windows_light={win_light} should still yield high contrast"
                );
            }
        }
    }

    #[test]
    fn an_override_beats_the_windows_preference() {
        for win_light in [true, false] {
            assert_eq!(resolve(Appearance::Light, win_light, false), Which::Light);
            assert_eq!(resolve(Appearance::Dark, win_light, false), Which::Dark);
        }
    }

    #[test]
    fn system_follows_windows() {
        assert_eq!(resolve(Appearance::System, true, false), Which::Light);
        assert_eq!(resolve(Appearance::System, false, false), Which::Dark);
    }

    #[test]
    fn the_resolved_palette_is_the_one_that_gets_drawn() {
        set_palette(Which::Light);
        assert_eq!(palette().bg_panel, L_BG_PANEL);
        set_palette(Which::HighContrast);
        assert_eq!(palette().bg_panel, high_contrast_palette().bg_panel);
        set_palette(Which::Dark);
        assert_eq!(palette().bg_panel, BG_PANEL);
    }

    #[test]
    fn the_light_palette_is_light_and_keeps_its_contrast() {
        let p = light_palette();
        let luminance =
            |c: Color| 0.2126 * (c.0 as f32) + 0.7152 * (c.1 as f32) + 0.0722 * (c.2 as f32);

        // Light surfaces, dark ink: the inverse of the sampled palette.
        assert!(luminance(p.bg_panel) > 200.0, "the panel should be light");
        assert!(luminance(p.bg_card) > 190.0, "cards should be light");
        assert!(
            luminance(p.text_primary) < 80.0,
            "primary text should be dark"
        );

        // A light theme fails differently from a dark one: pale text on a pale
        // card is the easy mistake, and it is nearly invisible to whoever wrote
        // it. The threshold is 40 rather than the high-contrast palette's 128
        // because this is an ordinary theme, and text_secondary is meant to be
        // soft.
        let pairs: [(Color, Color, &str); 5] = [
            (p.text_primary, p.bg_panel, "title on panel"),
            (p.text_secondary, p.bg_panel, "caption on panel"),
            (p.text_primary, p.bg_card, "battery on card"),
            (p.accent_text, p.bg_panel, "value on panel"),
            (p.accent, p.track_inactive, "filled track on unfilled"),
        ];
        for (fg, bg, what) in pairs {
            let gap = (luminance(fg) - luminance(bg)).abs();
            assert!(gap > 40.0, "{what}: luminance gap only {gap:.0}");
        }
    }

    #[test]
    fn the_sampled_dark_palette_is_untouched() {
        // Adding a theme must not edit the one that was measured.
        set_high_contrast(false);
        assert_eq!(BG_PANEL, Color::rgb(0x131623));
        assert_eq!(ACCENT, Color::rgb(0x9184D9));
        assert_eq!(palette().bg_panel, BG_PANEL);
    }

    #[test]
    fn high_contrast_does_not_disturb_the_sampled_palette() {
        // The constants are still the record of what was measured from the
        // mockups; high contrast selects a different palette rather than
        // editing them.
        set_high_contrast(true);
        assert_eq!(BG_PANEL, Color::rgb(0x131623));
        set_high_contrast(false);
        assert_eq!(palette().bg_panel, BG_PANEL);
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
