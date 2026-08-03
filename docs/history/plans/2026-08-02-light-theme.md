# Light Theme Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A light palette that follows the user's Windows preference by default, with a manual override in Settings.

**Architecture:** The palette indirection already exists — `ui::theme::Palette`, `palette()`, and accessors that `layout.rs` reads through, all added for high contrast. This adds a third palette, a resolution function deciding which applies, and one Settings row reusing the noise row's segmented control.

**Tech Stack:** Rust 1.97, `windows` 0.58 (features already enabled), no new dependencies.

Design: `docs/history/specs/2026-08-02-light-theme-design.md`.

## Global Constraints

- **No new crates.** Reading the Windows preference uses `Win32_System_Registry`, already enabled and already used by `settings.rs`.
- **The dark palette does not change.** Its values were sampled from mockups and are the record of that; the rendered dark fixtures must stay byte-identical.
- **`theme.rs` claims every value in it was measured.** Values derived rather than sampled must say so, or that claim becomes false.
- **High contrast keeps overriding everything.**
- **TDD**, and the `CONTRIBUTING.md` gate passes before every commit — checked by **exit code**, not by grepping output for "FAILED", which matches the word "failed" in passing lines.

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/headset-tray/src/ui/theme.rs` | The light palette, and the pure resolution function. | 1, 2 |
| `crates/headset-tray/src/settings.rs` | Reading and writing the `Appearance` value, and the Windows preference. | 2 |
| `crates/headset-tray/src/ui/layout.rs` | The Appearance row, reusing the segmented control. | 3 |
| `crates/headset-tray/src/win32/mod.rs` | Hit handling, and re-resolving on `WM_SETTINGCHANGE`. | 4 |
| `crates/headset-tray/src/main.rs` | Light fixtures for `--render-panel`. | 5 |

---

### Task 1: The light palette

**Files:**
- Modify: `crates/headset-tray/src/ui/theme.rs`

**Interfaces:**
- Produces: `pub fn light_palette() -> Palette`.

- [ ] **Step 1: Sample the three text colours still missing**

Nine values are already established (design doc). Three text roles need the glyph runs
located, because text is anti-aliased and a box average returns the background.

**On a light background the rule inverts:** the dark palette took peak *luminance* within a
glyph run; here take the *darkest* pixel, since the ink is dark on light.

```powershell
Add-Type -AssemblyName System.Drawing
$bm = [System.Drawing.Bitmap]::FromFile("C:\Users\Micah\Documents\ShareX\Screenshots\2026-08\opera_pLYFhYEl3B.png")
function Ink($bm, $x0, $y0, $w, $h, $label) {
  $best = 999; $hex = ""
  for ($y=$y0; $y -lt $y0+$h; $y++) { for ($x=$x0; $x -lt $x0+$w; $x++) {
    $c = $bm.GetPixel($x,$y); $l = $c.R + $c.G + $c.B
    if ($l -lt $best) { $best = $l; $hex = "#{0:X2}{1:X2}{2:X2}" -f $c.R,$c.G,$c.B }
  }}
  "{0,-22} {1}" -f $label, $hex
}
# The purple value text sits on the switcher row, right-aligned.
Ink $bm 290 280 60 26 "accent_text (value)"
# The highlighted end label under the slider.
Ink $bm 300 352 55 18 "accent_label (MAX)"
# The footer.
Ink $bm 40 405 90 22 "text_muted (Refresh)"
$bm.Dispose()
```

Record the three results. If any comes back as a near-background grey, the box missed the
glyphs — widen it and re-run rather than accepting the value. The value text is purple and
the footer is grey; a purple result for `text_muted` means the wrong row was sampled.

- [ ] **Step 2: Write the failing test**

Add to the `tests` module in `theme.rs`:

```rust
    #[test]
    fn the_light_palette_is_light_and_keeps_its_contrast() {
        set_high_contrast(false);
        let p = light_palette();

        let luminance =
            |c: Color| 0.2126 * (c.0 as f32) + 0.7152 * (c.1 as f32) + 0.0722 * (c.2 as f32);

        // Light surfaces, dark ink: the inverse of the sampled palette.
        assert!(luminance(p.bg_panel) > 200.0, "the panel should be light");
        assert!(luminance(p.bg_card) > 190.0, "cards should be light");
        assert!(luminance(p.text_primary) < 80.0, "primary text should be dark");

        // The same pairs the high-contrast palette is checked on. A light theme
        // fails differently from a dark one: pale text on a pale card is the
        // easy mistake, and it is invisible to whoever wrote it on a good
        // monitor.
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
```

The gap threshold is 40 rather than the high-contrast palette's 128: this is an ordinary
theme, not a maximum-separation one, and 128 would fail on `text_secondary`, which is
deliberately soft.

- [ ] **Step 3: Run it and watch it fail**

Run: `cargo test -p headset-tray --lib ui::theme`
Expected: fails to compile on `light_palette`. Add the signature returning
`unimplemented!()`, re-run, and confirm it fails at run time.

- [ ] **Step 4: Implement**

```rust
// ------------------------------------------------------- light palette ---

// Sampled from the light mockups by analysing colour frequency and locating
// features, not by reading fixed coordinates: those images are 363x457 while
// the dark ones are 358x521, so positions do not carry over. Guessed
// coordinates were tried first and returned background.
const L_BG_PANEL: Color = Color::rgb(0xF0F0F5);
const L_BG_CARD: Color = Color::rgb(0xE8E8EE);
const L_BORDER_CARD: Color = Color::rgb(0xD2D2D8);
const L_BG_BUTTON: Color = Color::rgb(0xDFDDED);
const L_TRACK_INACTIVE: Color = Color::rgb(0x9B9CA6);
const L_TEXT_PRIMARY: Color = Color::rgb(0x23252F);
const L_TEXT_SECONDARY: Color = Color::rgb(0x9397AB);
const L_ACCENT: Color = Color::rgb(0x6153B8);
const L_STATE_LIVE: Color = Color::rgb(0x389669);
// Sampled in step 1 of this task: replace with the measured values.
const L_ACCENT_TEXT: Color = Color::rgb(0x000000);
const L_ACCENT_LABEL: Color = Color::rgb(0x000000);
const L_TEXT_MUTED: Color = Color::rgb(0x000000);

// DERIVED, not measured. Neither mockup shows the warning banner or a muted
// microphone, so these keep the relationships the sampled dark palette uses.
//
// The banner is deliberately almost indistinguishable from a card, because in
// the dark palette it is: #1D1E31 against #1A1D29. The warning is carried by
// the glyph and the wording, not by colour, and inventing an amber here would
// be inventing a design decision nobody made.
const L_BG_BANNER: Color = Color::rgb(0xE6E5EF);
const L_BORDER_BANNER: Color = Color::rgb(0xCFCEDD);
const L_STATE_MUTED: Color = Color::rgb(0xC62B36);

/// The light palette. See the constants above for which values were sampled
/// and which were derived.
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
    }
}
```

Replace the three `0x000000` placeholders with the step 1 measurements before running the
tests. A black `accent_text` will pass the contrast check and look wrong, which is exactly
why step 1 comes first.

- [ ] **Step 5: Verify and commit**

```powershell
cargo test --workspace; "TEST: $LASTEXITCODE"
cargo clippy --workspace --all-targets -- -D warnings; "CLIPPY: $LASTEXITCODE"
cargo fmt --check; "FMT: $LASTEXITCODE"
```
All three must print 0.

```bash
git add crates/headset-tray/src/ui/theme.rs
git commit -m "feat(tray): add the light palette"
```

---

### Task 2: Choosing a theme

**Files:**
- Modify: `crates/headset-tray/src/settings.rs`
- Modify: `crates/headset-tray/src/ui/theme.rs`

**Interfaces:**
- Produces: `pub enum Appearance { System, Light, Dark }` with `settings::appearance()` /
  `settings::set_appearance()`, `settings::windows_prefers_light()`, and
  `theme::resolve(appearance, windows_prefers_light, high_contrast) -> Which`.

- [ ] **Step 1: Write the failing table test**

This is where the precedence rule lives and it is the part most likely to be got wrong, so
it is tested exhaustively. In `theme.rs`:

```rust
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
```

Run: `cargo test -p headset-tray --lib ui::theme` — fails to compile until the types exist,
then fails at run time against `unimplemented!()`.

- [ ] **Step 2: Implement the types and the resolution**

In `theme.rs`:

```rust
/// What the user asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    /// Follow Windows. The default.
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
```

Then replace the `HIGH_CONTRAST` static with one holding the resolved `Which`, keeping
`set_high_contrast` working for the existing call sites:

```rust
/// The resolved palette, stored as a discriminant. An atomic rather than a
/// lock: read on every primitive during a paint, written approximately never.
static RESOLVED: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1); // Dark

pub fn set_palette(which: Which) {
    let v = match which {
        Which::Light => 0,
        Which::Dark => 1,
        Which::HighContrast => 2,
    };
    RESOLVED.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// Kept so existing callers and tests read naturally.
pub fn set_high_contrast(on: bool) {
    set_palette(if on { Which::HighContrast } else { Which::Dark });
}

pub fn palette() -> Palette {
    match RESOLVED.load(std::sync::atomic::Ordering::Relaxed) {
        0 => light_palette(),
        2 => high_contrast_palette(),
        _ => sampled_palette(),
    }
}
```

- [ ] **Step 3: Read and write the preference**

In `settings.rs`, next to `show_synapse_warning`, following the same pattern:

```rust
const APPEARANCE_VALUE: &str = "Appearance";

/// The user's appearance choice. Absent means follow Windows, which is the
/// default a fresh install gets.
pub fn appearance() -> crate::ui::theme::Appearance {
    use crate::ui::theme::Appearance;
    match read_string(HKEY_CURRENT_USER, APP_KEY, APPEARANCE_VALUE).as_deref() {
        Some("light") => Appearance::Light,
        Some("dark") => Appearance::Dark,
        _ => Appearance::System,
    }
}

pub fn set_appearance(a: crate::ui::theme::Appearance) -> bool {
    use crate::ui::theme::Appearance;
    set_string(
        APP_KEY,
        APPEARANCE_VALUE,
        match a {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        },
    )
}

/// Whether Windows is set to light for applications.
///
/// Absent is dark, which is what Windows itself does with this value.
pub fn windows_prefers_light() -> bool {
    const PERSONALIZE: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    read_dword(HKEY_CURRENT_USER, PERSONALIZE, "AppsUseLightTheme") == Some(1)
}
```

If `settings.rs` has no `read_dword`, add one beside `read_string` using `RegGetValueW`
with `RRF_RT_REG_DWORD`, returning `Option<u32>`.

- [ ] **Step 4: Apply it at startup**

In `win32::run_ui_with`, replace the existing high-contrast line:

```rust
        crate::ui::theme::set_palette(crate::ui::theme::resolve(
            crate::settings::appearance(),
            crate::settings::windows_prefers_light(),
            high_contrast_enabled(),
        ));
```

- [ ] **Step 5: Verify and commit**

```powershell
cargo test --workspace; "TEST: $LASTEXITCODE"
cargo clippy --workspace --all-targets -- -D warnings; "CLIPPY: $LASTEXITCODE"
```

```bash
git add crates/headset-tray/src/settings.rs crates/headset-tray/src/ui/theme.rs crates/headset-tray/src/win32/mod.rs
git commit -m "feat(tray): resolve the palette from the user's choice and Windows"
```

---

### Task 3: The Appearance row

**Files:**
- Modify: `crates/headset-tray/src/ui/layout.rs`

**Interfaces:**
- Consumes: `Appearance` from Task 2.
- Produces: `HitTarget::AppearanceSystem`, `AppearanceDark`, `AppearanceLight`.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn the_settings_view_offers_all_three_appearance_choices() {
        let p = build(&connected(), View::Settings, SliderParam::GameChat, None);
        let targets: Vec<HitTarget> = p.hits.iter().map(|(_, t)| *t).collect();
        for want in [
            HitTarget::AppearanceSystem,
            HitTarget::AppearanceDark,
            HitTarget::AppearanceLight,
        ] {
            assert!(targets.contains(&want), "{want:?} is not reachable");
        }
    }

    #[test]
    fn the_appearance_segments_do_not_overlap() {
        let p = build(&connected(), View::Settings, SliderParam::GameChat, None);
        let seg = |t: HitTarget| p.hits.iter().find(|(_, h)| *h == t).expect("present").0;
        let (a, b, c) = (
            seg(HitTarget::AppearanceSystem),
            seg(HitTarget::AppearanceDark),
            seg(HitTarget::AppearanceLight),
        );
        assert!(a.right() <= b.x, "system overlaps dark");
        assert!(b.right() <= c.x, "dark overlaps light");
    }

    #[test]
    fn the_appearance_subtitle_says_what_actually_resolved() {
        // AUTO must not be a black box: a user whose Windows is dark and who
        // picks AUTO should be able to see why the panel stayed dark.
        assert_eq!(appearance_subtitle(Appearance::Light), "Light theme");
        assert_eq!(appearance_subtitle(Appearance::Dark), "Dark theme");
        assert!(appearance_subtitle(Appearance::System).starts_with("Following Windows"));
    }
```

Run and confirm they fail.

- [ ] **Step 2: Add the hit targets and the subtitle**

Add the three variants to `HitTarget`, then:

```rust
/// What the Appearance row says beneath its title.
pub fn appearance_subtitle(a: Appearance) -> String {
    match a {
        Appearance::Light => "Light theme".to_string(),
        Appearance::Dark => "Dark theme".to_string(),
        Appearance::System => {
            let resolved = if crate::settings::windows_prefers_light() {
                "light"
            } else {
                "dark"
            };
            format!("Following Windows — {resolved}")
        }
    }
}
```

- [ ] **Step 3: Draw the row**

In `settings_body`, after the two existing toggle rows, add a row of the same height and
card treatment, with a three-segment control on its right. **Factor the segment drawing out
of `noise_section` into a shared helper first** — the point of this design is that it is the
same control, and two copies would drift:

```rust
/// A row of segments, the active one filled with the accent. Used by the noise
/// mode row and by the Appearance row, deliberately: the panel teaches this
/// pattern once.
fn segmented(
    b: &mut Builder,
    rect: Rect,
    labels: &[&str],
    active: Option<usize>,
    targets: &[HitTarget],
) {
    // ... body moved from noise_section, unchanged in behaviour ...
}
```

Have `noise_section` call it, and confirm the rendered dark fixtures are still
byte-identical afterwards — that is the check that the refactor changed nothing.

- [ ] **Step 4: Verify and commit**

```powershell
cargo test --workspace; "TEST: $LASTEXITCODE"
.\target\release\headset-tray.exe --render-panel .\out
```
Compare `out\` against the committed expectations for the **dark** states: identical.

```bash
git add crates/headset-tray/src/ui/layout.rs
git commit -m "feat(tray): add the Appearance row, reusing the segmented control"
```

---

### Task 4: Making it work

**Files:**
- Modify: `crates/headset-tray/src/win32/mod.rs`

- [ ] **Step 1: Handle the clicks**

In `on_panel_press`:

```rust
        HitTarget::AppearanceSystem => set_appearance(ctx, Appearance::System),
        HitTarget::AppearanceDark => set_appearance(ctx, Appearance::Dark),
        HitTarget::AppearanceLight => set_appearance(ctx, Appearance::Light),
```

and the helper:

```rust
/// Stores the choice, re-resolves, and repaints.
fn set_appearance(ctx: &mut Ctx, a: crate::ui::theme::Appearance) {
    if crate::settings::set_appearance(a) {
        crate::ui::theme::set_palette(crate::ui::theme::resolve(
            a,
            crate::settings::windows_prefers_light(),
            high_contrast_enabled(),
        ));
        redraw_panel(ctx);
    }
}
```

- [ ] **Step 2: Follow Windows while the panel is open**

Extend the existing `WM_SETTINGCHANGE` arm — it already re-reads high contrast — to
re-resolve the whole thing, so switching Windows to light updates a panel set to `AUTO`:

```rust
        WM_SETTINGCHANGE => {
            crate::ui::theme::set_palette(crate::ui::theme::resolve(
                crate::settings::appearance(),
                crate::settings::windows_prefers_light(),
                high_contrast_enabled(),
            ));
            with_ctx(|ctx| {
                if ctx.panel_visible {
                    redraw_panel(ctx);
                }
            });
            LRESULT(0)
        }
```

- [ ] **Step 3: Verify by hand**

Build, install, and open the panel:

1. Settings → Appearance shows three segments, with `AUTO` active on a fresh install.
2. Click the sun. **The panel turns light immediately**, and the subtitle reads
   `Light theme`.
3. Close and reopen the panel: still light. Exit and restart the tray: still light — the
   choice is in the registry, not in memory.
4. Click `AUTO`. On this machine Windows is dark, so the panel returns to dark and the
   subtitle reads `Following Windows — dark`.
5. With the panel open and on `AUTO`, switch Windows to light (Settings → Personalisation →
   Colours → Choose your mode → Light). **The panel follows without being reopened.**
6. Turn on high contrast. The panel switches to the high-contrast palette regardless of the
   Appearance choice, and the Appearance row still shows what is selected.

Step 5 is the one that fails if `WM_SETTINGCHANGE` was missed, and step 3 is the one that
fails if the value was never written.

- [ ] **Step 4: Commit**

```bash
git add crates/headset-tray/src/win32/mod.rs
git commit -m "feat(tray): apply the appearance choice, and follow Windows when set to auto"
```

---

### Task 5: Fixtures and documentation

**Files:**
- Modify: `crates/headset-tray/src/main.rs`, `README.md`, `CHANGELOG.md`

- [ ] **Step 1: Add light fixtures**

`--render-panel` sets the palette per case already, for the high-contrast fixture. Add
light versions of the main and settings views the same way, so the theme is diffable:

```rust
    for (name, which) in [
        ("light-gamechat-balanced", Which::Light),
        ("light-settings", Which::Light),
    ] { /* push cases, setting the palette per case as the high-contrast one does */ }
```

Confirm every **dark** fixture is byte-identical to before.

- [ ] **Step 2: Document it**

In `README.md`, under "Using it", after the settings paragraph:

```markdown
The panel follows your Windows light or dark setting by default. **Appearance** in settings
overrides it — auto, dark, or light. High contrast, if you use it, wins over all three.
```

In `CHANGELOG.md`, under a new `## [Unreleased]` heading:

```markdown
### Added

- A light theme, following your Windows setting by default, with an override in settings.
```

- [ ] **Step 3: Verify and commit**

```powershell
cargo fmt --check; "FMT: $LASTEXITCODE"
cargo clippy --workspace --all-targets -- -D warnings; "CLIPPY: $LASTEXITCODE"
cargo test --workspace; "TEST: $LASTEXITCODE"
cargo build --workspace --release; "BUILD: $LASTEXITCODE"
```

```bash
git add -A
git commit -m "docs: document the light theme, and render it as a fixture"
```

---

## Done when

- A fresh install follows Windows; an override sticks across a restart.
- Switching Windows' mode updates an open panel set to auto.
- High contrast still overrides everything.
- The dark fixtures are byte-identical, proving the shared segmented control changed nothing.
- `theme.rs` distinguishes sampled values from derived ones.
- The full gate passes, checked by exit code.
