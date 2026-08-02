//! Panel layout and hit-testing. Pure: no OS access, no device, no window.
//!
//! Everything that decides *where* something is lives here, so tick spacing,
//! value-to-position mapping and its inverse, and every clickable region can be
//! tested without a GPU. `render` walks the primitive list and decides nothing.

use crate::state::HeadsetState;
use crate::ui::theme::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center_y(&self) -> f32 {
        self.y + self.h / 2.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    /// Filled and/or stroked rounded rectangle.
    RoundRect {
        rect: Rect,
        radius: f32,
        fill: Option<Color>,
        stroke: Option<Color>,
        stroke_w: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        fill: Option<Color>,
        stroke: Option<Color>,
        stroke_w: f32,
    },
    /// A soft radial fade, used for the knob glow.
    Glow {
        cx: f32,
        cy: f32,
        r: f32,
        color: Color,
    },
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        w: f32,
        color: Color,
    },
    /// Open or closed polyline. Icons are built from these.
    Path {
        points: Vec<(f32, f32)>,
        closed: bool,
        fill: Option<Color>,
        stroke: Option<Color>,
        stroke_w: f32,
    },
    Text {
        rect: Rect,
        text: String,
        size: f32,
        weight: u32,
        color: Color,
        align: Align,
        /// Extra spacing between characters, for the small tracked-out captions.
        tracking: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitTarget {
    Gear,
    Back,
    MutePill,
    Switcher,
    SliderTrack,
    Refresh,
    ToggleStartup,
    ToggleWarning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Main,
    Settings,
}

/// Which parameter the single slider is currently driving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderParam {
    Sidetone,
    GameChat,
}

impl SliderParam {
    /// Inclusive maximum. One tick is drawn per selectable value, which is why
    /// the game/chat track reads denser than the sidetone track.
    pub fn max(self) -> u8 {
        match self {
            SliderParam::Sidetone => 15,
            SliderParam::GameChat => 20,
        }
    }
    pub fn ticks(self) -> usize {
        self.max() as usize + 1
    }
    pub fn button_label(self) -> &'static str {
        match self {
            SliderParam::Sidetone => "Sidetone",
            SliderParam::GameChat => "Game / Chat",
        }
    }
    pub fn end_labels(self) -> [&'static str; 3] {
        match self {
            SliderParam::Sidetone => ["OFF", "", "MAX"],
            SliderParam::GameChat => ["CHAT", "BALANCED", "GAME"],
        }
    }
    pub fn other(self) -> SliderParam {
        match self {
            SliderParam::Sidetone => SliderParam::GameChat,
            SliderParam::GameChat => SliderParam::Sidetone,
        }
    }
}

/// The value text shown to the right of the switcher, exactly as the mockups
/// render it.
pub fn format_value(param: SliderParam, value: Option<u8>) -> String {
    let Some(v) = value else {
        return "--".to_string();
    };
    match param {
        SliderParam::Sidetone => {
            if v == 0 {
                "Off".to_string()
            } else {
                v.to_string()
            }
        }
        SliderParam::GameChat => match v.cmp(&10) {
            std::cmp::Ordering::Equal => "Balanced".to_string(),
            std::cmp::Ordering::Greater => format!("Game +{}", v - 10),
            std::cmp::Ordering::Less => format!("Chat +{}", 10 - v),
        },
    }
}

/// Which of the three end labels is highlighted, if any.
fn active_end_label(param: SliderParam, value: Option<u8>) -> Option<usize> {
    let v = value?;
    Some(match param {
        SliderParam::Sidetone => {
            if v == 0 {
                0
            } else if v == param.max() {
                2
            } else {
                return None;
            }
        }
        SliderParam::GameChat => match v.cmp(&10) {
            std::cmp::Ordering::Equal => 1,
            std::cmp::Ordering::Less if v == 0 => 0,
            std::cmp::Ordering::Greater if v == param.max() => 2,
            _ => return None,
        },
    })
}

pub struct Panel {
    pub primitives: Vec<Primitive>,
    pub hits: Vec<(Rect, HitTarget)>,
    /// Panel height, excluding the shadow margin.
    pub height: f32,
    /// Track geometry, retained so drag handling can map x back to a value.
    pub track: Option<TrackGeometry>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackGeometry {
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
    pub param: SliderParam,
}

impl TrackGeometry {
    pub fn x_for(&self, value: u8) -> f32 {
        let max = self.param.max() as f32;
        self.x0 + (self.x1 - self.x0) * (value as f32 / max)
    }
    /// Nearest tick to a pointer position, clamped to the ends.
    pub fn value_at(&self, px: f32) -> u8 {
        let max = self.param.max() as f32;
        let t = ((px - self.x0) / (self.x1 - self.x0)).clamp(0.0, 1.0);
        (t * max).round() as u8
    }
}

impl Panel {
    pub fn hit(&self, px: f32, py: f32) -> Option<HitTarget> {
        self.hits
            .iter()
            .find(|(r, _)| r.contains(px, py))
            .map(|(_, t)| *t)
    }
}

struct Builder {
    p: Vec<Primitive>,
    hits: Vec<(Rect, HitTarget)>,
    dim: bool,
}

impl Builder {
    fn tint(&self, c: Color) -> Color {
        if self.dim {
            c.with_alpha(DISABLED_ALPHA)
        } else {
            c
        }
    }
    fn text(&mut self, rect: Rect, s: &str, size: f32, weight: u32, color: Color, align: Align) {
        self.p.push(Primitive::Text {
            rect,
            text: s.to_string(),
            size,
            weight,
            color,
            align,
            tracking: 0.0,
        });
    }
    fn caption(&mut self, rect: Rect, s: &str, color: Color, align: Align) {
        self.p.push(Primitive::Text {
            rect,
            text: s.to_string(),
            size: FS_CAPTION,
            weight: W_SEMIBOLD,
            color,
            align,
            tracking: 1.1,
        });
    }
    fn card(&mut self, rect: Rect, fill: Color, stroke: Color) {
        self.p.push(Primitive::RoundRect {
            rect,
            radius: CARD_RADIUS,
            fill: Some(fill),
            stroke: Some(stroke),
            stroke_w: 1.0,
        });
    }
}

/// Builds the panel for a given state.
pub fn build(state: &HeadsetState, view: View, param: SliderParam, preview: Option<u8>) -> Panel {
    let mut b = Builder {
        p: Vec::new(),
        hits: Vec::new(),
        dim: state.connected == Some(false),
    };

    // Panel background is drawn first and sized at the end, once the content
    // height is known. A placeholder keeps its index stable.
    b.p.push(Primitive::RoundRect {
        rect: Rect::new(0.0, 0.0, PANEL_W, 0.0),
        radius: PANEL_RADIUS,
        fill: Some(BG_PANEL),
        stroke: None,
        stroke_w: 0.0,
    });

    let mut y = HEADER_TOP;
    header(&mut b, state, view, &mut y);

    let mut track = None;
    match view {
        View::Main => main_body(&mut b, state, param, preview, &mut y, &mut track),
        View::Settings => settings_body(&mut b, state, &mut y),
    }

    // Footer
    y += GAP * 0.5;
    let footer = Rect::new(MARGIN, y, CONTENT_W, FOOTER_H);
    refresh_icon(&mut b, MARGIN + 9.0, footer.center_y());
    b.text(
        Rect::new(MARGIN + 22.0, y, CONTENT_W - 22.0, FOOTER_H),
        "Refresh",
        FS_BODY,
        W_REGULAR,
        TEXT_MUTED,
        Align::Left,
    );
    b.hits.push((footer, HitTarget::Refresh));
    y = footer.bottom() + 6.0;

    let height = y;
    if let Some(Primitive::RoundRect { rect, .. }) = b.p.first_mut() {
        rect.h = height;
    }

    Panel {
        primitives: b.p,
        hits: b.hits,
        height,
        track,
    }
}

fn header(b: &mut Builder, state: &HeadsetState, view: View, y: &mut f32) {
    let connected = state.connected == Some(true);
    let dot_color = if connected { ACCENT } else { TEXT_SECONDARY };
    b.p.push(Primitive::Circle {
        cx: MARGIN + 4.0,
        cy: *y + 11.0,
        r: 4.0,
        fill: Some(dot_color),
        stroke: None,
        stroke_w: 0.0,
    });

    b.text(
        Rect::new(MARGIN + 16.0, *y, CONTENT_W - 60.0, 20.0),
        &state.device_name(),
        FS_TITLE,
        W_BOLD,
        TEXT_PRIMARY,
        Align::Left,
    );

    let status = match state.connected {
        Some(true) => format!("CONNECTED · {LINK_TYPE_LABEL}"),
        Some(false) => "DISCONNECTED".to_string(),
        None => "SEARCHING".to_string(),
    };
    b.caption(
        Rect::new(MARGIN + 16.0, *y + 22.0, CONTENT_W - 60.0, 14.0),
        &status,
        TEXT_SECONDARY,
        Align::Left,
    );

    // Gear / Back button, top right.
    let btn = Rect::new(MARGIN + CONTENT_W - 30.0, *y - 2.0, 30.0, 30.0);
    let active = view == View::Settings;
    b.p.push(Primitive::RoundRect {
        rect: btn,
        radius: BUTTON_RADIUS,
        fill: Some(if active {
            ACCENT.with_alpha(0x33)
        } else {
            BG_BUTTON
        }),
        stroke: Some(if active { ACCENT } else { BORDER_CARD }),
        stroke_w: 1.0,
    });
    gear_icon(
        b,
        btn.x + 15.0,
        btn.center_y(),
        if active { ACCENT } else { TEXT_MUTED },
    );
    b.hits.push((
        btn,
        if active {
            HitTarget::Back
        } else {
            HitTarget::Gear
        },
    ));

    *y += HEADER_H;
}

fn main_body(
    b: &mut Builder,
    state: &HeadsetState,
    param: SliderParam,
    preview: Option<u8>,
    y: &mut f32,
    track: &mut Option<TrackGeometry>,
) {
    // ---- status card -------------------------------------------------------
    let card = Rect::new(MARGIN, *y, CONTENT_W, CARD_H);
    b.card(card, BG_CARD, BORDER_CARD);

    let bat_y = card.y + 24.0;
    battery_icon(
        b,
        card.x + 18.0,
        bat_y + 4.0,
        state.battery,
        b.tint(TEXT_PRIMARY),
    );
    let pct = state
        .battery
        .map(|v| format!("{v}%"))
        .unwrap_or_else(|| "--".to_string());
    b.text(
        Rect::new(card.x + 52.0, bat_y - 10.0, 120.0, 26.0),
        &pct,
        FS_BATTERY,
        W_BOLD,
        b.tint(TEXT_PRIMARY),
        Align::Left,
    );
    b.caption(
        Rect::new(card.x + 53.0, bat_y + 16.0, 120.0, 14.0),
        "BATTERY",
        b.tint(TEXT_SECONDARY),
        Align::Left,
    );

    // ---- mute pill ---------------------------------------------------------
    let muted = state.effectively_muted();
    let (pill_text, glyph_color) = match muted {
        Some(true) => ("MUTED", STATE_MUTED),
        Some(false) => ("LIVE", STATE_LIVE),
        None => ("--", TEXT_SECONDARY),
    };
    // Wide enough for "MUTED" at this weight. Sized to the longest label rather
    // than the shortest: at 86 px it wrapped to "MUTE / D".
    let pill_w = 100.0;
    let pill = Rect::new(card.right() - pill_w - 14.0, card.y + 18.0, pill_w, 34.0);
    let hardware_locked = state.mic_mute_hardware == Some(true);
    b.p.push(Primitive::RoundRect {
        rect: pill,
        radius: BUTTON_RADIUS,
        fill: Some(BG_PANEL),
        stroke: Some(if muted == Some(true) {
            ACCENT
        } else {
            BORDER_CARD
        }),
        stroke_w: 1.0,
    });
    mic_icon(
        b,
        pill.x + 18.0,
        pill.center_y(),
        glyph_color,
        muted == Some(true),
    );
    b.text(
        Rect::new(pill.x + 32.0, pill.y, pill.w - 40.0, pill.h),
        pill_text,
        FS_PILL,
        W_SEMIBOLD,
        b.tint(TEXT_PRIMARY),
        Align::Left,
    );
    // A hardware-muted mic cannot be released from software, so the click is
    // not offered rather than offered and ignored.
    if !hardware_locked && !b.dim {
        b.hits.push((pill, HitTarget::MutePill));
    }

    *y = card.bottom() + GAP;

    if hardware_locked {
        b.text(
            Rect::new(MARGIN, *y - 6.0, CONTENT_W, 16.0),
            "Muted by the headset's own switch",
            FS_CAPTION,
            W_REGULAR,
            TEXT_SECONDARY,
            Align::Right,
        );
        *y += 12.0;
    }

    // ---- switcher + value --------------------------------------------------
    let sw = Rect::new(MARGIN, *y, SWITCHER_W, SWITCHER_H);
    b.p.push(Primitive::RoundRect {
        rect: sw,
        radius: BUTTON_RADIUS,
        fill: None,
        stroke: Some(b.tint(ACCENT)),
        stroke_w: 1.5,
    });
    swap_icon(b, sw.x + 16.0, sw.center_y(), b.tint(ACCENT));
    b.text(
        Rect::new(sw.x + 28.0, sw.y, sw.w - 34.0, sw.h),
        param.button_label(),
        FS_BODY,
        W_SEMIBOLD,
        b.tint(TEXT_PRIMARY),
        Align::Left,
    );
    if !b.dim {
        b.hits.push((sw, HitTarget::Switcher));
    }

    let value = preview.or(match param {
        SliderParam::Sidetone => state.sidetone,
        SliderParam::GameChat => state.game_chat,
    });
    b.text(
        Rect::new(MARGIN, sw.y, CONTENT_W, SWITCHER_H),
        &format_value(param, value),
        FS_BODY,
        W_SEMIBOLD,
        b.tint(ACCENT_TEXT),
        Align::Right,
    );

    *y = sw.bottom() + 22.0;

    // ---- slider ------------------------------------------------------------
    // Measured dot centres run x 26..332 in the mockup, which includes the 8 px
    // shadow margin; panel-local that is 18..324, one pixel inside the card
    // edges at 17 and 325.
    let tx0 = MARGIN + 1.0;
    let tx1 = MARGIN + CONTENT_W - 1.0;
    let ty = *y;
    let geom = TrackGeometry {
        x0: tx0,
        x1: tx1,
        y: ty,
        param,
    };

    let filled_to = value.map(|v| geom.x_for(v));
    b.p.push(Primitive::Line {
        x0: tx0,
        y0: ty,
        x1: tx1,
        y1: ty,
        w: TRACK_LINE_W,
        color: b.tint(TRACK_INACTIVE),
    });
    if let Some(fx) = filled_to {
        if fx > tx0 {
            b.p.push(Primitive::Line {
                x0: tx0,
                y0: ty,
                x1: fx,
                y1: ty,
                w: TRACK_LINE_W,
                color: b.tint(ACCENT),
            });
        }
    }
    for i in 0..param.ticks() {
        let x = geom.x_for(i as u8);
        let active = filled_to.map(|fx| x <= fx + 0.5).unwrap_or(false);
        b.p.push(Primitive::Circle {
            cx: x,
            cy: ty,
            r: DOT_R,
            fill: Some(b.tint(if active { ACCENT } else { TRACK_INACTIVE })),
            stroke: None,
            stroke_w: 0.0,
        });
    }
    if let Some(v) = value {
        let kx = geom.x_for(v);
        if !b.dim {
            b.p.push(Primitive::Glow {
                cx: kx,
                cy: ty,
                r: KNOB_GLOW_R,
                color: ACCENT.with_alpha(0x55),
            });
        }
        b.p.push(Primitive::Circle {
            cx: kx,
            cy: ty,
            r: KNOB_R,
            fill: Some(b.tint(ACCENT)),
            stroke: None,
            stroke_w: 0.0,
        });
    }
    // Generous vertical hit area: the visible track is 2 px tall.
    let hit = Rect::new(MARGIN, ty - 14.0, CONTENT_W, 28.0);
    if !b.dim {
        b.hits.push((hit, HitTarget::SliderTrack));
    }
    *track = Some(geom);

    // End labels
    let labels = param.end_labels();
    let active_idx = active_end_label(param, value);
    let label_y = ty + 14.0;
    let aligns = [Align::Left, Align::Center, Align::Right];
    for (i, text) in labels.iter().enumerate() {
        if text.is_empty() {
            continue;
        }
        let color = if active_idx == Some(i) {
            ACCENT_LABEL
        } else {
            TEXT_SECONDARY
        };
        b.caption(
            Rect::new(MARGIN, label_y, CONTENT_W, 14.0),
            text,
            b.tint(color),
            aligns[i],
        );
    }
    *y = label_y + 18.0 + GAP;

    // ---- warning banner ----------------------------------------------------
    if state.warn_vendor_software {
        let banner = Rect::new(MARGIN, *y, CONTENT_W, BANNER_H);
        b.card(banner, BG_BANNER, BORDER_BANNER);
        warning_icon(b, banner.x + 18.0, banner.center_y(), TEXT_MUTED);
        b.text(
            Rect::new(
                banner.x + 32.0,
                banner.y + 8.0,
                banner.w - 44.0,
                banner.h - 16.0,
            ),
            "Synapse is running and may override these settings.",
            FS_BODY - 1.0,
            W_REGULAR,
            TEXT_MUTED,
            Align::Left,
        );
        *y = banner.bottom() + GAP * 0.5;
    }
}

fn settings_body(b: &mut Builder, _state: &HeadsetState, y: &mut f32) {
    b.caption(
        Rect::new(MARGIN, *y, CONTENT_W, 14.0),
        "SETTINGS",
        TEXT_SECONDARY,
        Align::Left,
    );
    *y += 22.0;

    let rows: [(&str, &str, HitTarget, bool); 2] = [
        (
            "Start on Windows startup",
            "Launch the tray app when you sign in.",
            HitTarget::ToggleStartup,
            crate::settings_run_on_startup(),
        ),
        (
            "Display Synapse warning",
            "Warn when Synapse may override settings.",
            HitTarget::ToggleWarning,
            crate::settings_show_warning(),
        ),
    ];

    for (title, desc, target, on) in rows {
        let card = Rect::new(MARGIN, *y, CONTENT_W, 66.0);
        b.card(card, BG_CARD, BORDER_CARD);
        b.text(
            Rect::new(card.x + 16.0, card.y + 12.0, card.w - 76.0, 18.0),
            title,
            FS_BODY + 1.0,
            W_SEMIBOLD,
            TEXT_PRIMARY,
            Align::Left,
        );
        // Sized to hold the longer description on one line. Text is vertically
        // centred in its box, so a wrap does not push downward -- it grows both
        // ways and collides with the title above.
        b.text(
            Rect::new(card.x + 16.0, card.y + 34.0, card.w - 62.0, 20.0),
            desc,
            FS_DESCRIPTION,
            W_REGULAR,
            TEXT_SECONDARY,
            Align::Left,
        );
        toggle(
            b,
            Rect::new(card.right() - 58.0, card.center_y() - 11.0, 42.0, 22.0),
            on,
        );
        b.hits.push((card, target));
        *y = card.bottom() + 12.0;
    }

    // Back button
    let back = Rect::new(MARGIN, *y + 4.0, 88.0, 34.0);
    b.p.push(Primitive::RoundRect {
        rect: back,
        radius: BUTTON_RADIUS,
        fill: Some(BG_CARD),
        stroke: Some(BORDER_CARD),
        stroke_w: 1.0,
    });
    back_icon(b, back.x + 18.0, back.center_y(), TEXT_PRIMARY);
    b.text(
        Rect::new(back.x + 30.0, back.y, back.w - 34.0, back.h),
        "Back",
        FS_BODY,
        W_SEMIBOLD,
        TEXT_PRIMARY,
        Align::Left,
    );
    b.hits.push((back, HitTarget::Back));
    *y = back.bottom() + GAP * 0.5;
}

fn toggle(b: &mut Builder, r: Rect, on: bool) {
    b.p.push(Primitive::RoundRect {
        rect: r,
        radius: r.h / 2.0,
        fill: Some(if on {
            ACCENT.with_alpha(0xAA)
        } else {
            TRACK_INACTIVE
        }),
        stroke: None,
        stroke_w: 0.0,
    });
    let knob_x = if on {
        r.right() - r.h / 2.0
    } else {
        r.x + r.h / 2.0
    };
    b.p.push(Primitive::Circle {
        cx: knob_x,
        cy: r.center_y(),
        r: r.h / 2.0 - 3.0,
        fill: Some(if on { TEXT_PRIMARY } else { TEXT_SECONDARY }),
        stroke: None,
        stroke_w: 0.0,
    });
}

// ------------------------------------------------------------------- icons ---
// Drawn as primitives rather than an icon font, so they render identically
// regardless of which fonts are installed.

fn gear_icon(b: &mut Builder, cx: f32, cy: f32, c: Color) {
    b.p.push(Primitive::Circle {
        cx,
        cy,
        r: 4.2,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.4,
    });
    for i in 0..6 {
        let a = std::f32::consts::PI * 2.0 * (i as f32 / 6.0);
        let (s, co) = a.sin_cos();
        b.p.push(Primitive::Line {
            x0: cx + co * 5.0,
            y0: cy + s * 5.0,
            x1: cx + co * 7.2,
            y1: cy + s * 7.2,
            w: 1.6,
            color: c,
        });
    }
}

fn mic_icon(b: &mut Builder, cx: f32, cy: f32, c: Color, muted: bool) {
    // Capsule body. Wider and shorter than a first guess: drawn tall and narrow
    // at this size it reads as an arrow rather than a microphone.
    b.p.push(Primitive::RoundRect {
        rect: Rect::new(cx - 3.2, cy - 7.5, 6.4, 9.0),
        radius: 3.2,
        fill: Some(c),
        stroke: None,
        stroke_w: 0.0,
    });
    // Cradle: a U under the body, drawn as three segments.
    b.p.push(Primitive::Path {
        points: vec![
            (cx - 6.0, cy - 0.5),
            (cx - 6.0, cy + 2.0),
            (cx - 3.0, cy + 4.6),
            (cx + 3.0, cy + 4.6),
            (cx + 6.0, cy + 2.0),
            (cx + 6.0, cy - 0.5),
        ],
        closed: false,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.5,
    });
    b.p.push(Primitive::Line {
        x0: cx,
        y0: cy + 4.6,
        x1: cx,
        y1: cy + 7.5,
        w: 1.5,
        color: c,
    });
    if muted {
        b.p.push(Primitive::Line {
            x0: cx - 7.5,
            y0: cy + 8.0,
            x1: cx + 7.5,
            y1: cy - 8.0,
            w: 1.8,
            color: c,
        });
    }
}

fn battery_icon(b: &mut Builder, cx: f32, cy: f32, pct: Option<u8>, c: Color) {
    let body = Rect::new(cx - 12.0, cy - 7.0, 22.0, 13.0);
    b.p.push(Primitive::RoundRect {
        rect: body,
        radius: 3.5,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.4,
    });
    b.p.push(Primitive::RoundRect {
        rect: Rect::new(body.right() + 1.5, cy - 3.0, 2.2, 6.0),
        radius: 1.0,
        fill: Some(c),
        stroke: None,
        stroke_w: 0.0,
    });
    if let Some(p) = pct {
        let inner = body.w - 5.0;
        let w = (inner * (p as f32 / 100.0)).max(1.5);
        b.p.push(Primitive::RoundRect {
            rect: Rect::new(body.x + 2.5, body.y + 2.5, w, body.h - 5.0),
            radius: 1.6,
            fill: Some(ACCENT),
            stroke: None,
            stroke_w: 0.0,
        });
    }
}

fn swap_icon(b: &mut Builder, cx: f32, cy: f32, c: Color) {
    b.p.push(Primitive::Path {
        points: vec![(cx - 5.0, cy - 2.5), (cx + 5.0, cy - 2.5)],
        closed: false,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.4,
    });
    b.p.push(Primitive::Path {
        points: vec![(cx + 2.5, cy - 5.0), (cx + 5.0, cy - 2.5), (cx + 2.5, cy)],
        closed: false,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.4,
    });
    b.p.push(Primitive::Path {
        points: vec![(cx + 5.0, cy + 2.5), (cx - 5.0, cy + 2.5)],
        closed: false,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.4,
    });
    b.p.push(Primitive::Path {
        points: vec![(cx - 2.5, cy), (cx - 5.0, cy + 2.5), (cx - 2.5, cy + 5.0)],
        closed: false,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.4,
    });
}

fn warning_icon(b: &mut Builder, cx: f32, cy: f32, c: Color) {
    b.p.push(Primitive::Path {
        points: vec![(cx, cy - 7.0), (cx + 7.5, cy + 6.0), (cx - 7.5, cy + 6.0)],
        closed: true,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.3,
    });
    b.p.push(Primitive::Line {
        x0: cx,
        y0: cy - 2.5,
        x1: cx,
        y1: cy + 1.5,
        w: 1.3,
        color: c,
    });
}

fn refresh_icon(b: &mut Builder, cx: f32, cy: f32) {
    let c = TEXT_MUTED;
    b.p.push(Primitive::Path {
        points: vec![
            (cx + 6.0, cy - 1.0),
            (cx + 4.0, cy - 5.0),
            (cx - 1.0, cy - 6.5),
            (cx - 5.5, cy - 3.5),
            (cx - 6.0, cy + 1.5),
            (cx - 3.0, cy + 5.5),
            (cx + 2.0, cy + 6.0),
        ],
        closed: false,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.4,
    });
    b.p.push(Primitive::Path {
        points: vec![
            (cx + 2.5, cy - 4.5),
            (cx + 6.5, cy - 1.5),
            (cx + 3.0, cy + 1.5),
        ],
        closed: false,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.4,
    });
}

fn back_icon(b: &mut Builder, cx: f32, cy: f32, c: Color) {
    b.p.push(Primitive::Path {
        points: vec![(cx + 3.0, cy - 4.5), (cx - 2.0, cy), (cx + 3.0, cy + 4.5)],
        closed: false,
        fill: None,
        stroke: Some(c),
        stroke_w: 1.6,
    });
    b.p.push(Primitive::Line {
        x0: cx - 2.0,
        y0: cy,
        x1: cx + 6.0,
        y1: cy,
        w: 1.6,
        color: c,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected() -> HeadsetState {
        HeadsetState {
            device_name: Some("BlackShark V3 Pro PS HID".into()),
            connected: Some(true),
            battery: Some(49),
            sidetone: Some(0),
            game_chat: Some(10),
            mic_mute_hardware: Some(false),
            mic_mute_os: Some(false),
            warn_vendor_software: true,
        }
    }

    #[test]
    fn tick_counts_follow_the_parameter_range() {
        assert_eq!(SliderParam::Sidetone.ticks(), 16);
        assert_eq!(SliderParam::GameChat.ticks(), 21);
    }

    #[test]
    fn value_and_position_round_trip_at_every_tick() {
        for param in [SliderParam::Sidetone, SliderParam::GameChat] {
            let g = TrackGeometry {
                x0: 26.0,
                x1: 332.0,
                y: 100.0,
                param,
            };
            for v in 0..=param.max() {
                assert_eq!(g.value_at(g.x_for(v)), v, "{param:?} value {v}");
            }
        }
    }

    #[test]
    fn the_track_spans_the_measured_width() {
        let p = build(&connected(), View::Main, SliderParam::GameChat, None);
        let g = p.track.expect("main view has a track");
        // The mockup's dot centres are at x 26 and x 332, but those are
        // screenshot coordinates and include the 8 px shadow margin. Layout
        // works panel-local, so the same points are 18 and 324. Getting this
        // wrong once is what this assertion exists to prevent.
        assert!((g.x0 - 18.0).abs() < 1.0, "x0 was {}", g.x0);
        assert!((g.x1 - 324.0).abs() < 1.0, "x1 was {}", g.x1);
        assert!(
            (g.x1 - g.x0 - 306.0).abs() < 1.0,
            "the measured track is 306 px wide, got {}",
            g.x1 - g.x0
        );
    }

    #[test]
    fn positions_clamp_outside_the_track() {
        let g = TrackGeometry {
            x0: 26.0,
            x1: 332.0,
            y: 0.0,
            param: SliderParam::GameChat,
        };
        assert_eq!(g.value_at(-500.0), 0);
        assert_eq!(g.value_at(9999.0), 20);
    }

    #[test]
    fn value_text_matches_the_mockups_exactly() {
        use SliderParam::*;
        assert_eq!(format_value(GameChat, Some(10)), "Balanced");
        assert_eq!(format_value(GameChat, Some(17)), "Game +7");
        assert_eq!(format_value(GameChat, Some(3)), "Chat +7");
        assert_eq!(format_value(Sidetone, Some(0)), "Off");
        assert_eq!(format_value(Sidetone, Some(14)), "14");
    }

    #[test]
    fn an_unknown_value_never_renders_as_a_number() {
        assert_eq!(format_value(SliderParam::Sidetone, None), "--");
        assert_eq!(format_value(SliderParam::GameChat, None), "--");
    }

    #[test]
    fn every_interactive_target_is_hittable_in_the_main_view() {
        let p = build(&connected(), View::Main, SliderParam::GameChat, None);
        for want in [
            HitTarget::Gear,
            HitTarget::MutePill,
            HitTarget::Switcher,
            HitTarget::SliderTrack,
            HitTarget::Refresh,
        ] {
            let found = p.hits.iter().any(|(_, t)| *t == want);
            assert!(found, "{want:?} has no hit region");
        }
    }

    #[test]
    fn clicking_the_knob_position_hits_the_track() {
        let p = build(&connected(), View::Main, SliderParam::GameChat, None);
        let g = p.track.unwrap();
        assert_eq!(p.hit(g.x_for(10), g.y), Some(HitTarget::SliderTrack));
    }

    #[test]
    fn a_disconnected_panel_offers_no_controls() {
        let mut s = connected();
        s.connected = Some(false);
        s.battery = None;
        s.sidetone = None;
        s.game_chat = None;
        let p = build(&s, View::Main, SliderParam::Sidetone, None);
        for banned in [
            HitTarget::MutePill,
            HitTarget::Switcher,
            HitTarget::SliderTrack,
        ] {
            assert!(
                !p.hits.iter().any(|(_, t)| *t == banned),
                "{banned:?} must not be clickable while disconnected"
            );
        }
        // Gear and Refresh stay available: both work without the headset.
        assert!(p.hits.iter().any(|(_, t)| *t == HitTarget::Gear));
        assert!(p.hits.iter().any(|(_, t)| *t == HitTarget::Refresh));
    }

    #[test]
    fn a_hardware_muted_mic_is_not_clickable() {
        let mut s = connected();
        s.mic_mute_hardware = Some(true);
        let p = build(&s, View::Main, SliderParam::Sidetone, None);
        assert!(
            !p.hits.iter().any(|(_, t)| *t == HitTarget::MutePill),
            "software cannot release a hardware switch, so it must not offer to"
        );
    }

    #[test]
    fn the_banner_appears_only_when_warranted() {
        let with = build(&connected(), View::Main, SliderParam::Sidetone, None);
        let mut s = connected();
        s.warn_vendor_software = false;
        let without = build(&s, View::Main, SliderParam::Sidetone, None);
        assert!(
            with.height > without.height,
            "the banner should make the panel taller"
        );
    }

    #[test]
    fn settings_view_exposes_both_toggles_and_back() {
        let p = build(&connected(), View::Settings, SliderParam::Sidetone, None);
        for want in [
            HitTarget::ToggleStartup,
            HitTarget::ToggleWarning,
            HitTarget::Back,
        ] {
            assert!(p.hits.iter().any(|(_, t)| *t == want), "{want:?} missing");
        }
        assert!(p.track.is_none(), "settings has no slider");
    }

    #[test]
    fn preview_overrides_the_device_value_while_dragging() {
        let p = build(&connected(), View::Main, SliderParam::GameChat, Some(17));
        let has_text = p
            .primitives
            .iter()
            .any(|prim| matches!(prim, Primitive::Text { text, .. } if text == "Game +7"));
        assert!(
            has_text,
            "the dragged value should be shown, not the stored one"
        );
    }

    #[test]
    fn the_panel_is_the_measured_width() {
        let p = build(&connected(), View::Main, SliderParam::Sidetone, None);
        match p.primitives.first() {
            Some(Primitive::RoundRect { rect, .. }) => {
                assert_eq!(rect.w, 342.0);
                assert_eq!(rect.h, p.height);
            }
            other => panic!("expected the panel background first, got {other:?}"),
        }
    }
}
