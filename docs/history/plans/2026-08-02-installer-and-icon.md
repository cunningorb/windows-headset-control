# Installer and Icon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce a double-clickable setup executable that installs the tray with a Start menu entry carrying the headset icon, and publish it as an alpha release.

**Architecture:** The icon drawing becomes a pure size-parameterised function shared by the runtime `HICON` and a hand-written ICO encoder, and the resulting file is embedded in the executable as a resource so shortcuts inherit it. Inno Setup packages the release binaries; a tag-triggered workflow publishes the result.

**Tech Stack:** Rust 1.97 on `x86_64-pc-windows-gnu`, `windows` crate 0.58 (features already enabled), `windres` from the WinLibs toolchain, Inno Setup 6, GitHub Actions.

Design: `docs/history/specs/2026-08-02-installer-and-icon-design.md`.

## Global Constraints

- **No new crates.** The ICO encoder is written by hand for this reason. `windres` and Inno Setup are build tools, not dependencies of the shipped binary.
- **`windows` stays pinned at 0.58.** `IShellLinkW` (`Win32_UI_Shell`) and `IPersistFile` (`Win32_System_Com`) both exist under features already enabled — verified. No feature additions are needed by this plan.
- **Target is `x86_64-pc-windows-gnu`.** `windres.exe` comes from that toolchain.
- **Per-user, no administrator rights.** `PrivilegesRequired=lowest`; nothing is written outside `HKEY_CURRENT_USER` and `%LOCALAPPDATA%`.
- **One owner per registry key.** Only the Inno installer registers in Add/Remove Programs.
- **`unsafe` only in `crates/headset-tray/src/win32/`.** The icon pixel generation and the ICO encoder are safe Rust and live under `ui/`.
- **TDD**, and the `CONTRIBUTING.md` gate passes before every commit.

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/headset-tray/src/ui/icon.rs` | **New.** Pure: the headset shape at any size, and the ICO encoder. | 1, 2 |
| `crates/headset-tray/src/win32/mod.rs` | `build_icon` becomes a wrapper over `icon_pixels(32)`. | 1 |
| `crates/headset-tray/src/main.rs` | `--export-icon` mode. | 2 |
| `crates/headset-tray/assets/headset.ico` | **New, generated and committed.** | 2 |
| `crates/headset-tray/build.rs`, `crates/headset-tray/headset-tray.rc` | **New.** Embed the icon as a resource. | 3 |
| `crates/headset-tray/src/install.rs` | Start menu shortcut; drop the Add/Remove registration. | 4 |
| `installer/headset-tray.iss`, `build-installer.ps1` | **New.** The installer and its local build. | 5 |
| `.github/workflows/release.yml` | **New.** Tag-triggered release. | 6 |
| `Cargo.toml`, `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md` | Version and documentation. | 7 |

---

### Task 1: The headset shape at any size

**Problem being fixed:** `build_icon` in `win32/mod.rs` hardcodes 32×32 — the arc is centred at `(16.0, 19.0)` with radii `10.5..=13.5`, ear cups span `x` 3..=8 and 23..=28 over `y` 15..27. A multi-size icon needs the same shape at 16, 48, 128 and 256, and the drawing lives inside an `unsafe` function that also does COM-adjacent bitmap work.

**Files:**
- Create: `crates/headset-tray/src/ui/icon.rs`
- Modify: `crates/headset-tray/src/ui/mod.rs` (declare the module)
- Modify: `crates/headset-tray/src/win32/mod.rs` (`build_icon`)

**Interfaces:**
- Produces: `pub const FILL: u32`, `pub const OUTLINE: u32`, and
  `pub fn icon_pixels(n: usize) -> Vec<u32>` — straight-alpha BGRA, `n * n` entries, row-major top-down. Task 2 encodes these; `build_icon` wraps them.

- [ ] **Step 1: Write the failing tests**

Create `crates/headset-tray/src/ui/icon.rs` with the constants, the signature returning `unimplemented!()`, and this test module:

```rust
//! The headset icon, as pixels. Pure: no OS, no window, no device.
//!
//! One definition serves three consumers — the tray's runtime `HICON`, the
//! multi-size `.ico` embedded in the executable, and the tests that keep those
//! two from drifting apart.

/// Straight-alpha BGRA. Two opaque colours and full transparency: the shape is
/// not anti-aliased, which is what lets the `.ico` stay small and exact.
pub const FILL: u32 = 0xFF_F0F0F0;
pub const OUTLINE: u32 = 0xFF_1A1A1A;
pub const CLEAR: u32 = 0x00_000000;

// The 32-pixel drawing these came from, divided by 32. At n = 32 they reproduce
// it exactly; at any other size they scale it.
const RING_CX: f32 = 0.5;
const RING_CY: f32 = 0.593_75;
const RING_INNER: f32 = 0.328_125;
const RING_OUTER: f32 = 0.421_875;
const CUP_TOP: f32 = 0.468_75;
const CUP_BOTTOM: f32 = 0.843_75;
const CUP_LEFT_X0: f32 = 0.093_75;
const CUP_LEFT_X1: f32 = 0.281_25;
const CUP_RIGHT_X0: f32 = 0.718_75;
const CUP_RIGHT_X1: f32 = 0.906_25;

pub fn icon_pixels(n: usize) -> Vec<u32> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(px: &[u32], n: usize, x: usize, y: usize) -> u32 {
        px[y * n + x]
    }

    #[test]
    fn every_pixel_is_fill_outline_or_clear() {
        // The ICO encoder relies on there being no partial alpha.
        for n in [16, 32, 48, 128] {
            for p in icon_pixels(n) {
                assert!(
                    p == FILL || p == OUTLINE || p == CLEAR,
                    "size {n} produced an unexpected colour {p:#010x}"
                );
            }
        }
    }

    #[test]
    fn the_shape_is_symmetric_about_the_vertical_axis() {
        // Two ear cups and a centred arc: a left/right asymmetry means a
        // constant was scaled wrongly.
        for n in [16, 32, 48, 128] {
            let px = icon_pixels(n);
            for y in 0..n {
                for x in 0..n {
                    assert_eq!(
                        at(&px, n, x, y),
                        at(&px, n, n - 1 - x, y),
                        "size {n} differs at ({x},{y}) and its mirror"
                    );
                }
            }
        }
    }

    #[test]
    fn the_icon_is_neither_empty_nor_solid_at_every_size() {
        for n in [16, 32, 48, 128, 256] {
            let px = icon_pixels(n);
            assert_eq!(px.len(), n * n, "size {n} produced the wrong pixel count");
            let drawn = px.iter().filter(|p| **p != CLEAR).count();
            assert!(drawn > n * n / 50, "size {n} drew almost nothing: {drawn}px");
            assert!(drawn < n * n / 2, "size {n} drew almost everything: {drawn}px");
        }
    }

    #[test]
    fn the_thirty_two_pixel_rendering_is_pinned() {
        // This is the icon that has been shipping. Changing the drawing must
        // fail here, and the fix is to re-run `--export-icon` and commit the
        // regenerated file along with an updated expectation.
        let px = icon_pixels(32);
        let drawn = px.iter().filter(|p| **p != CLEAR).count();
        let fill = px.iter().filter(|p| **p == FILL).count();
        assert_eq!((drawn, fill), (PINNED_DRAWN, PINNED_FILL));

        // The ear cups sit at the sides, the arc across the top, and the middle
        // of the band area is hollow.
        assert_eq!(at(&px, 32, 5, 20), FILL, "left ear cup");
        assert_eq!(at(&px, 32, 26, 20), FILL, "right ear cup");
        assert_eq!(at(&px, 32, 16, 7), FILL, "top of the headband arc");
        assert_eq!(at(&px, 32, 16, 20), CLEAR, "the middle is open");
    }

    /// Filled in from the first run. See the test above.
    const PINNED_DRAWN: usize = 0;
    const PINNED_FILL: usize = 0;
}
```

- [ ] **Step 2: Declare the module and watch the tests fail**

Add `pub mod icon;` to `crates/headset-tray/src/ui/mod.rs`.

Run: `cargo test -p headset-tray --lib ui::icon`
Expected: all four FAIL with `not implemented`.

- [ ] **Step 3: Implement**

Integer bounds are derived by rounding the fraction times `n`, which reproduces the original 32-pixel bounds exactly (`0.09375 * 32 = 3`, `0.28125 * 32 = 9`, `0.46875 * 32 = 15`, `0.84375 * 32 = 27`) and scales cleanly:

```rust
pub fn icon_pixels(n: usize) -> Vec<u32> {
    let nf = n as f32;
    let bound = |f: f32| (f * nf).round() as i32;
    let (cup_top, cup_bottom) = (bound(CUP_TOP), bound(CUP_BOTTOM));
    let (l0, l1) = (bound(CUP_LEFT_X0), bound(CUP_LEFT_X1));
    let (r0, r1) = (bound(CUP_RIGHT_X0), bound(CUP_RIGHT_X1));

    let mut shape = vec![false; n * n];
    for y in 0..n as i32 {
        for x in 0..n as i32 {
            // Headband: an arc centred low, so only its upper half is drawn.
            let dx = (x as f32 + 0.5) / nf - RING_CX;
            let dy = (y as f32 + 0.5) / nf - RING_CY;
            let r = (dx * dx + dy * dy).sqrt();
            let band = (RING_INNER..=RING_OUTER).contains(&r) && dy < 0.0;

            // Ear cups, with their four outer corners notched off so the ends
            // read as rounded rather than square.
            let in_left = (l0..l1).contains(&x);
            let in_right = (r0..r1).contains(&x);
            let in_rows = (cup_top..cup_bottom).contains(&y);
            let end_row = y == cup_top || y == cup_bottom - 1;
            let end_col = x == l0 || x == l1 - 1 || x == r0 || x == r1 - 1;
            let cup = (in_left || in_right) && in_rows && !(end_row && end_col);

            if band || cup {
                shape[y as usize * n + x as usize] = true;
            }
        }
    }

    // Outline every clear pixel that touches the shape, so the glyph reads on a
    // light taskbar as well as a dark one.
    let mut px = vec![CLEAR; n * n];
    for y in 0..n as i32 {
        for x in 0..n as i32 {
            let i = y as usize * n + x as usize;
            if shape[i] {
                px[i] = FILL;
                continue;
            }
            let touches = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..n as i32).contains(&nx)
                        && (0..n as i32).contains(&ny)
                        && shape[ny as usize * n + nx as usize]
                })
            });
            if touches {
                px[i] = OUTLINE;
            }
        }
    }
    px
}
```

- [ ] **Step 4: Fill in the pinned numbers from the actual output**

Run: `cargo test -p headset-tray --lib the_thirty_two_pixel_rendering_is_pinned`
It fails with the real values in the `left`/`right` of the assertion. Put those into
`PINNED_DRAWN` and `PINNED_FILL`, re-run, and confirm it passes along with the four
positional assertions. If a positional assertion fails, the scaling is wrong — fix the
constants, not the assertion.

- [ ] **Step 5: Point `build_icon` at it**

In `win32/mod.rs`, replace the whole shape-and-pixel body of `build_icon` with a call,
keeping the `CreateBitmap` / `CreateIconIndirect` half unchanged:

```rust
unsafe fn build_icon() -> windows::core::Result<HICON> {
    const N: usize = 32;
    let pixels = crate::ui::icon::icon_pixels(N);

    let color: HBITMAP = CreateBitmap(
        N as i32,
        N as i32,
        1,
        32,
        Some(pixels.as_ptr() as *const std::ffi::c_void),
    );
    // An all-zero mask means "use the colour bitmap's alpha everywhere".
    let mask: HBITMAP = CreateBitmap(N as i32, N as i32, 1, 1, None);
    let info = ICONINFO {
        fIcon: TRUE,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask,
        hbmColor: color,
    };
    let icon = CreateIconIndirect(&info)?;
    let _ = DeleteObject(color);
    let _ = DeleteObject(mask);
    Ok(icon)
}
```

- [ ] **Step 6: Gate and commit**

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

Run the tray briefly and confirm the tray icon still looks like a headset — the pinned test
proves the pixels are identical, but look once anyway.

```bash
git add crates/headset-tray/src/ui/icon.rs crates/headset-tray/src/ui/mod.rs crates/headset-tray/src/win32/mod.rs
git commit -m "refactor(tray): draw the icon at any size from one definition"
```

---

### Task 2: Write the `.ico`

**Problem being fixed:** there is no icon file, so nothing outside the running process can show the headset — not Explorer, not a shortcut.

**Files:**
- Modify: `crates/headset-tray/src/ui/icon.rs` (the encoder and its tests)
- Modify: `crates/headset-tray/src/main.rs` (`--export-icon`)
- Create: `crates/headset-tray/assets/headset.ico` (generated, committed)

**Interfaces:**
- Consumes: `icon_pixels` from Task 1.
- Produces: `pub const ICON_SIZES: [usize; 5]` and `pub fn encode_ico(sizes: &[usize]) -> Vec<u8>`. Task 3 embeds the file; the drift test compares against this function.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `ui/icon.rs`:

```rust
    fn u16_at(b: &[u8], o: usize) -> u16 {
        u16::from_le_bytes([b[o], b[o + 1]])
    }
    fn u32_at(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    }

    #[test]
    fn the_ico_header_declares_an_icon_with_one_entry_per_size() {
        let ico = encode_ico(&ICON_SIZES);
        assert_eq!(u16_at(&ico, 0), 0, "reserved");
        assert_eq!(u16_at(&ico, 2), 1, "type 1 = icon");
        assert_eq!(u16_at(&ico, 4) as usize, ICON_SIZES.len());
    }

    #[test]
    fn every_directory_entry_points_inside_the_file() {
        // A wrong offset or length yields an icon Windows silently refuses to
        // load, which looks exactly like "the icon didn't work" with no error.
        let ico = encode_ico(&ICON_SIZES);
        for (i, size) in ICON_SIZES.iter().enumerate() {
            let e = 6 + i * 16;
            let declared = ico[e] as usize;
            assert_eq!(
                declared,
                if *size == 256 { 0 } else { *size },
                "256 is encoded as 0; {size} was encoded as {declared}"
            );
            assert_eq!(u16_at(&ico, e + 4), 1, "planes");
            assert_eq!(u16_at(&ico, e + 6), 32, "bit count");
            let len = u32_at(&ico, e + 8) as usize;
            let off = u32_at(&ico, e + 12) as usize;
            assert!(off + len <= ico.len(), "entry {i} runs past the end of the file");
            assert_eq!(u32_at(&ico, off), 40, "each image starts with a 40-byte header");
            assert_eq!(u32_at(&ico, off + 4) as usize, *size, "biWidth");
            assert_eq!(
                u32_at(&ico, off + 8) as usize,
                size * 2,
                "biHeight is doubled: colour bitmap plus AND mask"
            );
        }
    }

    #[test]
    fn the_committed_icon_matches_what_the_code_generates() {
        // The one that actually catches a forgotten regeneration.
        let committed = include_bytes!("../../assets/headset.ico");
        assert_eq!(
            committed.as_slice(),
            encode_ico(&ICON_SIZES).as_slice(),
            "assets/headset.ico is stale; regenerate it with \
             `cargo run -p headset-tray -- --export-icon crates/headset-tray/assets/headset.ico`"
        );
    }
```

- [ ] **Step 2: Add the signature and watch them fail**

```rust
/// Sizes Windows asks for: taskbar and tray, Start menu, and Explorer's larger
/// views. 256 costs about 256 KB uncompressed and is what keeps the largest
/// views sharp — see the design note on the size trade-off.
pub const ICON_SIZES: [usize; 5] = [16, 32, 48, 128, 256];

pub fn encode_ico(sizes: &[usize]) -> Vec<u8> {
    unimplemented!()
}
```

Run: `cargo test -p headset-tray --lib ui::icon`
Expected: the two structural tests fail with `not implemented`. The third fails to compile,
because `assets/headset.ico` does not exist yet — create a zero-byte placeholder so it
compiles and fails on content instead:

```powershell
New-Item -ItemType Directory -Force crates\headset-tray\assets | Out-Null
Set-Content -Path crates\headset-tray\assets\headset.ico -Value $null
```

Re-run and confirm all three now fail at runtime.

- [ ] **Step 3: Implement the encoder**

```rust
/// Encodes the icon as a Windows `.ico`.
///
/// The format is a 6-byte header, one 16-byte directory entry per image, then
/// the images. Each image is a DIB whose declared height is **twice** the real
/// height: the format expects a colour bitmap followed by an AND mask. The mask
/// is left all-zero because the 32-bit colour data carries its own alpha.
///
/// Written by hand rather than with an image crate: this is the whole format,
/// and the project takes no dependency it can avoid.
pub fn encode_ico(sizes: &[usize]) -> Vec<u8> {
    const HEADER: usize = 6;
    const ENTRY: usize = 16;
    const DIB_HEADER: usize = 40;

    let mask_stride = |n: usize| n.div_ceil(32) * 4;
    let image_len = |n: usize| DIB_HEADER + n * n * 4 + n * mask_stride(n);

    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());

    let mut offset = HEADER + ENTRY * sizes.len();
    for &n in sizes {
        // 256 does not fit in a byte and is encoded as zero.
        let dim = if n >= 256 { 0u8 } else { n as u8 };
        out.push(dim); // width
        out.push(dim); // height
        out.push(0); // palette size: none
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(image_len(n) as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += image_len(n);
    }

    for &n in sizes {
        let px = icon_pixels(n);

        out.extend_from_slice(&(DIB_HEADER as u32).to_le_bytes()); // biSize
        out.extend_from_slice(&(n as i32).to_le_bytes()); // biWidth
        out.extend_from_slice(&((n * 2) as i32).to_le_bytes()); // biHeight
        out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
        out.extend_from_slice(&0u32.to_le_bytes()); // biCompression: BI_RGB
        out.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
        out.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
        out.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
        out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
        out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

        // Bottom-up, which is what a positive biHeight means.
        for y in (0..n).rev() {
            for x in 0..n {
                out.extend_from_slice(&px[y * n + x].to_le_bytes());
            }
        }
        out.resize(out.len() + n * mask_stride(n), 0);
    }
    out
}
```

- [ ] **Step 4: Add `--export-icon` and generate the file**

In `main.rs`, add the arm to the `match` in `main`:

```rust
        "--export-icon" => run_export_icon(),
```

and the mode itself:

```rust
/// Writes the multi-size icon to a path. Run when the drawing changes; the
/// result is committed and embedded as a resource.
#[cfg(windows)]
fn run_export_icon() {
    use headset_tray::ui::icon::{encode_ico, ICON_SIZES};

    let Some(path) = std::env::args().nth(2) else {
        report("Export icon", "Usage: headset-tray.exe --export-icon <path.ico>");
        return;
    };
    match std::fs::write(&path, encode_ico(&ICON_SIZES)) {
        Ok(()) => report("Export icon", &format!("Wrote {path}")),
        Err(e) => report("Export icon", &format!("Could not write {path}: {e}")),
    }
}
```

Add it to the `--help` text alongside the other modes. Then generate the real file:

```powershell
cargo run -p headset-tray -- --export-icon crates\headset-tray\assets\headset.ico
```

- [ ] **Step 5: Verify the tests pass and that Windows accepts the file**

Run: `cargo test -p headset-tray --lib ui::icon`
Expected: all pass, including the committed-file comparison.

Then confirm the shell can actually load it — a structurally valid file that Windows rejects
would still fail silently later:

```powershell
Add-Type -AssemblyName System.Drawing
$i = New-Object System.Drawing.Icon("crates\headset-tray\assets\headset.ico", 48, 48)
"loaded at $($i.Width)x$($i.Height)"
$i.Dispose()
"file size: $((Get-Item crates\headset-tray\assets\headset.ico).Length) bytes"
```
Expected: loads at 48×48, file around 350 KB.

- [ ] **Step 6: Look at it**

Convert to PNG and view it, at more than one size. An icon that passes every structural test
can still be visually wrong:

```powershell
Add-Type -AssemblyName System.Drawing
foreach ($s in 16,32,48,256) {
  $i = New-Object System.Drawing.Icon("crates\headset-tray\assets\headset.ico", $s, $s)
  $i.ToBitmap().Save("$env:TEMP\icon-$s.png", [System.Drawing.Imaging.ImageFormat]::Png)
  $i.Dispose()
}
```
Open the PNGs. Each should read as a headset. If 16px is mush, the outline pass is eating
the shape at small sizes and the fix belongs in Task 1.

- [ ] **Step 7: Gate and commit**

```bash
git add crates/headset-tray/src/ui/icon.rs crates/headset-tray/src/main.rs crates/headset-tray/assets/headset.ico
git commit -m "feat(tray): export the icon as a multi-size .ico"
```

---

### Task 3: Embed the icon in the executable

**Problem being fixed:** the executable has no icon resource, so Explorer shows the default and a shortcut pointing at it would inherit that.

**Files:**
- Create: `crates/headset-tray/build.rs`, `crates/headset-tray/headset-tray.rc`
- Modify: `crates/headset-tray/Cargo.toml` (declare the build script)

- [ ] **Step 1: Write the resource script**

`crates/headset-tray/headset-tray.rc`:

```
1 ICON "assets/headset.ico"
```

Resource ID 1: Windows uses the lowest-numbered icon resource as the application icon, so
Explorer and shortcuts pick this up with no further configuration.

- [ ] **Step 2: Write the build script**

`crates/headset-tray/build.rs`:

```rust
//! Embeds the application icon.
//!
//! Uses `windres` from the GNU toolchain this project targets rather than a
//! resource crate, which keeps the dependency count at zero. A missing
//! `windres` is a warning and not an error: losing the icon is not a reason to
//! be unable to build.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=headset-tray.rc");
    println!("cargo:rerun-if-changed=assets/headset.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("headset-tray-rc.o");

    match Command::new("windres")
        .args(["headset-tray.rc", "-O", "coff", "-o"])
        .arg(&out)
        .status()
    {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg-bins={}", out.display());
        }
        Ok(s) => println!("cargo:warning=windres failed ({s}); building without an icon"),
        Err(e) => println!("cargo:warning=windres unavailable ({e}); building without an icon"),
    }
}
```

- [ ] **Step 3: Declare it**

In `crates/headset-tray/Cargo.toml`, under `[package]`:

```toml
build = "build.rs"
```

- [ ] **Step 4: Build and verify the resource is actually in the binary**

```powershell
cargo build --release -p headset-tray
Add-Type -AssemblyName System.Drawing
$i = [System.Drawing.Icon]::ExtractAssociatedIcon((Resolve-Path .\target\release\headset-tray.exe))
"extracted icon: $($i.Width)x$($i.Height)"
$i.ToBitmap().Save("$env:TEMP\exe-icon.png", [System.Drawing.Imaging.ImageFormat]::Png)
$i.Dispose()
```
Expected: an icon is extracted. Open `exe-icon.png` and confirm it is the headset and not a
generic default — extraction succeeding tells you a resource exists, not which one.

Also confirm the build still succeeds when the tool is missing, since the fallback is the
part most likely to be wrong:

```powershell
$saved = $env:PATH
$env:PATH = ($env:PATH -split ';' | Where-Object { -not (Test-Path (Join-Path $_ 'windres.exe')) }) -join ';'
cargo build --release -p headset-tray 2>&1 | Select-String "warning: windres"
$env:PATH = $saved
cargo build --release -p headset-tray | Out-Null
```
Expected: a `windres unavailable` warning and a successful build.

- [ ] **Step 5: Gate and commit**

```bash
git add crates/headset-tray/build.rs crates/headset-tray/headset-tray.rc crates/headset-tray/Cargo.toml
git commit -m "build(tray): embed the application icon with windres"
```

---

### Task 4: Start menu shortcut, and one owner for Add/Remove Programs

**Problem being fixed:** `install()` creates no Start menu entry, so an installed tray has no way to be launched. It also registers in Add/Remove Programs, which the Inno installer is about to own.

**Files:**
- Modify: `crates/headset-tray/src/install.rs`

**Interfaces:**
- Produces: `pub fn start_menu_shortcut() -> Option<PathBuf>`, and shortcut creation and removal wired into `install()` / `uninstall()`.

- [ ] **Step 1: Write the failing test for the path**

The COM call cannot be unit-tested, but the path it writes to can. Add to the `tests` module
in `install.rs`:

```rust
    #[test]
    fn the_shortcut_goes_in_the_per_user_start_menu() {
        let p = start_menu_shortcut().expect("APPDATA is set on Windows");
        let s = p.to_string_lossy().to_lowercase();
        assert!(s.ends_with("headset tray.lnk"), "{}", p.display());
        assert!(s.contains(r"\microsoft\windows\start menu\programs"), "{}", p.display());
        // Per-user throughout: nothing goes in the all-users Start menu, which
        // would need administrator rights this project does not ask for.
        assert!(!s.contains("programdata"), "{}", p.display());
    }
```

Run: `cargo test -p headset-tray --lib install::` and confirm it fails to compile, then add
the signature returning `unimplemented!()` and confirm it fails at runtime.

- [ ] **Step 2: Implement the path and the shortcut**

```rust
/// Where the Start menu entry goes. Per-user: the all-users Start menu needs
/// administrator rights, which this project never asks for.
pub fn start_menu_shortcut() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Headset Tray.lnk"),
    )
}

/// Creates the Start menu shortcut pointing at `exe`.
///
/// No icon is specified: the shortcut inherits the executable's own icon
/// resource, so there is one icon to keep current rather than two.
fn create_shortcut(exe: &Path) -> Result<(), InstallError> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    let Some(dest) = start_menu_shortcut() else {
        return Ok(());
    };
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    unsafe {
        // Already initialised in the tray process; harmless and required here,
        // because --install runs before any window exists.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
        link.SetPath(&HSTRING::from(exe.as_os_str()))
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
        link.SetDescription(&HSTRING::from("Headset settings in the notification area"))
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
        if let Some(dir) = exe.parent() {
            let _ = link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()));
        }
        let file: IPersistFile = link
            .cast()
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
        file.Save(&HSTRING::from(dest.as_os_str()), true)
            .map_err(|e| InstallError::Shortcut(e.to_string()))?;
    }
    Ok(())
}
```

Add the error variant alongside the existing ones:

```rust
    #[error("could not create the Start menu shortcut: {0}")]
    Shortcut(String),
```

- [ ] **Step 3: Wire it in, and hand Add/Remove Programs to Inno**

In `install()`, after the executable is copied and the startup entry is written, call
`create_shortcut(&exe)?`. **Delete the call that writes `UNINSTALL_KEY`**, and delete the
now-unused registration helper, leaving the constant only if `uninstall()` still reads it.

In `uninstall()`, remove the Start menu shortcut:

```rust
    if let Some(lnk) = start_menu_shortcut() {
        let _ = std::fs::remove_file(lnk);
    }
```

and **stop deleting the Add/Remove Programs key** — it belongs to the installer now. Add a
comment saying so, because removing a deletion looks like an oversight otherwise:

```rust
    // The Add/Remove Programs entry belongs to the Inno installer, which has its
    // own uninstaller. Deleting it here would strand an installation that this
    // path did not create. `--uninstall` is the inverse of `--install`, not a
    // general uninstaller; see the design note on ownership.
```

- [ ] **Step 4: Verify end to end by hand**

```powershell
cargo build --release -p headset-tray
.\target\release\headset-tray.exe --install
```

Then confirm:
- **The Start menu entry exists and carries the headset icon.** Press Start and type
  "Headset Tray". This is the thing the whole task is for.
- Launching it starts the tray.
- `Get-ChildItem "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall" | Where-Object PSChildName -like "*Headset*"` returns **nothing** — no Add/Remove entry from this path.
- `--uninstall` removes the folder, the shortcut, and the `Run` value.

- [ ] **Step 5: Gate and commit**

```bash
git add crates/headset-tray/src/install.rs
git commit -m "feat(tray): create a Start menu shortcut, and leave Add/Remove Programs to the installer"
```

---

### Task 5: The installer

**Files:**
- Create: `installer/headset-tray.iss`, `build-installer.ps1`
- Modify: `.gitignore` (ignore `/dist/` — already present; confirm)

- [ ] **Step 1: Install Inno Setup**

```powershell
winget install --id JRSoftware.InnoSetup --exact --accept-package-agreements --accept-source-agreements
```

`iscc.exe` lands in `C:\Program Files (x86)\Inno Setup 6`. Confirm:

```powershell
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" /? 2>&1 | Select-Object -First 2
```

- [ ] **Step 2: Write the script**

`installer/headset-tray.iss`. The `AppId` GUID below is fixed — **regenerating it makes an
upgrade install side by side instead of replacing**:

```ini
; Per-user installer. No administrator rights, nothing outside HKCU and
; %LOCALAPPDATA%, matching how this project installs by hand.
#define AppName "Headset Tray"
#define AppExe "headset-tray.exe"
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{8F2C5A31-7D64-4E19-B0C3-9A5E7F1D2B48}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=cunningorb
AppSupportURL=https://github.com/cunningorb/windows-headset-control
DefaultDirName={localappdata}\Programs\HeadsetTray
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=HeadsetTray-{#AppVersion}-setup
SetupIconFile=..\crates\headset-tray\assets\headset.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Close a running tray before replacing its file.
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "startup"; Description: "Start {#AppName} when I sign in"; GroupDescription: "Additional options:"

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\headsetctl.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; No IconFilename: the shortcut inherits the executable's own icon resource.
Name: "{userprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "Headset settings in the notification area"

[Registry]
; The same value the tray's Settings toggle reads and writes. A Startup-folder
; shortcut would make that toggle lie.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; \
  ValueName: "HeadsetTray"; ValueData: """{app}\{#AppExe}"""; Tasks: startup; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExe}"; Description: "Launch {#AppName}"; Flags: postinstall nowait skipifsilent
```

- [ ] **Step 3: Write the local build script**

`build-installer.ps1`:

```powershell
#!/usr/bin/env pwsh
# Builds the release binaries and packages them into a setup executable.
$ErrorActionPreference = 'Stop'

$iscc = Get-Command iscc -ErrorAction SilentlyContinue
if (-not $iscc) {
    $fallback = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    if (Test-Path $fallback) { $iscc = $fallback } else {
        throw "Inno Setup is not installed. Install it with:`n  winget install --id JRSoftware.InnoSetup --exact"
    }
} else { $iscc = $iscc.Source }

# The single source of truth for the version is the workspace manifest.
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Building Headset Tray $version"

cargo build --workspace --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

New-Item -ItemType Directory -Force dist | Out-Null
& $iscc "/DAppVersion=$version" installer\headset-tray.iss
if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed" }

Get-ChildItem dist\*-setup.exe | Select-Object Name, Length
```

- [ ] **Step 4: Build it and install it**

```powershell
.\build-installer.ps1
```
Expected: `dist\HeadsetTray-0.1.0-setup.exe` exists. Then run it and confirm, in order:

1. The setup executable itself shows the headset icon in Explorer.
2. The wizard runs without an administrator prompt.
3. "Start when I sign in" is offered as a checkbox.
4. After installing, **the Start menu entry exists and carries the headset icon**.
5. "Launch Headset Tray" starts the tray.
6. Settings → Installed apps lists "Headset Tray", exactly once.
7. The tray's own Settings submenu shows "Run on Windows startup" ticked, matching the
   checkbox chosen in step 3. If it does not, the installer wrote the wrong registry
   location.
8. Uninstalling from Settings removes the folder, the Start menu entry, and the `Run` value.

- [ ] **Step 5: Commit**

```bash
git add installer/headset-tray.iss build-installer.ps1
git commit -m "build: package the tray with Inno Setup"
```

---

### Task 6: Publish on a tag

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write the workflow**

```yaml
name: release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write

jobs:
  release:
    runs-on: windows-2022
    steps:
      - uses: actions/checkout@v7

      - name: Install toolchain
        run: |
          rustup toolchain install stable --profile minimal
          rustup target add x86_64-pc-windows-gnu

      - name: Install Inno Setup
        run: choco install innosetup --no-progress -y

      - name: Build and package
        shell: pwsh
        run: |
          $env:PATH += ";${env:ProgramFiles(x86)}\Inno Setup 6"
          .\build-installer.ps1

      - name: Publish
        shell: pwsh
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          $tag = "${{ github.ref_name }}"
          # Anything with a hyphen is a semver pre-release: -alpha, -beta, -rc.
          $pre = if ($tag -match '-') { '--prerelease' } else { '' }
          gh release create $tag (Get-ChildItem dist\*-setup.exe).FullName `
            --title $tag `
            --notes "Unsigned installer: Windows SmartScreen will warn. Choose More info, then Run anyway. See CHANGELOG.md for what changed." `
            $pre
```

- [ ] **Step 2: Check the workflow parses before relying on a tag push**

A malformed workflow fails only once a tag exists, and tags are awkward to retract:

```powershell
gh workflow list 2>&1 | Select-String "release"
```
If it does not appear after pushing the branch, the YAML is wrong. Fix before tagging.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: publish the installer on a version tag"
```

---

### Task 7: Version, documentation, changelog

**Files:**
- Modify: `Cargo.toml`, `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`

- [ ] **Step 1: Set the alpha version**

In `Cargo.toml`, `version = "0.1.0-alpha.1"`. Then `cargo build --workspace` to refresh
`Cargo.lock`, and confirm nothing rejects the pre-release string.

- [ ] **Step 2: Point the README at the download**

Replace the Installing section with:

```markdown
## Installing

Download the setup executable from the
[latest release](https://github.com/cunningorb/windows-headset-control/releases/latest)
and run it. It installs for your user only — no administrator rights, no driver, no
service — and offers to start the tray when you sign in.

**Windows will warn you.** The installer is not code-signed, so SmartScreen shows
"Windows protected your PC". Choose **More info**, then **Run anyway**. Signing is
tracked in `docs/release-signing.md`.

To remove it: Settings → Installed apps → Headset Tray → Uninstall.

### From source

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the toolchain requirements, then:

```powershell
.\build-installer.ps1        # produces dist\HeadsetTray-<version>-setup.exe
```

or install a build directly without packaging it:

```powershell
cargo build --release
.\target\release\headset-tray.exe --install
```
```

- [ ] **Step 3: Explain the two paths in `CONTRIBUTING.md`**

Add to the Building section:

```markdown
### Installing a local build

`headset-tray.exe --install` copies the executable to `%LOCALAPPDATA%\Programs\HeadsetTray`,
adds a Start menu shortcut, and sets the run-at-sign-in value. It deliberately does **not**
register in Add/Remove Programs — that belongs to the Inno installer, and two things writing
that key is how a stale entry pointing at a deleted file happens.

`--uninstall` reverses exactly that, and leaves an Inno-made installation alone. Remove one
of those through Settings → Installed apps.
```

- [ ] **Step 4: Update the changelog**

Replace the `## [Unreleased]` heading with `## [0.1.0-alpha.1] - 2026-08-02` and add under
Added:

```markdown
- A signed-in-user installer with a Start menu entry, published as a GitHub Release.
- An application icon: the headset the tray draws, embedded in the executable at five sizes,
  so Explorer and the Start menu show it too.
```

- [ ] **Step 5: Gate and commit**

```bash
git add Cargo.toml Cargo.lock README.md CONTRIBUTING.md CHANGELOG.md
git commit -m "docs: point at the installer, and set the alpha version"
```

---

### Task 8: Cut the alpha

**Files:** none — this task produces a tag and a release.

- [ ] **Step 1: Merge to `main` and confirm CI is green**

```powershell
git checkout main
git merge --no-ff installer-and-icon -m "Merge installer and icon"
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
git push origin main
gh run watch (gh run list --limit 1 --json databaseId | ConvertFrom-Json).databaseId --exit-status
```

Do not tag a commit whose CI has not passed: the release workflow builds from the tag, and a
failing build there produces a release with no asset.

- [ ] **Step 2: Tag and push**

```powershell
git tag -a v0.1.0-alpha.1 -m "Alpha 1: installer, Start menu entry, application icon"
git push origin v0.1.0-alpha.1
```

- [ ] **Step 3: Watch the release workflow**

```powershell
Start-Sleep -Seconds 15
$id = (gh run list --workflow release.yml --limit 1 --json databaseId | ConvertFrom-Json).databaseId
gh run watch $id --exit-status
```

If it fails, read the failing step, fix on a branch, merge, **delete the tag locally and
remotely**, and re-tag:

```powershell
git tag -d v0.1.0-alpha.1
git push origin :refs/tags/v0.1.0-alpha.1
```

- [ ] **Step 4: Verify the release from the outside**

Not by looking at the workflow — by doing what a stranger would do:

```powershell
gh release view v0.1.0-alpha.1
gh release download v0.1.0-alpha.1 --dir "$env:TEMP\alpha" --clobber
Get-ChildItem "$env:TEMP\alpha"
```
Expected: exactly one `HeadsetTray-0.1.0-alpha.1-setup.exe`, marked as a pre-release, of
plausible size (a few MB).

- [ ] **Step 5: Install from the downloaded artifact**

Run the downloaded file — not the locally built one — and repeat the checks from Task 5
Step 4. This is the only step that proves the published artifact works, as opposed to the
one on the build machine.

- [ ] **Step 6: Report**

State plainly what was verified by installing the downloaded artifact, and what was not:
code signing is absent, and no machine other than the development one has run it.

---

## Done when

- The Start menu has a "Headset Tray" entry with the headset icon, and it launches the tray.
- The executable shows the headset icon in Explorer.
- `dist\HeadsetTray-0.1.0-alpha.1-setup.exe` installs, runs, and uninstalls cleanly with no
  administrator prompt and exactly one Installed-apps entry.
- The release page carries the installer as a pre-release, and installing the **downloaded**
  copy works.
- The committed `.ico` cannot drift from the drawing without a test failing.
- The full gate passes.
