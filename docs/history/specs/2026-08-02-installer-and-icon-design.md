# Installer and Icon Design

Status: approved 2026-08-02.

## Problem

The tray can only be installed by running `headset-tray.exe --install` from a build
directory, and it creates no Start menu entry — so after installing there is no way to
launch it, only an Add/Remove Programs entry to remove it. There is also no icon resource
anywhere: `build_icon()` draws the headset procedurally at runtime, so the executable shows
the default icon in Explorer and any shortcut would inherit that.

Anyone other than the author has no way to install this at all.

## Goals

- A double-clickable setup executable that installs, creates a Start menu entry, and offers
  to start at sign-in — the experience anyone expects from Windows software.
- The Start menu entry carries the same headset icon the tray shows.
- A place to download it from.

## Non-goals

- **Code signing.** The installer will be unsigned, so SmartScreen will warn. Fixing that
  needs a certificate and is what `docs/release-signing.md` anticipates. Stated here so it
  is a known limitation rather than a surprise.
- **Per-machine installation.** This project is per-user by design and requires no
  administrator rights. That does not change.
- **An auto-updater.**

## Ownership

Two installation paths exist, and each registry key has exactly one owner. Two things
writing the same uninstall key is how a phantom Installed-apps entry pointing at a deleted
file happens.

| | Inno setup exe | `headset-tray.exe --install` |
| --- | --- | --- |
| Program files | yes | yes |
| Start menu shortcut | yes | yes |
| Run-at-startup value | yes | yes |
| Add/Remove Programs entry | yes | **no — removed** |

`--install` survives as a developer shortcut for putting a local build in place. It stops
registering in Add/Remove Programs; only the setup executable does that.

Both write the **same** `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value that the
tray's Settings toggle reads and writes. A Startup-folder shortcut would look equivalent
and would make that toggle lie, so it is not used.

`--uninstall` removes exactly what `--install` creates — the program folder, the Start menu
shortcut, and the `Run` value — and **leaves any Inno registration untouched**. It is the
inverse of the developer shortcut, not a general uninstaller. An installation made by the
setup executable is removed through Settings → Installed apps, which is where its entry
points. Running `--uninstall` against an Inno installation would delete the files while
leaving Inno's Add/Remove entry behind, so the documentation says plainly which tool
removes which installation.

## Icon

### One shape definition, many sizes

The drawing in `build_icon()` is 32×32 pixel constants: an arc centred at `(16.0, 19.0)`
with radii `10.5..=13.5`, ear cups spanning `x` 3..8 and 23..28. Those move into a pure
`crates/headset-tray/src/ui/icon.rs`:

```rust
/// The icon as straight-alpha BGRA, `n` x `n`. Pure: no OS, no window.
pub fn icon_pixels(n: usize) -> Vec<u32>
```

with every constant expressed as a fraction of `n` — `cx = 0.500·n`, `cy = 0.594·n`, ring
`0.328·n ..= 0.422·n`, ear cups at `0.094..0.266·n` and `0.719..0.891·n`, spanning
`0.469..0.844·n` vertically. These are the existing 32px constants divided by 32, so the
32px rendering is the current one.

`build_icon()` becomes a thin wrapper: `icon_pixels(32)` into an `HICON`. Runtime behaviour
is unchanged; there is now one definition instead of one usage.

### Writing the `.ico`

A new mode, `headset-tray.exe --export-icon <path>`, writes a multi-size icon containing
16, 32, 48, 128 and 256 px.

ICO is simple enough to write directly and needs no image crate: a 6-byte header, one
16-byte directory entry per image, then the images. Each image is a DIB —
`BITMAPINFOHEADER` with `biHeight` set to **twice** the real height (the format expects a
colour bitmap followed by an AND mask), 32bpp, `BI_RGB`, rows bottom-up, then an all-zero
AND mask padded to 4-byte rows. A directory entry records 0 for a 256px dimension, which is
how the format expresses 256.

**Size cost, accepted:** 32bpp uncompressed puts the 256×256 entry at 256 KB, taking the
`.ico` to roughly 350 KB and the executable from about 960 KB to about 1.3 MB. Compressing
would mean a PNG encoder — a new dependency, or hand-rolled DEFLATE for an image that is
two colours and would barely compress in stored blocks anyway. The size is accepted in
exchange for a sharp icon in Explorer's large views.

### Embedding it

The generated icon is committed at `crates/headset-tray/assets/headset.ico`. A `build.rs`
compiles `headset-tray.rc` with `windres` — already present in the WinLibs GNU toolchain
this project targets — and links the result, so the executable carries the icon as resource
ID 1. Explorer then shows the headset on the executable, and a shortcut pointing at the
executable inherits it without needing its own icon file.

`build.rs` degrades rather than failing the build if `windres` is missing: it prints a
warning and skips the resource. Losing an icon is not a reason to be unable to build.

### Keeping the committed file honest

A generated file in the tree drifts from its generator. Two tests prevent it:

- `icon_pixels(32)` is pinned, so changing the drawing fails with a message naming
  `--export-icon`.
- The committed `.ico` is regenerated in memory and compared byte-for-byte with the file on
  disk. This is the one that actually catches a forgotten regeneration.

## Installer

`installer/headset-tray.iss`:

- `PrivilegesRequired=lowest`, `DefaultDirName={localappdata}\Programs\HeadsetTray`.
- A fixed `AppId` GUID. **It must never change** — a new one makes an upgrade install
  side-by-side instead of replacing.
- `[Icons]`: `{userprograms}\Headset Tray`, pointing at the executable so it inherits the
  embedded icon.
- `[Registry]`: the HKCU `Run` value, with `uninsdeletevalue`, behind a `[Tasks]` checkbox
  so the user can decline start-at-sign-in.
- `[Run]`: a "Launch Headset Tray" checkbox, `postinstall nowait skipifsilent`.
- `CloseApplications=yes` so a running tray is closed before its file is replaced.
- `SetupIconFile` set to the same `.ico`, so the setup executable looks like the product.

## Build and release

`build-installer.ps1` builds the release binaries, then runs `iscc`, producing
`dist\HeadsetTray-<version>-setup.exe`. It fails with an actionable message if `iscc` is not
on `PATH`, naming the winget package.

`.github/workflows/release.yml` triggers on `v*` tags: installs Inno Setup on the runner,
builds, packages, and attaches the executable to a GitHub Release using `gh release create`
— no third-party actions, matching how the rest of this repository avoids dependencies it
does not need. Pre-release tags (`-alpha`, `-beta`, `-rc`) are marked as pre-releases.

## Versioning

The workspace version becomes `0.1.0-alpha.1` for the first release. Inno takes the same
string. Cargo and semver both accept it, and it sets the expectation this software has
earned: it speaks a protocol reconstructed by observation and has been run against exactly
one headset.

## Testing

Pure, deterministic, no hardware:

- `icon_pixels` shape invariants across sizes — non-empty, symmetric about the vertical
  axis, and every pixel either fill, outline, or transparent.
- The pinned 32×32 rendering.
- ICO structure: header fields, one directory entry per size, declared offsets and lengths
  landing inside the file, 256 encoded as 0.
- The committed `.ico` matches a fresh generation.

Manual, once: install from the setup executable, confirm the Start menu entry exists and
carries the headset icon, launch from it, confirm the tray appears, uninstall from Settings,
confirm the program folder, the Start menu entry, and the `Run` value are all gone.

## Consequences for existing documentation

`README.md` and `CONTRIBUTING.md` currently say to build from source and run `--install`.
They change to point at the release download for users, keeping the build-from-source path
for contributors. `CHANGELOG.md` gains the release.
