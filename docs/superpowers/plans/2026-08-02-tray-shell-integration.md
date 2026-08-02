# Tray Shell Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the tray up to the shell-integration and accessibility conventions that mainstream Windows tray utilities follow — keyboard-reachable icon, stable icon identity, correct placement against a side-docked taskbar, and a panel that does not become unreadable in high contrast.

**Architecture:** Four behavioural changes plus one decision record. The notification-icon changes (Tasks 1–2) are `NOTIFYICONDATAW` configuration and message-dispatch work in `win32/mod.rs`; the placement change (Task 3) extends the pure `win32::place` module so it stays unit-tested; the contrast change (Task 4) adds a second palette behind a runtime check in `ui::theme`.

**Tech Stack:** Rust 1.97.1, `x86_64-pc-windows-gnu`, `windows` crate 0.58 (features only — no new crates), Direct2D/DirectWrite.

## Prerequisite

**This plan depends on `2026-08-02-tray-shell-correctness.md` being merged first.**

- Task 1 here modifies `readd_icon`, which that plan's Task 2 creates. Applied out of order, the icon silently reverts to the legacy notification version after every Explorer restart.
- Task 3 here extends `win32::place`, which that plan's Task 1 creates.

## Global Constraints

- **No new crates.** Additional `windows` *features* are permitted; a version bump is not.
- **`windows` stays pinned at 0.58** — see the comment in `Cargo.toml` about `raw-dylib` and `dlltool.exe`.
- **The target is `x86_64-pc-windows-gnu`, not MSVC.**
- **`panic = "abort"` in release.** A panic in a window procedure kills the process.
- **`unsafe` lives only in `crates/headset-tray/src/win32/`.**
- **TDD** for anything pure; explicit manual verification steps for anything that only the shell can exercise.
- **The `CONTRIBUTING.md` gate must pass before every commit:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace --release`.

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/headset-tray/src/win32/mod.rs` | Notification-icon version and identity; the v4 message dispatch. | 1, 2 |
| `crates/headset-tray/src/win32/place.rs` | Gains taskbar-edge inference and side placement. Still pure, still tested. | 3 |
| `crates/headset-tray/src/win32/panel.rs` | Passes the icon and work rectangles to the extended placement call. | 3 |
| `crates/headset-tray/src/ui/theme.rs` | A high-contrast palette and the accessor that chooses between palettes. | 4 |
| `crates/headset-tray/src/ui/render.rs` | Reads colours through the accessor rather than the constants directly. | 4 |
| `docs/superpowers/specs/2026-08-02-phase3-panel-ui-design.md` | The accessibility decision record. | 5 |

---

### Task 1: Notification icon version 4, and a keyboard-reachable icon

**Problem being fixed:** `win32/mod.rs:196` calls `NIM_ADD` and never `NIM_SETVERSION`, so the icon runs at the legacy notification version. The user-visible consequence is that **the icon cannot be operated from the keyboard**: modern shells deliver `NIN_SELECT` and `NIN_KEYSELECT` only to v4 icons, so a keyboard user can focus the icon in the notification area and press Enter or Space to no effect. v4 also carries the icon's screen position in `wParam`, which removes the need for the `Shell_NotifyIconGetRect` call in the common case.

**A trap:** `NIN_KEYSELECT` is **not exposed by the `windows` 0.58 crate** — only `NIN_SELECT` (1024) and `NIN_POPUPOPEN` (1030) are. It must be defined locally as `NIN_SELECT | 1`.

**Files:**
- Modify: `crates/headset-tray/src/win32/mod.rs` (constants, `run_ui_with`, `readd_icon`, `wndproc`)

**Interfaces:**
- Consumes: `readd_icon(ctx: &mut Ctx)` from the correctness plan's Task 2.
- Produces: `fn set_icon_version(nid: &NOTIFYICONDATAW)`, called from both the startup path and `readd_icon`.

- [ ] **Step 1: Add the notification constants**

Add near the other message constants in `win32/mod.rs`:

```rust
/// Sent when the icon is activated by mouse or keyboard, under version 4.
///
/// `NIN_SELECT` is in the `windows` crate; `NIN_KEYSELECT` is not, so it is
/// spelled out here. The shell defines it as `NIN_SELECT | 1` and sends it for
/// Enter or Space on a focused icon — the whole reason for moving to version 4.
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
```

Add `NIN_SELECT` and `NOTIFYICON_VERSION_4` to the `windows::Win32::UI::Shell` import list, and `WM_CONTEXTMENU` to the `windows::Win32::UI::WindowsAndMessaging` list.

- [ ] **Step 2: Add the version setter**

`uVersion` lives in an anonymous union with `uTimeout`, so it is written through `Anonymous`:

```rust
/// Opts the icon into notification version 4.
///
/// Must follow every `NIM_ADD`, including the one in `readd_icon`: the shell
/// forgets the version along with the icon when it restarts, and an icon that
/// silently reverts to the legacy version stops answering the keyboard.
fn set_icon_version(nid: &NOTIFYICONDATAW) {
    let mut versioned = *nid;
    // uVersion shares a union with uTimeout; this field is only read by
    // NIM_SETVERSION, so setting it here affects nothing else.
    versioned.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    unsafe {
        if !Shell_NotifyIconW(NIM_SETVERSION, &versioned).as_bool() {
            tracing::warn!("could not set notification version 4; icon will not answer the keyboard");
        }
    }
}
```

- [ ] **Step 3: Call it from both add paths**

In `run_ui_with`, immediately after the existing `Shell_NotifyIconW(NIM_ADD, &nid).ok()?;`:

```rust
        set_icon_version(&nid);
```

And in `readd_icon`, inside the success branch:

```rust
        if Shell_NotifyIconW(NIM_ADD, &ctx.nid).as_bool() {
            set_icon_version(&ctx.nid);
            tracing::info!("re-added the tray icon after a shell restart");
        } else {
```

- [ ] **Step 4: Rewrite the `WM_TRAY` dispatch for version 4**

Under v4 the packing changes: the notification is in the **low word of `lParam`** (the high word carries the icon id), and `wParam` carries the icon's screen position. Replace the whole `WM_TRAY` arm:

```rust
        WM_TRAY => {
            // Version 4 packing: notification in the low word of lParam, icon
            // id in the high word. Masking is required -- comparing the whole
            // lParam against WM_LBUTTONUP silently stops matching.
            let notification = (lp.0 as u32) & 0xFFFF;
            match notification {
                // NIN_SELECT is a click; NIN_KEYSELECT is Enter or Space on a
                // focused icon. Both mean "activate", and handling only the
                // first is what makes an icon mouse-only.
                NIN_SELECT | NIN_KEYSELECT => {
                    with_ctx(toggle_panel);
                }
                // Replaces WM_RBUTTONUP under version 4, and is also what the
                // Menu key and Shift+F10 produce on a focused icon.
                WM_CONTEXTMENU => {
                    with_ctx(|ctx| show_menu(hwnd, ctx));
                }
                _ => {}
            }
            LRESULT(0)
        }
```

- [ ] **Step 5: Run the gate**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```
Expected: clean.

- [ ] **Step 6: Verify by hand, mouse and keyboard**

The keyboard path is the point of this task; testing only the mouse would pass while the feature does nothing.

1. Left-click the icon → panel opens. Left-click again → closes.
2. Right-click the icon → the Refresh/Exit menu appears.
3. **Keyboard:** press `Win+B` to focus the notification area, arrow to the headset icon, press `Enter`. The panel must open. Press `Esc`, then `Space` on the icon — it must open again.
4. **Keyboard menu:** with the icon focused, press the Menu key (or `Shift+F10`). The context menu must appear.
5. Restart Explorer (Task 2 of the correctness plan) and repeat step 3. This is what catches `set_icon_version` missing from `readd_icon`.

- [ ] **Step 7: Commit**

```bash
git add crates/headset-tray/src/win32/mod.rs
git commit -m "feat(tray): notification version 4, so the icon answers the keyboard"
```

---

### Task 2: Stable icon identity with `NIF_GUID`

**Problem being fixed:** the icon is identified by `hWnd` + `uID` (`win32/mod.rs:189`). Windows keys the user's "Show icon in the taskbar" choice and the icon's position to that identity, which is not stable across the app moving. `install::install()` relocates the exe to `%LOCALAPPDATA%\Programs\HeadsetTray`, so a user who pins the icon before installing loses that choice afterwards. `NIF_GUID` exists for exactly this.

**The catch, which drives the design below:** Windows binds a notification GUID to the **path of the registering executable**. Registering the same GUID from a different path fails. Since this app deliberately moves its own binary, `NIM_ADD` will fail the first time the installed copy runs, and the code must fall back rather than end up with no icon at all.

**Files:**
- Modify: `crates/headset-tray/src/win32/mod.rs` (`run_ui_with`, `readd_icon`)

**Interfaces:**
- Consumes: `set_icon_version` from Task 1.
- Produces: `fn add_icon(nid: &mut NOTIFYICONDATAW) -> bool` — a single add path with the GUID fallback, used by startup and by `readd_icon`.

- [ ] **Step 1: Add the GUID and the add-with-fallback helper**

```rust
/// Identifies this tray icon to the shell across restarts and reinstalls.
///
/// Arbitrary but permanent: the value itself means nothing, and changing it
/// discards every user preference attached to the icon. Do not regenerate it.
const ICON_GUID: GUID = GUID::from_u128(0x7f3a2c14_9b6d_4e58_a1c2_5d90e3b47f61);

/// Adds the notification icon, preferring a stable GUID identity.
///
/// Windows binds a notification GUID to the registering executable's path, so
/// the first run from a new location — which `--install` creates by design —
/// is rejected. Falling back to hWnd+uID identity keeps the icon working; the
/// user loses their pin-to-taskbar choice that once, rather than losing the
/// icon entirely.
///
/// Mutates `nid` so the caller keeps whichever identity actually succeeded:
/// every later NIM_MODIFY and the final NIM_DELETE must use the same one.
fn add_icon(nid: &mut NOTIFYICONDATAW) -> bool {
    nid.uFlags |= NIF_GUID;
    nid.guidItem = ICON_GUID;
    unsafe {
        if Shell_NotifyIconW(NIM_ADD, nid).as_bool() {
            return true;
        }
        tracing::warn!(
            "the shell refused this icon's GUID, most likely because the executable \
             moved; falling back to window-handle identity"
        );
        nid.uFlags &= !NIF_GUID;
        nid.guidItem = GUID::zeroed();
        Shell_NotifyIconW(NIM_ADD, nid).as_bool()
    }
}
```

Add `NIF_GUID` to the `windows::Win32::UI::Shell` import list.

- [ ] **Step 2: Route both add paths through it**

In `run_ui_with`, replace `Shell_NotifyIconW(NIM_ADD, &nid).ok()?;` with:

```rust
        if !add_icon(&mut nid) {
            return Err(windows::core::Error::from_win32());
        }
        set_icon_version(&nid);
```

In `readd_icon`, replace the `NIM_ADD` call:

```rust
        if add_icon(&mut ctx.nid) {
            set_icon_version(&ctx.nid);
            tracing::info!("re-added the tray icon after a shell restart");
        } else {
            tracing::error!("could not re-add the tray icon after a shell restart");
        }
```

`readd_icon` takes `&mut Ctx` already, so `&mut ctx.nid` is available.

- [ ] **Step 3: Run the gate**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```
Expected: clean.

- [ ] **Step 4: Verify by hand, including the fallback**

The fallback is the part most likely to be wrong, so exercise it deliberately:

1. Run the tray from `target\release`. Confirm the icon appears.
2. Right-click the taskbar → Taskbar settings → "Other system tray icons", and set the headset icon to always show.
3. Exit and relaunch from the same path. The icon should still be set to always show.
4. Exit, and run `headset-tray.exe --install`. The installed copy launches from a different path — **confirm an icon still appears**. A missing icon here means the fallback is broken; the log will contain "the shell refused this icon's GUID".
5. Exit the installed copy and relaunch it. Repeat step 2 for the installed path, then relaunch again and confirm the preference sticks.

- [ ] **Step 5: Commit**

```bash
git add crates/headset-tray/src/win32/mod.rs
git commit -m "feat(tray): give the icon a stable identity so pinning survives"
```

---

### Task 3: Place the panel correctly against a side-docked taskbar

**Problem being fixed:** `place::above_icon` assumes the taskbar is horizontal — it centres the panel on the icon and puts it above, dropping below only if there is no room. With the taskbar docked left or right, the icon is at the side of the screen and the panel is drawn over the taskbar.

**Files:**
- Modify: `crates/headset-tray/src/win32/place.rs` (new inference plus a new entry point, and their tests)
- Modify: `crates/headset-tray/src/win32/panel.rs` (call the new entry point)

**Interfaces:**
- Consumes: `Bounds`, `above_icon` from the correctness plan's Task 1.
- Produces: `pub enum TaskbarEdge { Bottom, Top, Left, Right }`, `pub fn taskbar_edge(icon: Bounds, work: Bounds) -> TaskbarEdge`, and `pub fn beside_icon(icon: Bounds, work: Bounds, w: i32, h: i32, margin: i32) -> (i32, i32)`. `beside_icon` replaces `above_icon` at the `panel.rs` call site and delegates to it for horizontal taskbars.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `place.rs`:

```rust
    #[test]
    fn the_taskbar_edge_is_inferred_from_where_the_icon_sits() {
        // The icon is always in the taskbar, and the taskbar is always the gap
        // between the work area and the screen. Whichever work-area edge the
        // icon is outside of is the edge the taskbar is docked to.
        let work = Bounds { left: 0, top: 0, right: 1920, bottom: 1032 };
        let below = Bounds { left: 1700, top: 1032, right: 1724, bottom: 1056 };
        assert_eq!(taskbar_edge(below, work), TaskbarEdge::Bottom);

        let work_top = Bounds { left: 0, top: 48, right: 1920, bottom: 1080 };
        let above = Bounds { left: 1700, top: 12, right: 1724, bottom: 36 };
        assert_eq!(taskbar_edge(above, work_top), TaskbarEdge::Top);

        let work_left = Bounds { left: 72, top: 0, right: 1920, bottom: 1080 };
        let at_left = Bounds { left: 24, top: 900, right: 48, bottom: 924 };
        assert_eq!(taskbar_edge(at_left, work_left), TaskbarEdge::Left);

        let work_right = Bounds { left: 0, top: 0, right: 1848, bottom: 1080 };
        let at_right = Bounds { left: 1872, top: 900, right: 1896, bottom: 924 };
        assert_eq!(taskbar_edge(at_right, work_right), TaskbarEdge::Right);
    }

    #[test]
    fn an_icon_inside_the_work_area_is_assumed_to_be_a_bottom_taskbar() {
        // The overflow-flyout fallback synthesises a rectangle from the cursor,
        // which can be anywhere. Bottom is the overwhelmingly common case and
        // the one the old code always assumed.
        let work = Bounds { left: 0, top: 0, right: 1920, bottom: 1032 };
        let stray = Bounds { left: 900, top: 500, right: 900, bottom: 500 };
        assert_eq!(taskbar_edge(stray, work), TaskbarEdge::Bottom);
    }

    #[test]
    fn a_left_taskbar_puts_the_panel_to_its_right() {
        let work = Bounds { left: 72, top: 0, right: 1920, bottom: 1080 };
        let icon = Bounds { left: 24, top: 900, right: 48, bottom: 924 };
        let (x, y) = beside_icon(icon, work, 342, 500, 8);
        assert_eq!(x, 72 + 8, "clear of the taskbar, one margin into the work area");
        assert!(y + 500 <= 1080, "bottom edge stays on screen, got y = {y}");
        assert!(y >= 0, "top edge stays on screen, got y = {y}");
    }

    #[test]
    fn a_right_taskbar_puts_the_panel_to_its_left() {
        let work = Bounds { left: 0, top: 0, right: 1848, bottom: 1080 };
        let icon = Bounds { left: 1872, top: 900, right: 1896, bottom: 924 };
        let (x, _) = beside_icon(icon, work, 342, 500, 8);
        assert_eq!(x, 1848 - 8 - 342, "clear of the taskbar on the other side");
    }

    #[test]
    fn a_horizontal_taskbar_still_goes_through_above_icon() {
        // Same answer as before this task existed: the common case must not
        // change behaviour.
        let work = Bounds { left: 0, top: 0, right: 1920, bottom: 1032 };
        let icon = Bounds { left: 1700, top: 1032, right: 1724, bottom: 1056 };
        assert_eq!(
            beside_icon(icon, work, 342, 500, 8),
            above_icon(icon, work, 342, 500, 8)
        );
    }
```

- [ ] **Step 2: Add the signatures and run the tests to verify they fail**

Add to `place.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskbarEdge { Bottom, Top, Left, Right }

pub fn taskbar_edge(icon: Bounds, work: Bounds) -> TaskbarEdge { unimplemented!() }

pub fn beside_icon(icon: Bounds, work: Bounds, w: i32, h: i32, margin: i32) -> (i32, i32) {
    unimplemented!()
}
```

Run: `cargo test -p headset-tray --lib win32::place`
Expected: the 5 new tests FAIL with `not implemented`; the 7 from the correctness plan still pass.

- [ ] **Step 3: Implement both**

```rust
/// Which screen edge the taskbar is docked to, inferred from the icon.
///
/// The taskbar is the gap between the work area and the screen, and the icon
/// lives in it, so the work-area edge the icon falls outside of is the answer.
/// No shell API is consulted: `SHAppBarMessage` would give the same result and
/// this stays testable.
pub fn taskbar_edge(icon: Bounds, work: Bounds) -> TaskbarEdge {
    if icon.top >= work.bottom {
        TaskbarEdge::Bottom
    } else if icon.bottom <= work.top {
        TaskbarEdge::Top
    } else if icon.right <= work.left {
        TaskbarEdge::Left
    } else if icon.left >= work.right {
        TaskbarEdge::Right
    } else {
        // Inside the work area: not a real icon rectangle. The cursor fallback
        // produces this. Bottom is the common case.
        TaskbarEdge::Bottom
    }
}

/// Top-left for a `w` x `h` panel placed clear of the taskbar the icon is in.
pub fn beside_icon(icon: Bounds, work: Bounds, w: i32, h: i32, margin: i32) -> (i32, i32) {
    match taskbar_edge(icon, work) {
        // A horizontal taskbar is what `above_icon` already handles, including
        // its own top/bottom flip.
        TaskbarEdge::Bottom | TaskbarEdge::Top => above_icon(icon, work, w, h, margin),
        edge => {
            let x = match edge {
                TaskbarEdge::Left => work.left + margin,
                _ => work.right - margin - w,
            };
            // Vertically centred on the icon, then pulled inside the work area.
            let centre_y = (icon.top + icon.bottom) / 2;
            let mut y = centre_y - h / 2;
            if y + h > work.bottom {
                y = work.bottom - h;
            }
            if y < work.top {
                y = work.top;
            }
            (x, y)
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p headset-tray --lib win32::place`
Expected: 12 passed.

- [ ] **Step 5: Use it**

In `panel.rs`, change the final line of `anchor`:

```rust
    place::beside_icon(place::Bounds::from_rect(icon), work, w, h, 8)
```

`reanchor_bottom` is unchanged: holding the bottom edge is still right for a horizontal taskbar, and for a side taskbar the panel's height changing should still grow it upward.

- [ ] **Step 6: Run the gate**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```
Expected: clean.

- [ ] **Step 7: Verify by hand**

1. Right-click the taskbar → Taskbar settings → Taskbar alignment / position → **Left**. (On Windows 11 this may require the registry `Settings` value under `HKCU\...\StuckRects3`, or use a Windows 10 machine; if the taskbar cannot be moved on the test machine, say so in the commit message rather than claiming it was verified.)
2. Click the tray icon. The panel must appear beside the taskbar, fully on screen, not over it.
3. Return the taskbar to the bottom and confirm nothing about the common case changed.

- [ ] **Step 8: Commit**

```bash
git add crates/headset-tray/src/win32/place.rs crates/headset-tray/src/win32/panel.rs
git commit -m "fix(tray): place the panel clear of a side-docked taskbar"
```

---

### Task 4: Stay readable in high contrast

**Problem being fixed:** `ui/theme.rs` is a single fixed dark palette sampled from the mockups, applied unconditionally. In Windows high-contrast mode the panel keeps rendering its own low-contrast greys, which is an accessibility failure rather than a matter of taste — high contrast is chosen by users who cannot read the default.

**Scope decision, stated rather than assumed:** this task covers **high contrast only**, not a light theme. A light palette would need to be designed and sampled, and there are no light mockups to sample from; inventing one would contradict the rule at the top of `theme.rs` that every value there was measured. High contrast avoids that problem entirely by taking its colours from the system, which is what high contrast is *for*.

**Files:**
- Modify: `crates/headset-tray/src/ui/theme.rs` (the palette accessor and its tests)
- Modify: `crates/headset-tray/src/ui/layout.rs` and `crates/headset-tray/src/ui/render.rs` (read through the accessor)
- Modify: `crates/headset-tray/src/win32/mod.rs` (detect the setting, repaint on change)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub struct Palette { … }` with one field per existing colour constant, `pub fn palette() -> Palette`, `pub fn set_high_contrast(on: bool)`, and `pub fn high_contrast_palette() -> Palette`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `theme.rs`:

```rust
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

        let luminance = |c: Color| {
            0.2126 * (c.0 as f32) + 0.7152 * (c.1 as f32) + 0.0722 * (c.2 as f32)
        };
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
    fn high_contrast_does_not_disturb_the_sampled_palette() {
        // The constants are still the record of what was measured from the
        // mockups; high contrast selects a different palette rather than
        // editing them.
        set_high_contrast(true);
        assert_eq!(BG_PANEL, Color::rgb(0x131623));
        set_high_contrast(false);
        assert_eq!(palette().bg_panel, BG_PANEL);
    }
```

- [ ] **Step 2: Add the signatures and run the tests to verify they fail**

```rust
/// Every colour the panel draws with, chosen for the current accessibility
/// settings. `layout` and `render` read colours from here rather than from the
/// constants, so a second palette needs no changes at the call sites.
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
}

pub fn palette() -> Palette { unimplemented!() }
pub fn high_contrast_palette() -> Palette { unimplemented!() }
pub fn set_high_contrast(on: bool) { unimplemented!() }
```

Run: `cargo test -p headset-tray --lib ui::theme`
Expected: the 3 new tests FAIL with `not implemented`; the existing palette tests still pass.

- [ ] **Step 3: Implement the palettes**

```rust
/// Set once at startup and whenever Windows reports a settings change. An
/// atomic rather than a lock: it is read on every primitive during a paint and
/// written approximately never.
static HIGH_CONTRAST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_high_contrast(on: bool) {
    HIGH_CONTRAST.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn palette() -> Palette {
    if HIGH_CONTRAST.load(std::sync::atomic::Ordering::Relaxed) {
        high_contrast_palette()
    } else {
        sampled_palette()
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
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p headset-tray --lib ui::theme`
Expected: all pass. If the luminance test fails, the palette is wrong — raise the separation rather than lowering the threshold.

- [ ] **Step 5: Read colours through the accessor**

In `ui/layout.rs` and `ui/render.rs`, replace direct uses of the colour constants with `palette()` fields. Bind it once per function that draws — `let p = palette();` — rather than calling it per primitive.

Do this mechanically, one file at a time, running `cargo test -p headset-tray` after each. The existing layout tests compare positions, not colours, so they must stay green throughout; a failure means a structural mistake, not a palette one.

- [ ] **Step 6: Detect the setting and react to changes**

In `win32/mod.rs`, add:

```rust
/// Whether Windows is in high-contrast mode.
fn high_contrast_enabled() -> bool {
    use windows::Win32::UI::Accessibility::HIGHCONTRASTW;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    const HCF_HIGHCONTRASTON: u32 = 0x0000_0001;

    unsafe {
        let mut hc = HIGHCONTRASTW {
            cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
            ..Default::default()
        };
        let ok = SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            std::mem::size_of::<HIGHCONTRASTW>() as u32,
            Some(&mut hc as *mut HIGHCONTRASTW as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        ok.is_ok() && (hc.dwFlags & HCF_HIGHCONTRASTON) != 0
    }
}
```

Add `"Win32_UI_Accessibility"` to the `windows` feature list in `Cargo.toml`, with a comment matching the style of the existing entries.

Call it once during `run_ui_with`, before the first paint:

```rust
        crate::ui::theme::set_high_contrast(high_contrast_enabled());
```

And react to changes in `wndproc`, before the fallback arm:

```rust
// The user turned high contrast on or off while we were running.
WM_SETTINGCHANGE => {
    crate::ui::theme::set_high_contrast(high_contrast_enabled());
    with_ctx(|ctx| {
        if ctx.panel_visible {
            redraw_panel(ctx);
        }
    });
    LRESULT(0)
}
```

Add `WM_SETTINGCHANGE` to the `windows::Win32::UI::WindowsAndMessaging` import list.

- [ ] **Step 7: Run the gate and re-render the fixtures**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
.\target\release\headset-tray.exe --render-panel .\scratch\panel
```
Expected: clean, and the rendered PNGs identical to before — with high contrast off, the palette is byte-for-byte the sampled one, so any visual change means Step 5 introduced a mistake.

- [ ] **Step 8: Verify by hand**

1. With the tray running and the panel open, press `Left Alt + Left Shift + Print Screen` (or Settings → Accessibility → Contrast themes) to turn high contrast on.
2. The panel must repaint to the high-contrast palette **without being reopened**.
3. Confirm every label is legible and the active noise segment is still distinguishable from the inactive two.
4. Turn high contrast off. The panel must return to the mockup palette.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml crates/headset-tray/src/ui/theme.rs crates/headset-tray/src/ui/layout.rs crates/headset-tray/src/ui/render.rs crates/headset-tray/src/win32/mod.rs
git commit -m "feat(tray): stay legible when Windows is in high contrast"
```

---

### Task 5: Record the accessibility trade-off

**Problem being addressed:** the panel is a custom-drawn layered window, so Windows sees one opaque rectangle: no UI Automation tree, no screen-reader output, no keyboard navigation inside the panel. Fixing that properly means replacing the renderer with real controls and giving up the pixel-exact mockup match the whole Phase 3 design was built around. That is not a change to make silently in either direction — so this task records the decision rather than making a code change.

**Files:**
- Modify: `docs/superpowers/specs/2026-08-02-phase3-panel-ui-design.md`

- [ ] **Step 1: Append the decision record**

Add at the end of the spec, after the noise-control addendum:

```markdown
## Accessibility: an accepted trade-off (recorded 2026-08-02)

The panel is a custom-drawn layered window. Windows therefore sees a single
bitmap: there is **no UI Automation tree**, so screen readers announce nothing,
there is no keyboard navigation between controls inside the panel, and no focus
indicator.

This is a consequence of the decision at the top of this document — that the
mockups are the source of truth and appearance should match them 1:1. A panel
built from real controls, or from XAML islands as EarTrumpet uses, would get an
accessibility tree, keyboard navigation, and high-contrast support from the
platform, at the cost of the pixel fidelity this design exists to achieve.

**The trade-off is accepted, with mitigations:**

- Every destructive or essential action — Refresh, Exit — is also on the
  right-click menu, which is a standard `TrackPopupMenu` and is fully keyboard
  and screen-reader accessible.
- The notification icon itself answers the keyboard (`NIN_KEYSELECT`).
- Every setting the panel exposes is also reachable from `headsetctl`, which is
  a console application and accessible by construction.
- High contrast is honoured by the palette, so the panel does not become
  unreadable for the users most likely to need it.

**Revisit this if** a user needs screen-reader access to the panel, or if the
panel grows controls that have no equivalent in the right-click menu or the
CLI. The rebuild is large and should be planned as its own phase, not
retrofitted.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/specs/2026-08-02-phase3-panel-ui-design.md
git commit -m "docs(spec): record the panel's accessibility trade-off and its mitigations"
```

---

## Done when

- The tray icon opens the panel from the keyboard (`Win+B`, arrow, Enter) and its context menu from the Menu key.
- Notification version 4 survives an Explorer restart.
- A pin-to-taskbar choice survives exit, relaunch, and `--install`, and an icon still appears if the GUID is refused.
- The panel appears clear of a left- or right-docked taskbar.
- Turning high contrast on repaints the open panel legibly, and turning it off restores the sampled palette byte-for-byte.
- The accessibility trade-off is recorded in the Phase 3 spec.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace --release` are all clean.
