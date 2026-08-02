# Tray Shell Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the five tray defects that make the app misbehave today — a vanishing icon after an Explorer restart, wrong panel placement on multi-monitor, a blurry panel on scaled displays, duplicate instances, and noise clicks with no visual feedback.

**Architecture:** The recurring cause is OS geometry being queried and reasoned about in the same `unsafe` breath, so none of it is testable. Task 1 splits placement into a pure `win32::place` module that the remaining geometry work builds on; Tasks 2–4 are shell-lifecycle fixes with manual verification steps; Task 5 mirrors the slider's existing local-preview pattern.

**Tech Stack:** Rust 1.97.1, `x86_64-pc-windows-gnu`, `windows` crate 0.58 (features only — no new crates), Direct2D/DirectWrite, `cargo test` + the `--render-panel` harness.

## Global Constraints

- **No new crates.** Additional `windows` *features* are permitted and have precedent in `Cargo.toml`; a version bump is not.
- **`windows` stays pinned at 0.58.** 0.59+ uses `raw-dylib`, which needs `dlltool.exe`, which the `x86_64-pc-windows-gnu` toolchain here does not have. See the comment in `Cargo.toml`.
- **The target is `x86_64-pc-windows-gnu`, not MSVC.** Any approach requiring `/MANIFEST:EMBED` or an MSVC-only linker flag is out.
- **`panic = "abort"` in release.** A panic inside a window procedure kills the process; unwinding across the FFI boundary is not an option.
- **`unsafe` lives only in `crates/headset-tray/src/win32/`.** Every other module is safe Rust. Pure logic extracted from `unsafe` code belongs in a module with tests.
- **TDD.** Anything that can be a pure function gets a failing test first. OS-lifecycle behaviour that cannot be unit-tested gets an explicit manual verification step, and the plan says so rather than pretending otherwise.
- **The `CONTRIBUTING.md` gate must pass before every commit:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace --release`.

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/headset-tray/src/win32/place.rs` | **New.** Pure placement geometry: work-area clamping, above-icon placement, bottom-edge holding. No OS calls, fully unit-tested. | 1 |
| `crates/headset-tray/src/win32/panel.rs` | Queries the OS for the icon rect and the *correct monitor's* work area, then delegates the arithmetic to `place`. | 1, 3 |
| `crates/headset-tray/src/win32/mod.rs` | Window procedure: Explorer-restart re-add, DPI scale in `Ctx`, hit-test coordinate conversion, pending-noise feedback. | 2, 3, 5 |
| `crates/headset-tray/src/win32/dpi.rs` | **New.** Process DPI awareness opt-in and the pure `scale_for_dpi` conversion. | 3 |
| `crates/headset-tray/src/state.rs` | `with_pending_noise` — the pure state override the panel renders while a write is in flight. | 5 |
| `crates/headset-tray/src/main.rs` | Single-instance guard before anything else starts. | 4 |
| `Cargo.toml` | Adds the `Win32_UI_HiDpi` feature. | 3 |

---

### Task 1: Pure placement geometry, and the correct monitor

**Problem being fixed:** `panel.rs:165` and `panel.rs:211` both call `SystemParametersInfoW(SPI_GETWORKAREA)`, which returns the **primary** display's work area only. On any multi-monitor setup the panel is clamped against the wrong rectangle. Negative coordinates — a monitor to the left of the primary — are the case that breaks most visibly.

**Files:**
- Create: `crates/headset-tray/src/win32/place.rs`
- Modify: `crates/headset-tray/src/win32/panel.rs` (replace the bodies of `anchor` and `reanchor_bottom`)
- Modify: `crates/headset-tray/src/win32/mod.rs:50` (declare the new module)
- Test: `crates/headset-tray/src/win32/place.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub struct Bounds { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }` with `pub fn width(&self) -> i32`, `pub fn height(&self) -> i32`, and `pub fn from_rect(r: RECT) -> Bounds`
  - `pub fn above_icon(icon: Bounds, work: Bounds, w: i32, h: i32, margin: i32) -> (i32, i32)`
  - `pub fn hold_bottom(current: Bounds, work: Bounds, h: i32) -> (i32, i32)`
  - Task 3 calls `above_icon` unchanged; the Plan B taskbar-edge task extends this module.

- [ ] **Step 1: Write the failing tests**

Create `crates/headset-tray/src/win32/place.rs` containing only this test module plus the type and function signatures with `unimplemented!()` bodies (see Step 3 for the real bodies — write the signatures now so the tests compile and fail at runtime rather than at compile time):

```rust
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
        Bounds { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
    }
    pub fn width(&self) -> i32 { self.right - self.left }
    pub fn height(&self) -> i32 { self.bottom - self.top }
}

/// Top-left for a `w` x `h` panel sitting just above `icon`, clamped to `work`.
pub fn above_icon(icon: Bounds, work: Bounds, w: i32, h: i32, margin: i32) -> (i32, i32) {
    unimplemented!()
}

/// Top-left for a panel whose height changed to `h`, holding its bottom edge.
pub fn hold_bottom(current: Bounds, work: Bounds, h: i32) -> (i32, i32) {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1920x1080 primary, taskbar 48 px tall at the bottom.
    fn primary() -> Bounds {
        Bounds { left: 0, top: 0, right: 1920, bottom: 1032 }
    }

    /// A tray icon near the right-hand end of that taskbar.
    fn icon() -> Bounds {
        Bounds { left: 1700, top: 1032, right: 1724, bottom: 1056 }
    }

    #[test]
    fn the_panel_sits_above_the_icon_and_centred_on_it() {
        let (x, y) = above_icon(icon(), primary(), 342, 500, 8);
        assert_eq!(x, 1712 - 342 / 2, "centred on the icon");
        assert_eq!(y, 1032 - 500 - 8, "bottom edge one margin above the icon");
    }

    #[test]
    fn a_panel_running_off_the_right_edge_is_pulled_back_in() {
        let narrow = Bounds { left: 1890, top: 1032, right: 1914, bottom: 1056 };
        let (x, _) = above_icon(narrow, primary(), 342, 500, 8);
        assert_eq!(x, 1920 - 342, "flush with the work-area right edge");
    }

    #[test]
    fn a_secondary_monitor_to_the_left_keeps_its_negative_coordinates() {
        // The bug this whole module exists for: SPI_GETWORKAREA reports the
        // primary monitor, so a panel on a left-hand secondary was clamped to
        // x >= 0 and jumped across to the primary display.
        let left_monitor = Bounds { left: -1920, top: 0, right: 0, bottom: 1032 };
        let left_icon = Bounds { left: -220, top: 1032, right: -196, bottom: 1056 };
        let (x, y) = above_icon(left_icon, left_monitor, 342, 500, 8);
        assert!(x < 0, "panel must stay on the left-hand monitor, got x = {x}");
        assert!(x >= -1920, "and inside it, got x = {x}");
        assert_eq!(y, 1032 - 500 - 8);
    }

    #[test]
    fn a_taskbar_at_the_top_puts_the_panel_below_the_icon() {
        let work = Bounds { left: 0, top: 48, right: 1920, bottom: 1080 };
        let top_icon = Bounds { left: 1700, top: 24, right: 1724, bottom: 48 };
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
        let current = Bounds { left: 1541, top: 524, right: 1883, bottom: 1024 };
        let (x, y) = hold_bottom(current, primary(), 620);
        assert_eq!(x, 1541, "horizontal position is untouched");
        assert_eq!(y, 1024 - 620, "grew upward, bottom edge still at 1024");
    }

    #[test]
    fn holding_the_bottom_still_respects_the_work_area() {
        let current = Bounds { left: 100, top: 900, right: 442, bottom: 1024 };
        let (_, y) = hold_bottom(current, primary(), 4000);
        assert_eq!(y, 0, "a panel taller than the screen starts at the top");
    }
}
```

- [ ] **Step 2: Declare the module and run the tests to verify they fail**

Add to `crates/headset-tray/src/win32/mod.rs`, next to the existing `pub(crate) mod panel;` at line 50:

```rust
pub(crate) mod place;
```

Run: `cargo test -p headset-tray --lib win32::place`
Expected: 7 tests FAIL, every one panicking with `not implemented`. If any test passes, the test is not exercising the function — fix the test before continuing.

- [ ] **Step 3: Implement the two functions**

Replace the two `unimplemented!()` bodies in `place.rs`:

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p headset-tray --lib win32::place`
Expected: 7 passed.

- [ ] **Step 5: Point the OS layer at the right monitor**

In `crates/headset-tray/src/win32/panel.rs`, add this helper and rewrite both public functions to use it. Note `MonitorFromRect` with `MONITOR_DEFAULTTONEAREST`, which is what makes the work area belong to the monitor the icon is actually on:

```rust
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
```

Rewrite `anchor` to end with the shared arithmetic. Keep the existing `Shell_NotifyIconGetRect` / `GetCursorPos` fallback logic; only the work-area lookup and the final clamp change:

```rust
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
            RECT { left: pt.x, top: pt.y, right: pt.x, bottom: pt.y }
        }
    };
    let work = work_area_for(icon);
    place::above_icon(place::Bounds::from_rect(icon), work, w, h, 8)
}
```

Rewrite `reanchor_bottom` the same way:

```rust
pub unsafe fn reanchor_bottom(hwnd: HWND, h: i32) -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut r = RECT::default();
    let _ = GetWindowRect(hwnd, &mut r);
    let work = work_area_for(r);
    place::hold_bottom(place::Bounds::from_rect(r), work, h)
}
```

Add `use crate::win32::place;` to the imports at the top of `panel.rs`, and delete the now-unused `SystemParametersInfoW` / `SPI_GETWORKAREA` / `SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS` imports from both function bodies.

- [ ] **Step 6: Run the full gate**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: clean. Clippy will catch the unused imports if any were missed.

- [ ] **Step 7: Verify by hand on a second monitor**

If a second monitor is available: move the taskbar to it (Settings → System → Display → "Show taskbar on all displays", then right-click the taskbar on the secondary), click the tray icon, confirm the panel appears above the icon on that monitor rather than jumping to the primary. Record the result in the commit message. If no second monitor is available, say so in the commit message rather than implying it was tested.

- [ ] **Step 8: Commit**

```bash
git add crates/headset-tray/src/win32/place.rs crates/headset-tray/src/win32/panel.rs crates/headset-tray/src/win32/mod.rs
git commit -m "fix(tray): place the panel on the monitor the icon is actually on"
```

---

### Task 2: Survive an Explorer restart

**Problem being fixed:** `win32/mod.rs:261` has no `TaskbarCreated` case. When Explorer restarts — a crash, a shell update, an `explorer.exe` kill — every tray icon is destroyed and the shell broadcasts `TaskbarCreated` to tell applications to re-register. Without it, the icon is gone until the user relaunches the app, while the process keeps running invisibly.

**Files:**
- Modify: `crates/headset-tray/src/win32/mod.rs` (module statics, `run_ui_with`, `wndproc`)

**Interfaces:**
- Consumes: nothing.
- Produces: `fn readd_icon(ctx: &mut Ctx)` — Plan B's `NIM_SETVERSION` task must extend this function too, or the version reverts to the legacy default after every shell restart.

- [ ] **Step 1: Add the registered-message static**

`RegisterWindowMessageW` returns a value in the `0xC000..=0xFFFF` range, decided by the shell at runtime, so it cannot be a `const` and must be compared at dispatch time. Add near the other message constants at the top of `win32/mod.rs`:

```rust
/// The shell's "I have restarted, re-add your icon" broadcast.
///
/// Registered at runtime rather than being a constant: `RegisterWindowMessageW`
/// allocates the value, and every process that registers the same string gets
/// the same number. Zero means registration failed, which must never match an
/// incoming message.
static TASKBAR_CREATED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
```

- [ ] **Step 2: Register the message during startup**

In `run_ui_with`, immediately after the two `RegisterClassW` calls succeed and **before** `Shell_NotifyIconW(NIM_ADD, ...)` at line 196:

```rust
// Registered before the icon is added: if the shell restarts between the
// two, the broadcast still has somewhere to land.
TASKBAR_CREATED.store(
    RegisterWindowMessageW(w!("TaskbarCreated")),
    std::sync::atomic::Ordering::Relaxed,
);
```

Add `RegisterWindowMessageW` to the `windows::Win32::UI::WindowsAndMessaging` import list.

- [ ] **Step 3: Add the re-add helper**

Add next to `refresh_tray` in `win32/mod.rs`:

```rust
/// Re-registers the notification icon after the shell restarted.
///
/// The previous icon was destroyed with the old taskbar, so this is `NIM_ADD`
/// and not `NIM_MODIFY`; modifying an icon the shell no longer knows about
/// fails silently and leaves the tray empty.
fn readd_icon(ctx: &mut Ctx) {
    unsafe {
        // Delete first, ignoring failure. If the shell somehow does still hold
        // the icon, adding a second one would leave a duplicate that no message
        // ever reaches.
        let _ = Shell_NotifyIconW(NIM_DELETE, &ctx.nid);
        if Shell_NotifyIconW(NIM_ADD, &ctx.nid).as_bool() {
            tracing::info!("re-added the tray icon after a shell restart");
        } else {
            tracing::error!("could not re-add the tray icon after a shell restart");
        }
    }
    refresh_tray(ctx);
}
```

- [ ] **Step 4: Dispatch the broadcast**

In `wndproc`, add this arm **before** the final `_ => DefWindowProcW(...)`. It must be a guard arm, since the value is not known at compile time:

```rust
// The shell restarted and took every tray icon with it. Re-register, and
// drop the panel: it was anchored to an icon that no longer exists.
m if m != 0 && m == TASKBAR_CREATED.load(std::sync::atomic::Ordering::Relaxed) => {
    with_ctx(|ctx| {
        if ctx.panel_visible {
            hide_panel(ctx);
        }
        readd_icon(ctx);
    });
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

- [ ] **Step 6: Verify by hand — this is the only real test**

No unit test can cover this; the behaviour lives entirely in the shell. Run it:

1. `cargo run --release -p headset-tray` (or launch the installed copy).
2. Confirm the tray icon is present.
3. Open Task Manager → Details → `explorer.exe` → End task. The taskbar disappears.
4. Task Manager → Run new task → `explorer.exe` → OK. The taskbar returns.
5. **Confirm the headset icon came back with it**, and that clicking it still opens the panel in the right place.
6. Repeat once more — a second restart catches a re-add that only works the first time.

Record the outcome in the commit message.

- [ ] **Step 7: Commit**

```bash
git add crates/headset-tray/src/win32/mod.rs
git commit -m "fix(tray): re-add the icon when Explorer restarts"
```

---

### Task 3: DPI awareness

**Problem being fixed:** Both render call sites hardcode the scale — `win32/mod.rs:342` and `main.rs:217` pass `1.0` — and the process has no DPI awareness declaration, so Windows treats it as DPI-unaware and bitmap-stretches the whole panel. On a 150% display that is a visibly soft panel with soft text. The renderer already accepts the scale and feeds it to Direct2D (`render.rs:87`); nothing has ever passed anything but `1.0`.

**The manifest route is unavailable** on this toolchain: `x86_64-pc-windows-gnu` has no `/MANIFEST:EMBED`. Use `SetProcessDpiAwarenessContext`, which has been available since Windows 10 1703 and must be called before any window is created.

**A consequence that must not be missed:** once the process is per-monitor aware, mouse coordinates arrive in *physical* pixels while `ui::layout`'s hit regions are in *logical* units. Hit-testing must divide by the scale or every click will land in the wrong place on a scaled display.

**Files:**
- Create: `crates/headset-tray/src/win32/dpi.rs`
- Modify: `Cargo.toml` (workspace `windows` features)
- Modify: `crates/headset-tray/src/win32/mod.rs` (`Ctx`, `run_ui_with`, `redraw_panel`, `on_panel_press`, `on_panel_drag`, `wndproc`)
- Test: `crates/headset-tray/src/win32/dpi.rs`

**Interfaces:**
- Consumes: `place::above_icon` and `place::hold_bottom` from Task 1, unchanged — they already work in physical pixels.
- Produces: `pub fn scale_for_dpi(dpi: u32) -> f32`, `pub unsafe fn make_process_per_monitor_aware()`, `pub unsafe fn window_scale(hwnd: HWND) -> f32`, and `Ctx.scale: f32`.

- [ ] **Step 1: Add the `Win32_UI_HiDpi` feature**

In the root `Cargo.toml`, in the `windows` feature list, after the Phase 3 block:

```toml
    # Phase 4: DPI awareness. A feature of the pinned 0.58, not a crate. The
    # manifest route is unavailable on x86_64-pc-windows-gnu, so the process
    # opts in at runtime through SetProcessDpiAwarenessContext.
    "Win32_UI_HiDpi",
```

Run: `cargo build -p headset-tray`
Expected: builds clean. If it fails, stop — the pin in `Cargo.toml` must not be changed to make this work.

- [ ] **Step 2: Write the failing test**

Create `crates/headset-tray/src/win32/dpi.rs`. Write the test module plus signatures with `unimplemented!()` bodies for the pure function:

```rust
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
    unimplemented!()
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
```

- [ ] **Step 3: Declare the module and run the test to verify it fails**

Add to `win32/mod.rs` next to the other submodule declarations:

```rust
pub(crate) mod dpi;
```

Run: `cargo test -p headset-tray --lib win32::dpi`
Expected: 2 tests FAIL with `not implemented`.

- [ ] **Step 4: Implement the pure function**

```rust
pub fn scale_for_dpi(dpi: u32) -> f32 {
    if dpi == 0 {
        return 1.0;
    }
    dpi as f32 / BASE_DPI
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p headset-tray --lib win32::dpi`
Expected: 2 passed.

- [ ] **Step 6: Add the two OS-touching helpers**

Append to `dpi.rs`:

```rust
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
```

- [ ] **Step 7: Opt in at startup**

In `run_ui_with`, make this the **first** statement inside the `unsafe` block — before `CoInitializeEx`, before `GetModuleHandleW`, and well before any `CreateWindowExW`:

```rust
// Before any window exists: the awareness context is fixed at first use.
dpi::make_process_per_monitor_aware();
```

- [ ] **Step 8: Carry the scale on `Ctx` and use it when rendering**

Add the field to `struct Ctx`, after `renderer`:

```rust
    /// Render scale for the display the panel is on. Mouse coordinates arrive
    /// in physical pixels and `ui::layout` works in logical ones, so this is
    /// also the divisor for hit-testing.
    scale: f32,
```

Initialise it in the `Ctx` construction at `win32/mod.rs:212`, alongside `level_track: None`:

```rust
                scale: 1.0,
```

In `redraw_panel`, replace the hardcoded scale. The scale is re-read on every repaint so that dragging the panel's monitor-to-monitor case is handled without extra bookkeeping:

```rust
    ctx.scale = unsafe { dpi::window_scale(ctx.panel_hwnd) };
    let img = match renderer.render(&panel, ctx.scale) {
```

- [ ] **Step 9: Convert mouse coordinates back to logical units**

This is the step that breaks clicking if it is skipped. In `on_panel_press`, replace the coordinate conversion at the top:

```rust
fn on_panel_press(ctx: &mut Ctx, x: f32, y: f32) {
    // Physical pixels in, logical units out: the window is sized in physical
    // pixels but every hit region came from `ui::layout`, which works in the
    // same logical units as the theme's metrics.
    let s = if ctx.scale > 0.0 { ctx.scale } else { 1.0 };
    let (lx, ly) = (x / s - crate::ui::theme::SHADOW, y / s - crate::ui::theme::SHADOW);
```

And in `on_panel_drag`:

```rust
    let s = if ctx.scale > 0.0 { ctx.scale } else { 1.0 };
    let v = g.value_at(x / s - crate::ui::theme::SHADOW);
```

- [ ] **Step 10: Repaint when the panel changes display**

Add to `wndproc`, before the fallback arm. `WM_DPICHANGED` is `0x02E0`:

```rust
// Dragged onto a display with a different scale, or the user changed the
// scaling while the panel was open. Re-render at the new density.
WM_DPICHANGED if panel::tag(hwnd) == panel::TAG_PANEL => {
    with_ctx(redraw_panel);
    LRESULT(0)
}
```

Add `WM_DPICHANGED` to the `windows::Win32::UI::WindowsAndMessaging` import list.

- [ ] **Step 11: Run the gate**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```
Expected: clean.

- [ ] **Step 12: Verify by hand at two scale factors**

1. Set the display to 100% (Settings → System → Display → Scale). Launch the tray, open the panel, click each slider and each noise segment. Confirm the click lands where the pointer is.
2. Set the display to 150%. Sign out and back in if Windows asks. Launch again.
3. Confirm the panel is **sharp**, not stretched, and that clicking a noise segment still activates the segment under the pointer rather than one to its left.

Step 3 is the regression that Step 9 exists to prevent; if clicks are offset, `ctx.scale` is not reaching the hit test.

- [ ] **Step 13: Commit**

```bash
git add Cargo.toml crates/headset-tray/src/win32/dpi.rs crates/headset-tray/src/win32/mod.rs
git commit -m "fix(tray): render the panel at the display's real pixel density"
```

---

### Task 4: Single-instance guard

**Problem being fixed:** `run_tray` (`main.rs:232`) adds a notification icon unconditionally. Launching the tray twice — double-clicking the exe while the installed copy is already running at startup, which `--install` makes easy — leaves two icons, two worker threads, and two `ControlSession`s contending for one device.

**Files:**
- Modify: `crates/headset-tray/src/win32/mod.rs` (the guard, and a message to raise the existing panel)
- Modify: `crates/headset-tray/src/main.rs` (`run_tray`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn claim_single_instance() -> SingleInstance`, `pub enum SingleInstance { Claimed(OwnedMutex), AlreadyRunning }`, and `pub const WM_SHOW_PANEL: u32` (`WM_APP + 3`).

- [ ] **Step 1: Add the message and the guard**

`Win32_System_Threading` is already an enabled feature, so `CreateMutexW` needs no manifest change. Add to `win32/mod.rs`, next to `WM_STATE`:

```rust
/// Posted by a second instance to ask the first to show itself.
pub const WM_SHOW_PANEL: u32 = WM_APP + 3;
```

Then the guard itself:

```rust
/// Holds the single-instance mutex for the life of the process.
///
/// Dropping this releases the claim, so it must be bound to a named local for
/// the whole run — `let _ = claim_single_instance()` would drop it immediately
/// and let a second instance straight in.
pub struct OwnedMutex(windows::Win32::Foundation::HANDLE);

impl Drop for OwnedMutex {
    fn drop(&mut self) {
        // May be null: the "could not create a mutex" branch below still hands
        // back an OwnedMutex so the caller has one code path, and closing a
        // null handle is an error rather than a no-op.
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

pub enum SingleInstance {
    Claimed(OwnedMutex),
    AlreadyRunning,
}

/// Claims the right to be the one running tray for this user session.
///
/// `Local\` rather than `Global\`: the tray is per-user and per-session by
/// design — it installs to `%LOCALAPPDATA%` and writes only to `HKEY_CURRENT_USER`
/// — and `Global\` would additionally block a second user on the same machine
/// from running their own.
pub fn claim_single_instance() -> SingleInstance {
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    unsafe {
        match CreateMutexW(None, TRUE, w!("Local\\HeadsetTray.SingleInstance")) {
            // The handle is returned even when the mutex already existed, so
            // the error code is what distinguishes the two, not the result.
            Ok(h) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    let _ = CloseHandle(h);
                    SingleInstance::AlreadyRunning
                } else {
                    SingleInstance::Claimed(OwnedMutex(h))
                }
            }
            // Without a mutex there is no way to tell. Starting is the less
            // annoying failure: a duplicate icon beats refusing to run.
            Err(e) => {
                tracing::warn!("single-instance mutex unavailable: {e}");
                SingleInstance::Claimed(OwnedMutex(windows::Win32::Foundation::HANDLE::default()))
            }
        }
    }
}

/// Asks an already-running tray to show its panel, so a second launch does
/// something useful instead of nothing.
pub fn signal_existing_instance() {
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    unsafe {
        let hwnd = FindWindowW(w!("HeadsetTrayWindow"), PCWSTR::null());
        if let Ok(h) = hwnd {
            let _ = PostMessageW(h, WM_SHOW_PANEL, WPARAM(0), LPARAM(0));
        }
    }
}
```

Note: `install.rs:138` already finds this window by the same class name; the string `"HeadsetTrayWindow"` must match the one passed to `RegisterClassW` in `run_ui_with`.

- [ ] **Step 2: Handle the raise message**

In `wndproc`, before the fallback arm:

```rust
// A second instance asked us to show ourselves rather than starting a
// duplicate tray.
WM_SHOW_PANEL => {
    with_ctx(|ctx| {
        if !ctx.panel_visible {
            toggle_panel(ctx);
        }
    });
    LRESULT(0)
}
```

- [ ] **Step 3: Use the guard before anything else starts**

In `main.rs`, make this the first thing `run_tray` does after `tracing_subscriber_init()` — before `tidy_previous_upgrade`, before the channel, before the worker thread, so a second instance never opens a `ControlSession`:

```rust
    // Bound to a named local: this must outlive the message loop. `let _ = ...`
    // would drop it here and defeat the whole guard.
    let _instance = match win32::claim_single_instance() {
        win32::SingleInstance::Claimed(guard) => guard,
        win32::SingleInstance::AlreadyRunning => {
            win32::signal_existing_instance();
            return;
        }
    };
```

- [ ] **Step 4: Run the gate**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```
Expected: clean.

- [ ] **Step 5: Verify by hand**

1. Launch `headset-tray.exe`. One icon appears.
2. Launch it again from a second terminal. **Confirm no second icon appears**, and that the first instance's panel opens.
3. Exit the tray from its right-click menu.
4. Launch again. Confirm it starts normally — a mutex that was not released on exit would show up here as the app refusing to start.

Step 4 is the one that catches a leaked handle; do not skip it.

- [ ] **Step 6: Commit**

```bash
git add crates/headset-tray/src/win32/mod.rs crates/headset-tray/src/main.rs
git commit -m "fix(tray): refuse to start a second instance, raise the first instead"
```

---

### Task 5: Immediate feedback on a noise click

**Problem being fixed:** `send_noise` in `win32/mod.rs` posts the command and returns without repainting, so a clicked segment does not move until the worker's read-back posts `WM_STATE` — at least one 250 ms-paced exchange later, and longer if a refresh is already in flight. The slider already solves this with `ctx.drag`, a locally previewed value shown until the device confirms. This mirrors that.

**Files:**
- Modify: `crates/headset-tray/src/state.rs` (the pure override, plus its tests)
- Modify: `crates/headset-tray/src/win32/mod.rs` (`Ctx`, `send_noise`, `redraw_panel`, `wndproc`'s `WM_STATE`, `hide_panel`)

**Interfaces:**
- Consumes: `Ctx.scale` from Task 3 (only because both touch `redraw_panel`; no logical dependency).
- Produces: `HeadsetState::with_pending_noise(&self, pending: Option<NoiseControl>) -> HeadsetState` and `Ctx.pending_noise: Option<NoiseControl>`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/headset-tray/src/state.rs`:

```rust
    #[test]
    fn a_pending_noise_write_is_shown_until_the_device_answers() {
        use headset_protocol::{NoiseControl, NoiseMode};
        let mut s = HeadsetState::default();
        s.apply(&frame(Param::NoiseCancellation.id(), &[0x01, 0x03]));

        let asked = NoiseControl { mode: NoiseMode::Ambient, anc_level: 3 };
        let shown = s.with_pending_noise(Some(asked));
        assert_eq!(shown.noise, Some(asked), "the panel shows what was asked for");
        assert_eq!(
            s.noise.map(|n| n.mode),
            Some(NoiseMode::Anc),
            "the device's own state is not overwritten by the request"
        );
    }

    #[test]
    fn no_pending_write_leaves_the_device_state_alone() {
        let mut s = HeadsetState::default();
        s.apply(&frame(Param::NoiseCancellation.id(), &[0x01, 0x03]));
        assert_eq!(s.with_pending_noise(None), s);
    }

    #[test]
    fn a_pending_write_does_not_invent_a_state_while_disconnected() {
        use headset_protocol::{NoiseControl, NoiseMode};
        // Nothing was ever read, so there is nothing to preview against. The
        // panel must keep showing "--" rather than a value the device never
        // reported and may refuse.
        let s = HeadsetState::default();
        let asked = NoiseControl { mode: NoiseMode::Anc, anc_level: 2 };
        assert_eq!(s.with_pending_noise(Some(asked)).noise, None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p headset-tray --lib state::`
Expected: FAIL to compile with `no method named with_pending_noise`. Add the signature with an `unimplemented!()` body, re-run, and confirm all three now fail at runtime with `not implemented`.

- [ ] **Step 3: Implement it**

Add to `impl HeadsetState` in `state.rs`:

```rust
    /// This state as the panel should draw it while a noise write is in flight.
    ///
    /// The device is still the source of truth — this does not touch `self`,
    /// and the read-back that follows every write is what finally decides. It
    /// exists because a write costs at least one 250 ms-paced exchange, and a
    /// control that does not move when clicked reads as broken.
    ///
    /// A pending write against an unknown state is ignored: there is nothing to
    /// preview against, and the device may refuse the write outright.
    pub fn with_pending_noise(&self, pending: Option<NoiseControl>) -> HeadsetState {
        let mut out = self.clone();
        if self.noise.is_some() {
            if let Some(p) = pending {
                out.noise = Some(p);
            }
        }
        out
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p headset-tray --lib state::`
Expected: all pass.

- [ ] **Step 5: Track the pending write on `Ctx`**

Add the field to `struct Ctx`, next to `drag`:

```rust
    /// Noise state asked for but not yet confirmed. Shown in place of the
    /// device's own until the read-back arrives, exactly as `drag` is for the
    /// slider.
    pending_noise: Option<NoiseControl>,
```

Initialise it as `pending_noise: None,` in the `Ctx` construction.

- [ ] **Step 6: Set it, show it, and clear it**

In `send_noise`, record the request and repaint:

```rust
fn send_noise(ctx: &mut Ctx, f: impl FnOnce(NoiseControl) -> NoiseControl) {
    let current = ctx.state.lock().ok().and_then(|s| s.noise);
    let Some(current) = current else { return };
    let want = f(current);
    let _ = ctx.commands.send(Command::SetNoise(want));
    // Show it immediately. The worker's read-back replaces it with whatever
    // the device actually holds, including when the device refuses.
    ctx.pending_noise = Some(want);
    redraw_panel(ctx);
}
```

In `redraw_panel`, apply the override where the state is cloned:

```rust
    let state = ctx.state.lock().map(|s| s.clone()).unwrap_or_default();
    let state = state.with_pending_noise(ctx.pending_noise);
```

In `wndproc`'s `WM_STATE` arm, clear it before repainting. The worker calls `notify` after every command whether it succeeded or failed, so this always runs and the preview cannot get stuck:

```rust
        WM_STATE => {
            CTX.with(|c| {
                if let Some(ctx) = c.borrow_mut().as_mut() {
                    // The device has spoken; stop showing what was asked for.
                    ctx.pending_noise = None;
                    refresh_tray(ctx);
                    if ctx.panel_visible {
                        redraw_panel(ctx);
                    }
                }
            });
            LRESULT(0)
        }
```

And in `hide_panel`, alongside `ctx.drag = None;`:

```rust
    ctx.pending_noise = None;
```

- [ ] **Step 7: Run the gate**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```
Expected: clean.

- [ ] **Step 8: Verify by hand against the headset**

This is also the first hardware test of the noise feature itself:

1. Launch the tray with the headset on. Confirm the noise row shows the real mode and level.
2. Click `AMBIENT`. The segment should activate **immediately**, then stay activated once the read-back lands.
3. Click `ANC`, then click level 1 and level 4. Same immediacy.
4. Turn the headset off and click a segment. Nothing should be sent — the row does not hit-test while disconnected — and the panel must not show an invented state.
5. Cross-check with `headsetctl noise` that the device agrees with the panel.

- [ ] **Step 9: Commit**

```bash
git add crates/headset-tray/src/state.rs crates/headset-tray/src/win32/mod.rs
git commit -m "fix(tray): show a noise change immediately instead of waiting for the read-back"
```

---

## Done when

- The tray icon returns after two consecutive Explorer restarts.
- The panel opens above its icon on a secondary monitor, including one positioned to the left of the primary.
- The panel is sharp at 150% scaling and clicks land under the pointer.
- A second launch raises the first instance instead of adding an icon.
- A noise segment activates on click rather than a quarter-second later.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace --release` are all clean.
