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

## Transparency and window composition

The mockups are a web page, so the 8 px margin around the panel is a CSS drop shadow over
the page background, not window transparency. Reproducing it on a real desktop needs the
panel to composite correctly over whatever happens to be behind it — taskbar, wallpaper,
another window.

**The panel is a layered window** (`WS_EX_LAYERED`) updated with `UpdateLayeredWindow`
from a premultiplied-alpha bitmap that Direct2D renders. That buys three things a plain
opaque window cannot:

- The drop shadow composites over the real background instead of over a guessed colour.
- Rounded corners get anti-aliased edges rather than a stair-stepped region clip.
- The knob glow can fade to true transparency at its edges.

The alternative — a normal window with the system `CS_DROPSHADOW` style — is far less code
but produces Windows' shadow, not the mockup's. Given the 1:1 requirement, the layered
window is the right cost.

The panel body itself is **opaque** `#131623`. Sampling confirms a uniform fill with no
blur or translucency behind it, so there is no acrylic or mica effect to reproduce.

## Fidelity verification

"As close to the screenshots as possible" is checkable rather than a matter of opinion, so
the build includes the means to check it.

`headset-tray.exe --render-panel <out.png> [--state <fixture>]` renders the panel offscreen
to a PNG at exact size, through the same Direct2D path the live window uses, with the
device state supplied from a fixture rather than hardware. Uses WIC, already a `windows`
crate feature; no new crates and no device needed.

That makes iteration objective: render, pixel-diff against the corresponding mockup, and
work the difference down. It also means a later change that quietly shifts the layout can
be caught by re-diffing rather than by noticing.

**The known fidelity risk is the font.** The mockups were rendered by a browser; the panel
renders through DirectWrite. If the page used `system-ui`, Opera on Windows resolves that
to Segoe UI and DirectWrite will match closely. If it named a specific family such as
Inter, glyph shapes and metrics will differ visibly and we would need that font installed
or embedded. The diff harness will show which case we are in on the first render, and this
is the most likely reason a first attempt looks subtly off.

## Designed for tweaking

Everything that determines appearance is data, in one place:

- `theme.rs` holds every colour and metric as a named constant.
- `layout.rs` computes positions from those constants; no literal offsets are scattered
  through drawing code.

A visual change is therefore a constant edit plus a rebuild, not a hunt through paint
routines. The palette test pins the constants to the sampled values, so an accidental
change fails a test — an intentional one just updates both together.

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
- **Pixel diff**: `--render-panel` output compared against the corresponding mockup for
  each state (live, muted, sidetone, game/chat, both settings states). Reported as a
  percentage of differing pixels and a worst-region location, so "close enough" is a
  number rather than an impression.
- **Not unit-tested**: Direct2D drawing itself, which needs a device. Verified by running
  and by the diff above.
- The `CONTRIBUTING.md` gate must pass, and footprint is measured and reported rather
  than asserted: target ~1 MB binary and under 20 MB resident.

## Out of scope

- The `RESERVED` card. Dropped until something fills it; the two indexed parameters
  (`0x15`, `0x60`) are the likely EQ table, and identifying them is separate work.
- Any change to the protocol, device, or CLI crates.

## Addendum: noise control (2026-08-02, after this spec was approved)

The `RESERVED` slot above has been filled. Parameter `0x12` was identified after this
spec was written (see `docs/device-research.md`), and the panel gained a noise-control
block between the slider and the warning banner:

```
  NOISE CONTROL                                ANC 3
  ┌──────────┬──────────┬──────────┐
  │   OFF    │   ANC    │ AMBIENT  │   active segment filled with accent
  └──────────┴──────────┴──────────┘
  ●───────●───────◉───────●
  1                       4
```

Four things about it are decisions rather than sampling, since the mockups predate the
parameter and have nothing to copy:

- **Three segments, not a toggle plus a switcher.** The device holds one mode byte with
  three observed values, so one control with three regions maps to it exactly and reaches
  any state in one click.
- **The level row is always drawn, and only hit-tests in ANC.** Hiding it would change the
  panel's height when the mode changes and make it jump — the same reasoning the
  disconnected state uses. The retained level stays visible in every named mode, because
  the device really does keep it and land on it when ANC returns.
- **The ends are labelled `1` and `4`, not `LOW` and `HIGH`.** Nothing observed establishes
  which end is the stronger cancellation. The numbers are the vendor UI's own.
- **`SEGMENT_H` is not a sampled metric.** It matches `SWITCHER_H`, and says so in
  `theme.rs`.

New hit targets: `NoiseOff`, `NoiseAnc`, `NoiseAmbient`, `NoiseLevel`. Clicking any of
them is a read-modify-write composed from the state the panel is showing, because mode and
level go out together; nothing is sent while that state is unknown.

This addendum also supersedes "Any change to the protocol, device, or CLI crates" for the
noise work, which added `headset-protocol::noise` and a `headsetctl noise` command.

## Accessibility: an accepted trade-off (recorded 2026-08-02)

The panel is a custom-drawn layered window. Windows therefore sees a single bitmap: there
is **no UI Automation tree**, so screen readers announce nothing, there is no keyboard
navigation between controls inside the panel, and no focus indicator.

This is a consequence of the decision at the top of this document — that the mockups are
the source of truth and appearance should match them 1:1. A panel built from real
controls, or from XAML islands as EarTrumpet uses, would get an accessibility tree,
keyboard navigation, and high-contrast support from the platform, at the cost of the pixel
fidelity this design exists to achieve.

**The trade-off is accepted, with mitigations:**

- Every destructive or essential action — Refresh, Exit — is also on the right-click menu,
  which is a standard `TrackPopupMenu` and is fully keyboard and screen-reader accessible.
- The notification icon itself answers the keyboard, via `NIN_KEYSELECT` under
  notification version 4. Enter or Space on the focused icon opens the panel, and the Menu
  key opens the context menu.
- Every setting the panel exposes is also reachable from `headsetctl`, which is a console
  application and accessible by construction.
- High contrast is honoured by the palette, so the panel does not become unreadable for
  the users most likely to need it.

**Revisit this if** a user needs screen-reader access to the panel, or if the panel grows
controls that have no equivalent in the right-click menu or the CLI. The rebuild is large
and should be planned as its own phase, not retrofitted.
