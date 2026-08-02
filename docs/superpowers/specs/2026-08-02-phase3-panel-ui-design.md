# Phase 3 Design: Direct2D Panel UI

Status: approved 2026-08-02. Replaces the tray's `TrackPopupMenu` interface with a
custom-drawn panel matching the supplied mockups.

## Source of truth for appearance

Eight mockup screenshots, `Documents\ShareX\Screenshots\2026-08\opera_*.png`. Every colour
and dimension below was **sampled from those images**, not estimated. Where this document
and the mockups disagree, the mockups win and this document is wrong.

## Palette

Sampled by pixel. Text colours are peak-luminance within the glyph run, because
anti-aliasing makes an averaged sample read too dark.

| Token | Hex | Used for |
| --- | --- | --- |
| `bg.panel` | `#131623` | Panel background |
| `bg.card` | `#1A1D29` | Status card, settings rows |
| `bg.banner` | `#1D1E31` | Synapse warning banner |
| `bg.button` | `#292B37` | Gear button |
| `border.card` | `#272935` | Card outline |
| `border.banner` | `#2D2E41` | Banner outline |
| `track.inactive` | `#2D2F3B` | Unfilled slider dots and line |
| `accent` | `#9184D9` | Knob, status dot, filled track, switcher border |
| `accent.text` | `#D2CEFD` | Current value text |
| `accent.label` | `#CECAF9` | Active end label |
| `text.primary` | `#E9E9ED` | Title, battery percent, switcher label |
| `text.secondary` | `#8E91A6` | Subtitle, BATTERY caption, inactive end labels |
| `text.muted` | `#B0B3C7` | Banner body, footer |
| `state.live` | `#4ECB89` | Unmuted mic glyph |
| `state.muted` | `#E0555F` | Muted mic glyph |

## Geometry

Measured from `opera_oA641NTCRx.png` (358×521 including the drop shadow).

| Metric | Value |
| --- | --- |
| Panel | 342 × 504, 8 px shadow margin on all sides |
| Side margin | 17 px; content width 308 px |
| Status card | y 87–156 (height 70), full content width |
| Switcher button | 125 × 36, left-aligned at the content margin |
| Slider track | x 26–332 (306 px usable), dot diameter ≈5 px |
| Knob | radius ≈5 px plus an accent glow |
| Banner | height 49 |

Panel height shrinks from the mocked 504 once the `RESERVED` card is dropped — expect
roughly 385, and it varies with whether the warning banner is present.

**Tick counts follow the parameter range**, one dot per selectable value: 16 for sidetone
(0–15) and 21 for game/chat (0–20). This is why the game/chat track reads denser than the
sidetone track in the mockups.

## Layout

```
  ● BlackShark V3 Pro PS                      [⚙]
    CONNECTED · 2.4 GHZ
  ┌──────────────────────────────────────────────┐
  │ [▮] 49%                        [🎤 LIVE]     │
  │     BATTERY                                  │
  └──────────────────────────────────────────────┘
  [⇄ Game / Chat]                       Balanced
  ●───●───●───●───◉───●───●───●───●───●───●───●
  CHAT            BALANCED                  GAME
  ┌──────────────────────────────────────────────┐
  │ ⚠ Synapse is running and may override these  │
  └──────────────────────────────────────────────┘
  ⟳ Refresh
```

Settings view replaces the body with two cards, each a title, a description, and a pill
toggle, followed by `← Back`. The footer persists across both views.

## Value formatting

Taken from the mockups, which show `Balanced`, `Game +7`, `Off`, and `14`.

- **Game/chat**: `10` → `Balanced`; `>10` → `Game +N`; `<10` → `Chat +N`, where N is the
  distance from 10. End labels `CHAT` / `BALANCED` / `GAME`, with the one nearest the
  current value drawn in `accent.label`.
- **Sidetone**: `0` → `Off`; otherwise the bare number. End labels `OFF` / `MAX`.
- **Unknown or refused**: `--`. Never a number, and never `255`.

## Architecture

Three new modules under `crates/headset-tray/src/ui/`.

### `layout.rs` — pure, no OS access

Takes `HeadsetState`, the active view, the selected slider parameter, and a panel width;
returns two lists:

- **Primitives** to draw: rounded rect, line, dot, text run, glyph, pill.
- **Hit regions**: a rectangle paired with a `HitTarget` (`Gear`, `Back`, `MutePill`,
  `Switcher`, `SliderTrack`, `Refresh`, `ToggleStartup`, `ToggleWarning`).

This split is the point of the design. Hit-testing, tick spacing, value-to-x mapping, and
its inverse are all decided here, so they are unit-testable with no window, no GPU, and no
device attached. `render.rs` becomes a dumb walker over the primitive list.

### `render.rs` — Direct2D and DirectWrite

Creates the render target, brushes, and text formats once; redraws on `WM_PAINT`. Joins
the existing `win32` module under the same confined-`unsafe` exemption in
`docs/architecture.md`. No new crates: both APIs are `windows` crate features.

### `theme.rs`

The palette and metric tables above, as constants. One place to change if the mockups
change.

### Changes to existing code

`win32/mod.rs` keeps the notification icon and message loop, gains a borderless popup
window, and **loses** `build_settings_menu` and `build_value_menu`. `show_menu` shrinks to
the right-click menu (Refresh, Exit). Nothing in `headset-protocol`, `headset-device`, or
`headset-cli` changes.

## Interaction

| Input | Result |
| --- | --- |
| Left-click icon | Show panel above the icon, clamped to the work area; focus it |
| Focus loss / `Esc` | Hide panel |
| Right-click icon | Classic menu: Refresh, Exit |
| Gear / Back | Swap view, same window |
| Mute pill | Toggle the Windows capture endpoint |
| Switcher | Swap the slider between sidetone and game/chat |
| Slider drag | Preview locally; send **one** write on release |
| Slider click | Jump to the clicked tick; one write |

### Why the slider commits on release

`MIN_REQUEST_INTERVAL` paces requests at 250 ms, and a sidetone write costs two exchanges
because of the observed enable preamble. Writing every step of a full game/chat drag would
queue twenty writes — about five seconds, with the device visibly lagging the knob. The
knob follows the pointer locally and one command is sent when it is released.

While a write is outstanding the panel shows the pending value; the read-back that follows
every write replaces it with what the device actually reports.

## States

- **Disconnected**: header reads `DISCONNECTED` with a grey dot; battery and mic show
  `--`; switcher and slider are drawn dimmed and do not hit-test. Layout does not change
  shape, so the panel does not jump when the headset wakes.
- **Refused value** (`0xFF`): treated as unknown, never rendered as a number.
- **Warning banner**: present only when Synapse is detected *and* the setting is on.
- **Hardware mute**: the pill reads `MUTED` and the click is disabled, with a line noting
  the headset's own switch. Software cannot move a hardware switch, and a control that
  appears to do nothing is worse than one that explains itself.

## Assumptions stated in code

- **`2.4 GHZ` is a constant.** Nothing in the vendor protocol reports link type. It is
  written as a named constant with a comment saying it is not measured.
- **The device name comes from the HID product string**, so it reads
  `BlackShark V3 Pro PS`, not the mockup's `V2`.

## Testing

- **Unit, `layout.rs`**: tick spacing for both ranges; value→x and x→value round-trip
  including both ends; hit-testing every target; disconnected state produces no
  interactive regions; refused values format as `--`; value formatting against the exact
  strings in the mockups (`Balanced`, `Game +7`, `Off`, `14`).
- **Unit, `theme.rs`**: the palette constants match the sampled hex values, so a careless
  edit fails a test rather than silently changing the design.
- **Not unit-tested**: Direct2D drawing itself, which needs a device. Verified by running.
- The `CONTRIBUTING.md` gate must pass, and footprint is measured and reported rather
  than asserted: target ~1 MB binary and under 20 MB resident.

## Out of scope

- The `RESERVED` card. Dropped until something fills it; the two indexed parameters
  (`0x15`, `0x60`) are the likely EQ table, and identifying them is separate work.
- Any change to the protocol, device, or CLI crates.
