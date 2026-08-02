# Phase 1 Enumeration and Read-Only Probe — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `headsetctl.exe` with `list`, `inspect`, and a strictly read-only `probe`, verified against real hardware, without disturbing Windows audio or writing a single byte to the device.

**Architecture:** A Cargo workspace of four crates. `headset-protocol` holds pure logic with zero OS access. `headset-device` holds all Windows HID access behind a `HidBackend` trait, with a real windows-rs implementation and a fixture-driven fake. `headset-cli` renders output above that trait, so nearly everything is testable with no hardware attached. `headset-tray` is a Phase 2 placeholder.

**Tech Stack:** Rust stable 1.97.1, target `x86_64-pc-windows-gnu`, `windows` 0.58, `clap` 4, `serde`/`serde_json`, `thiserror`, `anyhow`, `tracing`, `sha2`, `insta`.

## Global Constraints

Every task's requirements implicitly include this section.

- **Target triple is `x86_64-pc-windows-gnu`.** No MSVC linker exists on the development machine.
- **`windows` crate is pinned to `0.58`.** Versions 0.59+ link via `raw-dylib`, which on the GNU target requires `dlltool.exe`; rustup's self-contained MinGW does not ship it. Version 0.58 depends on `windows-targets`, which bundles prebuilt import libraries for `x86_64-pc-windows-gnu`. **Do not bump the `windows` major/minor version** without either installing MinGW-w64 binutils or migrating to the MSVC target.
- **No `hidapi`.** All HID access uses windows-rs directly. See spec §4.
- Never require administrator privileges. Never install a driver. Never modify firmware or pairing state.
- **This entire plan performs zero HID writes.** No task in this plan may call `WriteFile`, `HidD_SetFeature`, or `HidD_SetOutputReport` against a device.
- `COL03` (usage page `0x000B`, telephony/headset) is used by the Windows audio stack. It is never opened in `ReadWrite` mode.
- All descriptor inspection opens with `dwDesiredAccess = 0`.
- Serial numbers and device paths are redacted by default; `--include-sensitive` reveals them with a warning header.
- Libraries use `thiserror`. `anyhow` appears only in `headset-cli/src/main.rs`.
- No telemetry, no runtime network access.
- Repository stays private. No license file. `Copyright © 2026. All rights reserved.`
- Copy rule: repo name, executable names, and publisher identity stay neutral. Manufacturer and product names appear only to describe compatibility.

## Verified Hardware Baseline

Measured on the development machine 2026-08-01 with a read-only spike. Treat as ground truth for this plan; re-verify if hardware changes.

| Collection | Usage page | Usage | Input len | Output len | Feature len | In report ID | Out report ID |
| ---------- | ---------- | ----- | --------- | ---------- | ----------- | ------------ | ------------- |
| `COL01`    | `0x000C`   | `0x01`| 2         | 0          | 0           | `0x0C`       | —             |
| `COL02`    | `0xFF13`   | `0x01`| 62        | 62         | 0           | `0x07`       | `0x06`        |
| `COL03`    | `0x000B`   | `0x05`| 2         | 2          | 0           | `0x05`       | `0x05`        |
| `COL04`    | `0xFF14`   | `0x01`| 64        | 64         | 0           | `0x02`       | `0x02`        |

VID `0x1532`, PID `0x101B`, version `0x0100`, product string `BlackShark V3 Pro PS HID`, manufacturer `Razer Inc`, serial present on all four.

**Feature report length is 0 on every collection**, so the control transport is interrupt output plus interrupt input. `COL04` is the presumptive control collection.

---

## File Structure

```
Cargo.toml                                    workspace manifest, shared dep versions
rust-toolchain.toml                           pins channel + gnu target + components
README.md  SECURITY.md  CONTRIBUTING.md  THIRD_PARTY_NOTICES.md
docs/architecture.md  docs/clean-room-notes.md  docs/device-research.md
docs/release-signing.md  docs/threat-model.md
.github/workflows/ci.yml

crates/headset-protocol/src/lib.rs            re-exports
crates/headset-protocol/src/frame.rs          ControlFrame: 64-byte container, ID 0x02
crates/headset-protocol/src/error.rs          ProtocolError

crates/headset-device/src/lib.rs              re-exports
crates/headset-device/src/error.rs            DeviceError
crates/headset-device/src/model.rs            CollectionInfo, DeviceId, ReportItem, OpenMode
crates/headset-device/src/backend.rs          HidBackend + HidTransport traits
crates/headset-device/src/fake.rs             FakeHidBackend (fixtures, cfg-free)
crates/headset-device/src/windows/mod.rs      WindowsHidBackend
crates/headset-device/src/windows/ffi.rs      raw SetupAPI/HID calls, unsafe confined here
crates/headset-device/src/windows/transport.rs WindowsTransport (read-only in this plan)
crates/headset-device/src/select.rs           candidate ranking
crates/headset-device/tests/fixtures/*.json   captured, redacted enumeration fixtures

crates/headset-cli/src/main.rs                anyhow boundary, clap dispatch
crates/headset-cli/src/cli.rs                 clap types, VID/PID parsing
crates/headset-cli/src/redact.rs              redaction
crates/headset-cli/src/render/mod.rs          Renderer selection
crates/headset-cli/src/render/human.rs        human-readable output
crates/headset-cli/src/render/json.rs         JSON output
crates/headset-cli/src/cmd/list.rs            list command
crates/headset-cli/src/cmd/inspect.rs         inspect command
crates/headset-cli/src/cmd/probe.rs           probe command
crates/headset-cli/tests/snapshots/*.snap     insta snapshots

crates/headset-tray/src/lib.rs                Phase 2 placeholder
```

---

### Task 1: Workspace scaffold, toolchain pin, CI, legal baseline

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`
- Create: `crates/{headset-protocol,headset-device,headset-cli,headset-tray}/Cargo.toml` and `src/lib.rs` / `src/main.rs`
- Create: `README.md`, `SECURITY.md`, `CONTRIBUTING.md`, `THIRD_PARTY_NOTICES.md`
- Create: `docs/architecture.md`, `docs/clean-room-notes.md`, `docs/device-research.md`, `docs/threat-model.md`, `docs/release-signing.md`
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: nothing.
- Produces: a workspace where `cargo build --workspace` succeeds and crate names `headset_protocol`, `headset_device`, `headset_cli`, `headset_tray` resolve.

- [ ] **Step 1: Write the workspace manifest**

`Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/headset-protocol",
    "crates/headset-device",
    "crates/headset-cli",
    "crates/headset-tray",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.85"
publish = false
license = "UNLICENSED"

[workspace.dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
insta = { version = "1", features = ["json"] }

# Pinned to 0.58 deliberately. See Global Constraints.
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_Storage_FileSystem",
    "Win32_System_IO",
    "Win32_System_Threading",
    "Win32_Devices_DeviceAndDriverInstallation",
    "Win32_Devices_HumanInterfaceDevice",
] }

[profile.release]
strip = "debuginfo"
lto = "thin"
codegen-units = 1
panic = "abort"
```

- [ ] **Step 2: Pin the toolchain**

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
targets = ["x86_64-pc-windows-gnu"]
components = ["rustfmt", "clippy"]
```

- [ ] **Step 3: Create the four crate manifests**

`crates/headset-protocol/Cargo.toml`:

```toml
[package]
name = "headset-protocol"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }
```

`crates/headset-device/Cargo.toml`:

```toml
[package]
name = "headset-device"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true
license.workspace = true

[dependencies]
headset-protocol = { path = "../headset-protocol" }
serde = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }

[target.'cfg(windows)'.dependencies]
windows = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

`crates/headset-cli/Cargo.toml`:

```toml
[package]
name = "headset-cli"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true
license.workspace = true

[[bin]]
name = "headsetctl"
path = "src/main.rs"

[dependencies]
headset-device = { path = "../headset-device" }
headset-protocol = { path = "../headset-protocol" }
anyhow = { workspace = true }
clap = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[dev-dependencies]
insta = { workspace = true }
```

`crates/headset-tray/Cargo.toml`:

```toml
[package]
name = "headset-tray"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true
license.workspace = true

[dependencies]
```

- [ ] **Step 4: Create placeholder sources**

`crates/headset-protocol/src/lib.rs`:

```rust
//! Pure protocol logic. No operating-system access.
#![forbid(unsafe_code)]
```

`crates/headset-device/src/lib.rs`:

```rust
//! Windows HID device access behind a mockable backend trait.
```

`crates/headset-cli/src/main.rs`:

```rust
fn main() -> anyhow::Result<()> {
    Ok(())
}
```

`crates/headset-tray/src/lib.rs`:

```rust
//! Placeholder for the Phase 2 Windows tray application.
//!
//! Intentionally empty. The tray is designed in a separate spec and is not
//! implemented in Phase 1. Nothing here should be depended upon.
#![forbid(unsafe_code)]
```

- [ ] **Step 5: Verify the workspace builds**

Run: `cargo build --workspace`
Expected: `Finished` with no errors.

- [ ] **Step 6: Write the documentation baseline**

`README.md` — must contain, verbatim, a non-affiliation statement:

```markdown
# windows-headset-control

Experimental native Windows HID controller for supported wireless headset settings.

**Status:** experimental, private, unreleased. Nothing here is supported or fit for general use.

## What this is

A user-mode Windows utility that reads, and eventually controls, supported settings of a
wireless gaming headset over its proprietary HID interface.

- Runs as a normal user. No administrator rights.
- Installs no driver and no service.
- Reads and writes no firmware.
- Makes no network requests and collects no telemetry.

## Non-affiliation

This is an unofficial community interoperability utility. It is not affiliated with,
authorized by, endorsed by, or sponsored by Razer Inc. or any other manufacturer.
Product names are used only to describe hardware compatibility.

Copyright © 2026. All rights reserved.
```

`SECURITY.md`:

```markdown
# Security Policy

## Reporting

This project is private and experimental. Report suspected vulnerabilities privately
to the repository owner. Do not open public issues.

## Design constraints

- All USB/HID input is treated as untrusted. Response lengths are validated before parsing.
- No unbounded reads or allocations.
- No administrator privileges. No driver installation. No service installation.
- No firmware read, write, or modification.
- HID writes are gated behind an explicit allowlist. Broad command scanning is prohibited.
- Serial numbers and device paths are redacted from output by default.
- No telemetry. No runtime network access.
- Signing material is never committed. See `docs/release-signing.md`.
```

`CONTRIBUTING.md`:

```markdown
# Contributing

## Before every push

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

## Hard rules

- Never copy source, comments, assets, or structure from an unlicensed third-party project.
- Never commit `.pfx`, `.p12`, private keys, passwords, or signing tokens.
- Never add a HID write without adding it to the allowlist and documenting the rationale.
- Never send speculative or brute-forced HID command identifiers.
- Never publish or change repository visibility without explicit instruction.
- Redact serial numbers and device paths in issues, logs, and commit messages.
```

`THIRD_PARTY_NOTICES.md`:

```markdown
# Third-Party Notices

This product bundles no third-party source code. It links Rust crates whose licenses
are reproduced below. Regenerate with `cargo tree -f "{p} {l}"` when dependencies change.

| Crate | License |
| ----- | ------- |
| (populate by running the command below and pasting the deduplicated result) |

No source code from any third-party reverse-engineering project is included.
See `docs/clean-room-notes.md`.
```

Populate the table before committing:

```powershell
cargo tree --workspace --prefix none --format "| {p} | {l} |" | Sort-Object -Unique
```

If any dependency reports a license that is not MIT, Apache-2.0, BSD, ISC, Unicode, or
Zlib, stop and report it rather than committing. A copyleft dependency is a distribution
blocker and needs a decision, not a silent inclusion.

`docs/clean-room-notes.md`:

```markdown
# Clean-Room Notes

## Purpose

Record which public behavioral facts were consulted, and affirm that all implementation
code in this repository was written independently.

## Reference material treated as read-only research

- `https://github.com/RiskRunner0/blackshark-linux` — consulted as a statement that a
  vendor HID control path exists for a headset in this product family, and as the source
  of the initial hypotheses listed below. It was not forked. No source file, function,
  comment, asset, README text, or project structure was copied from it. It does not
  appear to carry an explicit license, so it is treated as all-rights-reserved.

## Hypotheses taken from public discussion, and their verification status

| Hypothesis | Source | Status on our hardware |
| ---------- | ------ | ---------------------- |
| VID `0x1532` | public discussion | Confirmed by our own enumeration |
| PID `0x0577` | public discussion | **Refuted.** Our hardware reports `0x101B` |
| Control on USB interface 5 | public discussion | Consistent; interface 5 carries the vendor collections, but exposes two of them |
| 64-byte reports | public discussion | Confirmed for `COL04` by our own `HidP_GetCaps` reading |
| Report ID `0x02` | public discussion | Confirmed for `COL04` by our own `HidP_GetValueCaps` reading |
| Sidetone range 0–15 | public discussion | Unverified. Out of Phase 1 scope |
| Firmware 1.3.x or newer | public discussion | Unverified |

Every "Confirmed" row above was established by reading Windows descriptors on our own
hardware, not by trusting the source. Facts observed from device behavior, USB
descriptors, and our own test results are recorded as facts. No implementation was
derived from third-party source code.

## Independence affirmation

All code in `crates/` was written from the specification in
`docs/history/specs/2026-07-31-windows-headset-control-design.md` and from
descriptor data measured on our own hardware.
```

`docs/architecture.md` — contains the crate table (name, responsibility, may it touch
the OS) copied from the File Structure section of this plan, the `HidBackend` /
`HidTransport` / `OpenMode` signatures from Task 3, and one paragraph stating that
`headset-protocol` declares `#![forbid(unsafe_code)]` and that all `unsafe` in the
workspace is confined to `crates/headset-device/src/windows/ffi.rs`.

`docs/threat-model.md` — contains a table with columns Asset, Threat, Mitigation, with
one row for each of: untrusted HID input (malformed or hostile report → length and
report-ID validated before parsing, fixed-size buffers, no unbounded allocation);
machine-identifying data (path or serial leaking into a bug report → redaction on by
default, `--include-sensitive` prints a warning banner); audio-stack contention
(opening the telephony collection breaks playback → `OpenMode::Descriptors` requests
zero access rights, and `COL03` is refused for `ReadWrite`); privilege (→ `asInvoker`
only, no driver, no service); and supply chain (→ no `.pfx`/key ever committed,
dependency licenses tracked in `THIRD_PARTY_NOTICES.md`).

`docs/device-research.md` — for Task 1, contains only the Verified Hardware Baseline
table copied verbatim from this plan, under a heading stating it was measured on our
own hardware with `HidP_GetCaps` and `HidP_GetValueCaps`. Task 8 populates the rest.

`docs/release-signing.md` — contains exactly this paragraph:

```markdown
# Release Signing

Signing is designed in a later phase. Nothing in this repository is signed today.

Two rules apply now and are not deferred:

1. No `.pfx`, `.p12`, private key, certificate password, or signing token may ever be
   committed. `.gitignore` blocks the common extensions; that is a backstop, not a
   substitute for care.
2. Signing secrets must never be exposed to pull-request workflows. When a signing job
   is added it will live in a separate, protected job gated on a GitHub environment.
```

- [ ] **Step 7: Write CI**

`.github/workflows/ci.yml`:

```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  check:
    runs-on: windows-2022
    steps:
      - uses: actions/checkout@v4

      - name: Install toolchain
        run: |
          rustup toolchain install stable --profile minimal --component rustfmt,clippy
          rustup target add x86_64-pc-windows-gnu
          rustup default stable

      - uses: Swatinem/rust-cache@v2

      - name: Format
        run: cargo fmt --all --check

      - name: Clippy
        run: cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings

      - name: Test
        run: cargo test --workspace --target x86_64-pc-windows-gnu
        env:
          HEADSET_HARDWARE_TESTS: ""

      - name: Release build
        run: cargo build --workspace --release --target x86_64-pc-windows-gnu
```

- [ ] **Step 8: Verify the full gate passes locally**

Run:
```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```
Expected: all four succeed.

- [ ] **Step 9: Commit**

```bash
git add .
git commit -m "chore: scaffold workspace, docs, and CI"
```

---

### Task 2: Device model and error types

**Files:**
- Create: `crates/headset-device/src/model.rs`
- Create: `crates/headset-device/src/error.rs`
- Modify: `crates/headset-device/src/lib.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `CollectionInfo`, `ReportItem`, `ReportKind`, `DeviceId`, `OpenMode`, `DeviceError`. Task 3 constructs `CollectionInfo`; Tasks 4–10 consume it.

- [ ] **Step 1: Write the failing test**

`crates/headset-device/src/model.rs` (test module at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CollectionInfo {
        CollectionInfo {
            id: DeviceId::new("\\\\?\\hid#vid_1532&pid_101b&mi_05&col04#7&abc&0&0000"),
            vendor_id: 0x1532,
            product_id: 0x101B,
            version: 0x0100,
            interface_number: Some(5),
            collection_number: Some(4),
            usage_page: 0xFF14,
            usage: 0x0001,
            input_report_len: 64,
            output_report_len: 64,
            feature_report_len: 0,
            product: Some("BlackShark V3 Pro PS HID".into()),
            manufacturer: Some("Razer Inc".into()),
            has_serial: true,
            report_items: vec![],
        }
    }

    #[test]
    fn vendor_defined_usage_page_is_detected() {
        assert!(sample().is_vendor_defined());
    }

    #[test]
    fn standard_usage_page_is_not_vendor_defined() {
        let mut c = sample();
        c.usage_page = 0x000B;
        assert!(!c.is_vendor_defined());
    }

    #[test]
    fn audio_stack_collection_is_flagged() {
        let mut c = sample();
        c.usage_page = 0x000B;
        c.usage = 0x0005;
        assert!(c.is_audio_stack_collection());
        assert!(!sample().is_audio_stack_collection());
    }

    #[test]
    fn interface_and_collection_parse_from_path() {
        let id = DeviceId::new("\\\\?\\hid#vid_1532&pid_101b&mi_05&col04#7&abc&0&0000");
        assert_eq!(id.interface_number(), Some(5));
        assert_eq!(id.collection_number(), Some(4));
    }

    #[test]
    fn path_without_interface_yields_none() {
        let id = DeviceId::new("\\\\?\\hid#vid_046d&pid_c52b#6&xyz&0&0000");
        assert_eq!(id.interface_number(), None);
        assert_eq!(id.collection_number(), None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p headset-device`
Expected: FAIL, `cannot find type CollectionInfo`.

- [ ] **Step 3: Write the implementation**

`crates/headset-device/src/model.rs` (above the test module):

```rust
use serde::Serialize;

/// Opaque handle to one HID collection. Wraps a Windows device interface path.
/// The raw path is machine-identifying and must be redacted before display.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// The raw Windows path. Callers must redact before displaying.
    pub fn raw(&self) -> &str {
        &self.0
    }

    /// Parses `&mi_NN` out of the interface path, if present.
    pub fn interface_number(&self) -> Option<u8> {
        parse_hex_token(&self.0, "&mi_")
    }

    /// Parses `&col_NN` / `&colNN` out of the interface path, if present.
    pub fn collection_number(&self) -> Option<u8> {
        parse_hex_token(&self.0, "&col")
    }
}

fn parse_hex_token(path: &str, marker: &str) -> Option<u8> {
    let lower = path.to_ascii_lowercase();
    let start = lower.find(marker)? + marker.len();
    let digits: String = lower[start..]
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .take(2)
        .collect();
    if digits.is_empty() {
        return None;
    }
    u8::from_str_radix(&digits, 16).ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportKind {
    Input,
    Output,
    Feature,
}

/// One declared item from the parsed report descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReportItem {
    pub kind: ReportKind,
    pub report_id: u8,
    pub usage_page: u16,
    pub usage_min: u16,
    pub usage_max: u16,
    /// Bits per field. Zero for button items.
    pub bit_size: u16,
    /// Number of fields. Zero for button items.
    pub report_count: u16,
    pub is_button: bool,
}

/// Everything readable about one HID collection without performing I/O.
#[derive(Clone, Debug)]
pub struct CollectionInfo {
    pub id: DeviceId,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub interface_number: Option<u8>,
    pub collection_number: Option<u8>,
    pub usage_page: u16,
    pub usage: u16,
    pub input_report_len: u16,
    pub output_report_len: u16,
    pub feature_report_len: u16,
    pub product: Option<String>,
    pub manufacturer: Option<String>,
    pub has_serial: bool,
    pub report_items: Vec<ReportItem>,
}

impl CollectionInfo {
    /// Vendor-defined usage pages occupy 0xFF00..=0xFFFF.
    pub fn is_vendor_defined(&self) -> bool {
        self.usage_page >= 0xFF00
    }

    /// Usage page 0x0B usage 0x05 is the telephony headset collection the
    /// Windows audio stack binds to. Never open this for I/O.
    pub fn is_audio_stack_collection(&self) -> bool {
        self.usage_page == 0x000B && self.usage == 0x0005
    }

    /// Declared report IDs for one report kind, ascending and deduplicated.
    pub fn report_ids(&self, kind: ReportKind) -> Vec<u8> {
        let mut ids: Vec<u8> = self
            .report_items
            .iter()
            .filter(|i| i.kind == kind)
            .map(|i| i.report_id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

/// How a collection should be opened. `Descriptors` maps to
/// `CreateFileW(dwDesiredAccess = 0)`, which cannot perform I/O and therefore
/// cannot contend with the audio stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMode {
    Descriptors,
    ReadWrite,
}
```

`crates/headset-device/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("no matching device is present")]
    DongleNotFound,

    #[error("dongle is present but the headset is not reachable over the wireless link")]
    WirelessLinkUnavailable,

    #[error("access to the device was denied; another process may hold it exclusively")]
    AccessDenied,

    #[error("device is busy")]
    Busy,

    #[error("device was disconnected during the operation")]
    DisconnectedDuringOp,

    #[error("device did not respond within {0:?}")]
    Timeout(std::time::Duration),

    #[error("{0} devices matched; disambiguate with an explicit selector")]
    AmbiguousDevice(usize),

    #[error("response failed validation: {0}")]
    ProtocolMismatch(String),

    #[error("device firmware is not supported: {0}")]
    UnsupportedFirmware(String),

    #[error("descriptor value outside the expected range: {0}")]
    UnexpectedDescriptor(String),

    #[error("refusing to open the audio-stack collection for I/O")]
    RefusedAudioCollection,

    #[error("windows error: {0}")]
    Os(String),
}
```

`crates/headset-device/src/lib.rs`:

```rust
//! Windows HID device access behind a mockable backend trait.

pub mod error;
pub mod model;

pub use error::DeviceError;
pub use model::{CollectionInfo, DeviceId, OpenMode, ReportItem, ReportKind};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p headset-device`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/headset-device
git commit -m "feat: add device model and error types"
```

---

### Task 3: Backend trait and fake backend

**Files:**
- Create: `crates/headset-device/src/backend.rs`
- Create: `crates/headset-device/src/fake.rs`
- Create: `crates/headset-device/tests/fixtures/blackshark-v3-pro-ps.json`
- Modify: `crates/headset-device/src/lib.rs`

**Interfaces:**
- Consumes: `CollectionInfo`, `DeviceId`, `OpenMode`, `DeviceError` from Task 2.
- Produces: `trait HidBackend { fn enumerate(&self) -> Result<Vec<CollectionInfo>, DeviceError>; fn open(&self, id: &DeviceId, mode: OpenMode) -> Result<Box<dyn HidTransport>, DeviceError>; }`, `trait HidTransport { fn read_report(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, DeviceError>; }`, and `FakeHidBackend::from_fixture_str(&str)`. Tasks 4–10 are written against these.

- [ ] **Step 1: Write the failing test**

`crates/headset-device/src/fake.rs` (test module at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::HidBackend;

    const FIXTURE: &str = include_str!("../tests/fixtures/blackshark-v3-pro-ps.json");

    #[test]
    fn fixture_yields_four_collections() {
        let b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        assert_eq!(b.enumerate().unwrap().len(), 4);
    }

    #[test]
    fn fixture_control_collection_has_expected_shape() {
        let b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let all = b.enumerate().unwrap();
        let c = all
            .iter()
            .find(|c| c.usage_page == 0xFF14)
            .expect("vendor page 0xFF14 present");
        assert_eq!(c.output_report_len, 64);
        assert_eq!(c.feature_report_len, 0);
        assert_eq!(c.report_ids(crate::ReportKind::Output), vec![0x02]);
    }

    #[test]
    fn opening_audio_collection_read_write_is_refused() {
        let b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let all = b.enumerate().unwrap();
        let audio = all
            .iter()
            .find(|c| c.is_audio_stack_collection())
            .expect("audio collection present");
        let err = b.open(&audio.id, OpenMode::ReadWrite).unwrap_err();
        assert!(matches!(err, DeviceError::RefusedAudioCollection));
    }

    #[test]
    fn opening_audio_collection_for_descriptors_is_allowed() {
        let b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let all = b.enumerate().unwrap();
        let audio = all.iter().find(|c| c.is_audio_stack_collection()).unwrap();
        assert!(b.open(&audio.id, OpenMode::Descriptors).is_ok());
    }

    #[test]
    fn unknown_device_id_is_not_found() {
        let b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let err = b
            .open(&DeviceId::new("nonexistent"), OpenMode::Descriptors)
            .unwrap_err();
        assert!(matches!(err, DeviceError::DongleNotFound));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p headset-device`
Expected: FAIL, fixture file missing and `FakeHidBackend` undefined.

- [ ] **Step 3: Write the fixture**

`crates/headset-device/tests/fixtures/blackshark-v3-pro-ps.json` — captured from the
development machine and redacted. Paths are synthetic; only the structural fragments
that the code parses are preserved.

```json
{
  "collections": [
    {
      "path": "\\\\?\\hid#vid_1532&pid_101b&mi_05&col01#0&fixture&0&0000",
      "vendor_id": 5426, "product_id": 4123, "version": 256,
      "usage_page": 12, "usage": 1,
      "input_report_len": 2, "output_report_len": 0, "feature_report_len": 0,
      "product": "BlackShark V3 Pro PS HID", "manufacturer": "Razer Inc",
      "has_serial": true,
      "report_items": [
        { "kind": "input", "report_id": 12, "usage_page": 12, "usage_min": 182, "usage_max": 182, "bit_size": 0, "report_count": 0, "is_button": true }
      ]
    },
    {
      "path": "\\\\?\\hid#vid_1532&pid_101b&mi_05&col02#0&fixture&0&0001",
      "vendor_id": 5426, "product_id": 4123, "version": 256,
      "usage_page": 65299, "usage": 1,
      "input_report_len": 62, "output_report_len": 62, "feature_report_len": 0,
      "product": "BlackShark V3 Pro PS HID", "manufacturer": "Razer Inc",
      "has_serial": true,
      "report_items": [
        { "kind": "input", "report_id": 7, "usage_page": 65299, "usage_min": 0, "usage_max": 0, "bit_size": 8, "report_count": 61, "is_button": false },
        { "kind": "output", "report_id": 6, "usage_page": 65299, "usage_min": 0, "usage_max": 0, "bit_size": 8, "report_count": 61, "is_button": false }
      ]
    },
    {
      "path": "\\\\?\\hid#vid_1532&pid_101b&mi_05&col03#0&fixture&0&0002",
      "vendor_id": 5426, "product_id": 4123, "version": 256,
      "usage_page": 11, "usage": 5,
      "input_report_len": 2, "output_report_len": 2, "feature_report_len": 0,
      "product": "BlackShark V3 Pro PS HID", "manufacturer": "Razer Inc",
      "has_serial": true,
      "report_items": [
        { "kind": "input", "report_id": 5, "usage_page": 11, "usage_min": 32, "usage_max": 32, "bit_size": 0, "report_count": 0, "is_button": true },
        { "kind": "output", "report_id": 5, "usage_page": 8, "usage_min": 42, "usage_max": 42, "bit_size": 0, "report_count": 0, "is_button": true }
      ]
    },
    {
      "path": "\\\\?\\hid#vid_1532&pid_101b&mi_05&col04#0&fixture&0&0003",
      "vendor_id": 5426, "product_id": 4123, "version": 256,
      "usage_page": 65300, "usage": 1,
      "input_report_len": 64, "output_report_len": 64, "feature_report_len": 0,
      "product": "BlackShark V3 Pro PS HID", "manufacturer": "Razer Inc",
      "has_serial": true,
      "report_items": [
        { "kind": "input", "report_id": 2, "usage_page": 65300, "usage_min": 0, "usage_max": 0, "bit_size": 8, "report_count": 63, "is_button": false },
        { "kind": "output", "report_id": 2, "usage_page": 65300, "usage_min": 0, "usage_max": 0, "bit_size": 8, "report_count": 63, "is_button": false }
      ]
    }
  ]
}
```

- [ ] **Step 4: Write the backend traits**

`crates/headset-device/src/backend.rs`:

```rust
use std::time::Duration;

use crate::error::DeviceError;
use crate::model::{CollectionInfo, DeviceId, OpenMode};

/// An open handle to one HID collection.
///
/// Phase 1 exposes reads only. A write method is deliberately absent so that no
/// caller can perform a HID write before the write phase is designed and approved.
pub trait HidTransport {
    /// Reads one input report. `buf` must be at least the collection's
    /// `input_report_len`. Returns the number of bytes read, including the
    /// leading report-ID byte.
    fn read_report(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, DeviceError>;

    /// The collection's declared input report length, including the report-ID byte.
    fn input_report_len(&self) -> u16;
}

pub trait HidBackend {
    fn enumerate(&self) -> Result<Vec<CollectionInfo>, DeviceError>;

    fn open(&self, id: &DeviceId, mode: OpenMode)
        -> Result<Box<dyn HidTransport>, DeviceError>;
}
```

- [ ] **Step 5: Write the fake backend**

`crates/headset-device/src/fake.rs` (above the test module):

```rust
use std::time::Duration;

use serde::Deserialize;

use crate::backend::{HidBackend, HidTransport};
use crate::error::DeviceError;
use crate::model::{CollectionInfo, DeviceId, OpenMode, ReportItem, ReportKind};

#[derive(Deserialize)]
struct FixtureRoot {
    collections: Vec<FixtureCollection>,
}

#[derive(Deserialize)]
struct FixtureCollection {
    path: String,
    vendor_id: u16,
    product_id: u16,
    version: u16,
    usage_page: u16,
    usage: u16,
    input_report_len: u16,
    output_report_len: u16,
    feature_report_len: u16,
    product: Option<String>,
    manufacturer: Option<String>,
    has_serial: bool,
    #[serde(default)]
    report_items: Vec<FixtureItem>,
}

#[derive(Deserialize)]
struct FixtureItem {
    kind: String,
    report_id: u8,
    usage_page: u16,
    usage_min: u16,
    usage_max: u16,
    bit_size: u16,
    report_count: u16,
    is_button: bool,
}

/// Hardware-free backend driven by a JSON fixture. Used by every test that
/// exercises filtering, ranking, redaction, or rendering.
pub struct FakeHidBackend {
    collections: Vec<CollectionInfo>,
    /// Input reports handed out by `read_report`, in order, per device path.
    canned_reads: Vec<(String, Vec<u8>)>,
}

impl FakeHidBackend {
    pub fn from_fixture_str(json: &str) -> Result<Self, DeviceError> {
        let root: FixtureRoot = serde_json::from_str(json)
            .map_err(|e| DeviceError::UnexpectedDescriptor(e.to_string()))?;

        let collections = root
            .collections
            .into_iter()
            .map(|c| {
                let id = DeviceId::new(c.path);
                CollectionInfo {
                    interface_number: id.interface_number(),
                    collection_number: id.collection_number(),
                    id,
                    vendor_id: c.vendor_id,
                    product_id: c.product_id,
                    version: c.version,
                    usage_page: c.usage_page,
                    usage: c.usage,
                    input_report_len: c.input_report_len,
                    output_report_len: c.output_report_len,
                    feature_report_len: c.feature_report_len,
                    product: c.product,
                    manufacturer: c.manufacturer,
                    has_serial: c.has_serial,
                    report_items: c
                        .report_items
                        .into_iter()
                        .map(|i| ReportItem {
                            kind: match i.kind.as_str() {
                                "output" => ReportKind::Output,
                                "feature" => ReportKind::Feature,
                                _ => ReportKind::Input,
                            },
                            report_id: i.report_id,
                            usage_page: i.usage_page,
                            usage_min: i.usage_min,
                            usage_max: i.usage_max,
                            bit_size: i.bit_size,
                            report_count: i.report_count,
                            is_button: i.is_button,
                        })
                        .collect(),
                }
            })
            .collect();

        Ok(Self { collections, canned_reads: Vec::new() })
    }

    /// Queues an input report to be returned by `read_report` for one device.
    pub fn push_read(&mut self, id: &DeviceId, report: Vec<u8>) {
        self.canned_reads.push((id.raw().to_string(), report));
    }
}

struct FakeTransport {
    input_report_len: u16,
    reports: Vec<Vec<u8>>,
    cursor: std::cell::Cell<usize>,
}

impl HidTransport for FakeTransport {
    fn read_report(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, DeviceError> {
        let i = self.cursor.get();
        let Some(report) = self.reports.get(i) else {
            return Err(DeviceError::Timeout(timeout));
        };
        self.cursor.set(i + 1);
        if buf.len() < report.len() {
            return Err(DeviceError::UnexpectedDescriptor(format!(
                "buffer {} smaller than report {}",
                buf.len(),
                report.len()
            )));
        }
        buf[..report.len()].copy_from_slice(report);
        Ok(report.len())
    }

    fn input_report_len(&self) -> u16 {
        self.input_report_len
    }
}

impl HidBackend for FakeHidBackend {
    fn enumerate(&self) -> Result<Vec<CollectionInfo>, DeviceError> {
        Ok(self.collections.clone())
    }

    fn open(
        &self,
        id: &DeviceId,
        mode: OpenMode,
    ) -> Result<Box<dyn HidTransport>, DeviceError> {
        let c = self
            .collections
            .iter()
            .find(|c| c.id == *id)
            .ok_or(DeviceError::DongleNotFound)?;

        if mode == OpenMode::ReadWrite && c.is_audio_stack_collection() {
            return Err(DeviceError::RefusedAudioCollection);
        }

        let reports = self
            .canned_reads
            .iter()
            .filter(|(p, _)| p == id.raw())
            .map(|(_, r)| r.clone())
            .collect();

        Ok(Box::new(FakeTransport {
            input_report_len: c.input_report_len,
            reports,
            cursor: std::cell::Cell::new(0),
        }))
    }
}
```

- [ ] **Step 6: Export the new modules**

`crates/headset-device/src/lib.rs`:

```rust
//! Windows HID device access behind a mockable backend trait.

pub mod backend;
pub mod error;
pub mod fake;
pub mod model;

pub use backend::{HidBackend, HidTransport};
pub use error::DeviceError;
pub use fake::FakeHidBackend;
pub use model::{CollectionInfo, DeviceId, OpenMode, ReportItem, ReportKind};
```

Add `serde_json` to `[dependencies]` (not just dev-dependencies) in
`crates/headset-device/Cargo.toml`, since `fake.rs` is compiled into the library.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p headset-device`
Expected: 10 passed (5 from Task 2, 5 new).

- [ ] **Step 8: Commit**

```bash
git add crates/headset-device
git commit -m "feat: add HID backend trait and fixture-driven fake backend"
```

---

### Task 4: Windows enumeration backend

**Files:**
- Create: `crates/headset-device/src/windows/mod.rs`
- Create: `crates/headset-device/src/windows/ffi.rs`
- Modify: `crates/headset-device/src/lib.rs`

**Interfaces:**
- Consumes: `CollectionInfo`, `ReportItem`, `DeviceError` from Task 2; `HidBackend` from Task 3.
- Produces: `WindowsHidBackend::new()` implementing `HidBackend::enumerate`. `open` returns `DeviceError::Os("not implemented")` until Task 9.

This code is adapted from a spike that was compiled and run successfully against the
real device on 2026-08-01. The FFI shapes below are correct for `windows` 0.58.

- [ ] **Step 1: Write the FFI layer**

`crates/headset-device/src/windows/ffi.rs`:

```rust
//! All `unsafe` in this crate is confined to this module.
//!
//! Every function here opens devices with `dwDesiredAccess = 0`, which grants
//! no read or write rights and therefore cannot contend with the audio stack.

use std::ffi::c_void;

use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetHidGuid, HidD_GetManufacturerString,
    HidD_GetPreparsedData, HidD_GetProductString, HidD_GetSerialNumberString, HidP_Feature,
    HidP_GetButtonCaps, HidP_GetCaps, HidP_GetValueCaps, HidP_Input, HidP_Output,
    HIDD_ATTRIBUTES, HIDP_BUTTON_CAPS, HIDP_CAPS, HIDP_REPORT_TYPE, HIDP_VALUE_CAPS,
    PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, NTSTATUS};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

use crate::error::DeviceError;
use crate::model::{ReportItem, ReportKind};

const HIDP_STATUS_SUCCESS: NTSTATUS = NTSTATUS(0x0011_0000u32 as i32);

/// Raw descriptor facts for one collection.
pub struct RawCaps {
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub input_report_len: u16,
    pub output_report_len: u16,
    pub feature_report_len: u16,
    pub product: Option<String>,
    pub manufacturer: Option<String>,
    pub has_serial: bool,
    pub report_items: Vec<ReportItem>,
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Lists every present HID device interface path.
pub fn enumerate_interface_paths() -> Result<Vec<String>, DeviceError> {
    let mut paths = Vec::new();
    unsafe {
        let hid_guid: GUID = HidD_GetHidGuid();
        let devinfo = SetupDiGetClassDevsW(
            Some(&hid_guid),
            None,
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
        .map_err(|e| DeviceError::Os(e.to_string()))?;

        let mut index = 0u32;
        loop {
            let mut iface = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiEnumDeviceInterfaces(devinfo, None, &hid_guid, index, &mut iface).is_err() {
                break;
            }
            index += 1;

            let mut required: u32 = 0;
            let _ =
                SetupDiGetDeviceInterfaceDetailW(devinfo, &iface, None, 0, Some(&mut required), None);
            // Reject implausible sizes rather than allocating whatever we are told.
            if required == 0 || required > 4096 {
                continue;
            }

            let mut buf = vec![0u8; required as usize];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(devinfo, &iface, Some(detail), required, None, None)
                .is_err()
            {
                continue;
            }

            let path_ptr = (&raw const (*detail).DevicePath) as *const u16;
            let mut len = 0usize;
            while *path_ptr.add(len) != 0 && len < 2048 {
                len += 1;
            }
            paths.push(String::from_utf16_lossy(std::slice::from_raw_parts(path_ptr, len)));
        }

        let _ = SetupDiDestroyDeviceInfoList(devinfo);
    }
    Ok(paths)
}

/// Opens with `dwDesiredAccess = 0` and reads descriptor facts. Never performs I/O.
pub fn read_caps(path: &str) -> Option<RawCaps> {
    unsafe {
        let mut wide: Vec<u16> = path.encode_utf16().collect();
        wide.push(0);

        let handle: HANDLE = CreateFileW(
            PCWSTR(wide.as_ptr()),
            0, // no read, no write
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
        .ok()?;

        let mut attrs = HIDD_ATTRIBUTES {
            Size: size_of::<HIDD_ATTRIBUTES>() as u32,
            ..Default::default()
        };
        if !HidD_GetAttributes(handle, &mut attrs).as_bool() {
            let _ = CloseHandle(handle);
            return None;
        }

        let mut prep = PHIDP_PREPARSED_DATA::default();
        let mut caps = HIDP_CAPS::default();
        let mut report_items = Vec::new();
        if HidD_GetPreparsedData(handle, &mut prep).as_bool() {
            if HidP_GetCaps(prep, &mut caps) == HIDP_STATUS_SUCCESS {
                report_items = collect_report_items(prep, &caps);
            } else {
                caps = HIDP_CAPS::default();
            }
            let _ = HidD_FreePreparsedData(prep);
        }

        let product = read_string(handle, HidD_GetProductString);
        let manufacturer = read_string(handle, HidD_GetManufacturerString);
        let has_serial = read_string(handle, HidD_GetSerialNumberString).is_some();

        let _ = CloseHandle(handle);

        Some(RawCaps {
            vendor_id: attrs.VendorID,
            product_id: attrs.ProductID,
            version: attrs.VersionNumber,
            usage_page: caps.UsagePage,
            usage: caps.Usage,
            input_report_len: caps.InputReportByteLength,
            output_report_len: caps.OutputReportByteLength,
            feature_report_len: caps.FeatureReportByteLength,
            product,
            manufacturer,
            has_serial,
            report_items,
        })
    }
}

type StringFn = unsafe fn(HANDLE, *mut c_void, u32) -> windows::Win32::Foundation::BOOLEAN;

unsafe fn read_string(handle: HANDLE, f: StringFn) -> Option<String> {
    let mut buf = [0u16; 256];
    if f(handle, buf.as_mut_ptr() as *mut c_void, (buf.len() * 2) as u32).as_bool() {
        let s = wide_to_string(&buf);
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

/// Reads declared report items from the parsed descriptor. Sends nothing.
unsafe fn collect_report_items(prep: PHIDP_PREPARSED_DATA, caps: &HIDP_CAPS) -> Vec<ReportItem> {
    let mut items = Vec::new();

    let kinds: [(ReportKind, HIDP_REPORT_TYPE, u16, u16); 3] = [
        (ReportKind::Input, HidP_Input, caps.NumberInputValueCaps, caps.NumberInputButtonCaps),
        (ReportKind::Output, HidP_Output, caps.NumberOutputValueCaps, caps.NumberOutputButtonCaps),
        (ReportKind::Feature, HidP_Feature, caps.NumberFeatureValueCaps, caps.NumberFeatureButtonCaps),
    ];

    for (kind, rtype, n_values, n_buttons) in kinds {
        if n_values > 0 && n_values < 1024 {
            let mut len = n_values;
            let mut vc = vec![HIDP_VALUE_CAPS::default(); n_values as usize];
            if HidP_GetValueCaps(rtype, vc.as_mut_ptr(), &mut len, prep) == HIDP_STATUS_SUCCESS {
                for v in vc.iter().take(len as usize) {
                    let (lo, hi) = if v.IsRange.as_bool() {
                        (v.Anonymous.Range.UsageMin, v.Anonymous.Range.UsageMax)
                    } else {
                        (v.Anonymous.NotRange.Usage, v.Anonymous.NotRange.Usage)
                    };
                    items.push(ReportItem {
                        kind,
                        report_id: v.ReportID,
                        usage_page: v.UsagePage,
                        usage_min: lo,
                        usage_max: hi,
                        bit_size: v.BitSize,
                        report_count: v.ReportCount,
                        is_button: false,
                    });
                }
            }
        }

        if n_buttons > 0 && n_buttons < 1024 {
            let mut len = n_buttons;
            let mut bc = vec![HIDP_BUTTON_CAPS::default(); n_buttons as usize];
            if HidP_GetButtonCaps(rtype, bc.as_mut_ptr(), &mut len, prep) == HIDP_STATUS_SUCCESS {
                for b in bc.iter().take(len as usize) {
                    let (lo, hi) = if b.IsRange.as_bool() {
                        (b.Anonymous.Range.UsageMin, b.Anonymous.Range.UsageMax)
                    } else {
                        (b.Anonymous.NotRange.Usage, b.Anonymous.NotRange.Usage)
                    };
                    items.push(ReportItem {
                        kind,
                        report_id: b.ReportID,
                        usage_page: b.UsagePage,
                        usage_min: lo,
                        usage_max: hi,
                        bit_size: 0,
                        report_count: 0,
                        is_button: true,
                    });
                }
            }
        }
    }

    items
}
```

- [ ] **Step 2: Write the backend**

`crates/headset-device/src/windows/mod.rs`:

```rust
mod ffi;

use crate::backend::{HidBackend, HidTransport};
use crate::error::DeviceError;
use crate::model::{CollectionInfo, DeviceId, OpenMode};

pub struct WindowsHidBackend;

impl WindowsHidBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsHidBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl HidBackend for WindowsHidBackend {
    fn enumerate(&self) -> Result<Vec<CollectionInfo>, DeviceError> {
        let mut out = Vec::new();
        for path in ffi::enumerate_interface_paths()? {
            let Some(raw) = ffi::read_caps(&path) else {
                // A device that vanished or denied a descriptor open is skipped,
                // not fatal: enumeration must survive concurrent disconnects.
                tracing::debug!("skipping unreadable HID interface");
                continue;
            };
            let id = DeviceId::new(path);
            out.push(CollectionInfo {
                interface_number: id.interface_number(),
                collection_number: id.collection_number(),
                id,
                vendor_id: raw.vendor_id,
                product_id: raw.product_id,
                version: raw.version,
                usage_page: raw.usage_page,
                usage: raw.usage,
                input_report_len: raw.input_report_len,
                output_report_len: raw.output_report_len,
                feature_report_len: raw.feature_report_len,
                product: raw.product,
                manufacturer: raw.manufacturer,
                has_serial: raw.has_serial,
                report_items: raw.report_items,
            });
        }
        Ok(out)
    }

    fn open(
        &self,
        _id: &DeviceId,
        _mode: OpenMode,
    ) -> Result<Box<dyn HidTransport>, DeviceError> {
        // Implemented in Task 9. Deliberately absent until then so that no
        // caller can open a real device before the read path is reviewed.
        Err(DeviceError::Os("open is not implemented until Task 9".into()))
    }
}
```

- [ ] **Step 3: Export it**

Add to `crates/headset-device/src/lib.rs`:

```rust
#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use windows::WindowsHidBackend;
```

- [ ] **Step 4: Write a hardware-gated smoke test**

`crates/headset-device/tests/hardware_enumerate.rs`:

```rust
//! Hardware test. Requires a real device and is excluded from CI.
//! Run with: $env:HEADSET_HARDWARE_TESTS=1; cargo test -p headset-device -- --ignored

#![cfg(windows)]

use headset_device::{HidBackend, WindowsHidBackend};

fn hardware_enabled() -> bool {
    std::env::var("HEADSET_HARDWARE_TESTS").as_deref() == Ok("1")
}

#[test]
#[ignore = "requires attached hardware"]
fn enumerates_at_least_one_collection() {
    if !hardware_enabled() {
        eprintln!("skipping: set HEADSET_HARDWARE_TESTS=1 to run");
        return;
    }
    let all = WindowsHidBackend::new().enumerate().expect("enumeration succeeds");
    assert!(!all.is_empty(), "expected at least one HID collection");
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --workspace`
Expected: PASS, with the hardware test reported as ignored.

- [ ] **Step 6: Verify against real hardware manually**

Run:
```powershell
$env:HEADSET_HARDWARE_TESTS=1
cargo test -p headset-device --target x86_64-pc-windows-gnu -- --ignored --nocapture
```
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/headset-device
git commit -m "feat: add Windows HID enumeration backend"
```

---

### Task 5: Redaction and CLI argument parsing

**Files:**
- Create: `crates/headset-cli/src/redact.rs`
- Create: `crates/headset-cli/src/cli.rs`
- Modify: `crates/headset-cli/src/main.rs`

**Interfaces:**
- Consumes: `DeviceId` from Task 2.
- Produces: `Redactor::new(include_sensitive: bool)`, `Redactor::path(&DeviceId) -> String`, `Redactor::serial(bool) -> String`, `parse_u16_id(&str) -> Result<u16, String>`, and the clap `Cli`/`Command` types used by Tasks 6, 7, 10.

- [ ] **Step 1: Write the failing test**

`crates/headset-cli/src/redact.rs` (test module at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use headset_device::DeviceId;

    const PATH: &str = "\\\\?\\hid#vid_1532&pid_101b&mi_05&col04#7&2f9a1b&0&0000";

    #[test]
    fn redacted_path_hides_the_raw_value() {
        let out = Redactor::new(false).path(&DeviceId::new(PATH));
        assert!(!out.contains("2f9a1b"));
        assert!(out.starts_with("path:sha256:"));
    }

    #[test]
    fn redacted_path_is_stable_across_calls() {
        let id = DeviceId::new(PATH);
        assert_eq!(Redactor::new(false).path(&id), Redactor::new(false).path(&id));
    }

    #[test]
    fn different_paths_redact_differently() {
        let a = Redactor::new(false).path(&DeviceId::new(PATH));
        let b = Redactor::new(false).path(&DeviceId::new("\\\\?\\hid#vid_1532&pid_101b&mi_05&col02#7&2f9a1b&0&0001"));
        assert_ne!(a, b);
    }

    #[test]
    fn sensitive_mode_reveals_the_raw_path() {
        assert_eq!(Redactor::new(true).path(&DeviceId::new(PATH)), PATH);
    }

    #[test]
    fn serial_presence_is_reported_without_the_value() {
        assert_eq!(Redactor::new(false).serial(true), "<present, redacted>");
        assert_eq!(Redactor::new(false).serial(false), "<absent>");
    }

    #[test]
    fn redacted_digest_is_truncated_to_eight_hex_chars() {
        let out = Redactor::new(false).path(&DeviceId::new(PATH));
        let digest = out.strip_prefix("path:sha256:").unwrap();
        assert_eq!(digest.len(), 8);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
```

`crates/headset-cli/src/cli.rs` (test module at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_with_prefix() {
        assert_eq!(parse_u16_id("0x1532").unwrap(), 0x1532);
        assert_eq!(parse_u16_id("0X1532").unwrap(), 0x1532);
    }

    #[test]
    fn parses_bare_hex() {
        assert_eq!(parse_u16_id("1532").unwrap(), 0x1532);
        assert_eq!(parse_u16_id("101b").unwrap(), 0x101B);
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(parse_u16_id("0x10000").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_u16_id("zzzz").is_err());
        assert!(parse_u16_id("").is_err());
        assert!(parse_u16_id("-1").is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p headset-cli`
Expected: FAIL, `Redactor` and `parse_u16_id` undefined.

- [ ] **Step 3: Write the redactor**

`crates/headset-cli/src/redact.rs` (above the test module):

```rust
use headset_device::DeviceId;
use sha2::{Digest, Sha256};

/// Controls whether machine-identifying values reach the output.
#[derive(Clone, Copy, Debug)]
pub struct Redactor {
    include_sensitive: bool,
}

impl Redactor {
    pub fn new(include_sensitive: bool) -> Self {
        Self { include_sensitive }
    }

    pub fn include_sensitive(&self) -> bool {
        self.include_sensitive
    }

    /// Device paths identify a machine and a USB topology. By default they are
    /// reduced to a truncated, unsalted SHA-256 so records still correlate
    /// across runs and across bug reports without leaking the value.
    pub fn path(&self, id: &DeviceId) -> String {
        if self.include_sensitive {
            return id.raw().to_string();
        }
        let digest = Sha256::digest(id.raw().to_ascii_lowercase().as_bytes());
        format!("path:sha256:{:02x}{:02x}{:02x}{:02x}", digest[0], digest[1], digest[2], digest[3])
    }

    /// Presence is reported; the value never is unless explicitly requested.
    pub fn serial(&self, present: bool) -> String {
        match (present, self.include_sensitive) {
            (false, _) => "<absent>".to_string(),
            (true, false) => "<present, redacted>".to_string(),
            (true, true) => "<present>".to_string(),
        }
    }

    /// Header printed above any output that contains machine-identifying data.
    pub fn warning_banner(&self) -> Option<&'static str> {
        self.include_sensitive.then_some(
            "WARNING: --include-sensitive is set. This output contains machine-identifying \
             values. Do not paste it into a public issue.",
        )
    }
}
```

Note: the serial *value* is never read into the CLI at all — `CollectionInfo` carries
only `has_serial`. `--include-sensitive` reveals paths; it cannot reveal a serial the
device layer never surfaced.

- [ ] **Step 4: Write the CLI types**

`crates/headset-cli/src/cli.rs` (above the test module):

```rust
use clap::{Args, Parser, Subcommand};

/// Accepts `0x1532`, `1532`, and `101b`. Always hexadecimal: USB IDs are
/// universally written in hex, so a decimal reading would silently mislead.
pub fn parse_u16_id(s: &str) -> Result<u16, String> {
    let t = s.trim();
    let body = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("`{s}` is not a hexadecimal USB id"));
    }
    u16::from_str_radix(body, 16).map_err(|_| format!("`{s}` does not fit in 16 bits"))
}

#[derive(Parser, Debug)]
#[command(
    name = "headsetctl",
    about = "Experimental native Windows HID controller for supported wireless headset settings.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Emit machine-readable JSON instead of human-readable text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Reveal device paths. Output will contain machine-identifying data.
    #[arg(long, global = true)]
    pub include_sensitive: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Enumerate HID collections. Sends nothing and opens nothing for I/O.
    List(ListArgs),
    /// Read descriptors for one enumerated collection. Never writes.
    Inspect(InspectArgs),
    /// Read-only protocol probe. Performs no HID writes.
    Probe(ProbeArgs),
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Show only this vendor id, e.g. 0x1532.
    #[arg(long, value_parser = parse_u16_id)]
    pub vendor_id: Option<u16>,

    /// Show only this product id, e.g. 0x101b.
    #[arg(long, value_parser = parse_u16_id)]
    pub product_id: Option<u16>,
}

#[derive(Args, Debug)]
pub struct InspectArgs {
    /// Index from `headsetctl list`.
    #[arg(long)]
    pub path_index: usize,
}

#[derive(Args, Debug)]
pub struct ProbeArgs {
    /// Candidate index from `headsetctl list`. Defaults to the ranked best.
    #[arg(long)]
    pub candidate: Option<usize>,

    /// Milliseconds to listen for an unsolicited input report.
    #[arg(long, default_value_t = 2000)]
    pub listen_ms: u64,
}
```

- [ ] **Step 5: Wire main.rs**

`crates/headset-cli/src/main.rs`:

```rust
mod cli;
mod redact;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("HEADSETCTL_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = cli::Cli::parse();
    let _ = redact::Redactor::new(args.include_sensitive);
    match args.command {
        cli::Command::List(_) => println!("list: not implemented until Task 7"),
        cli::Command::Inspect(_) => println!("inspect: not implemented until Task 7"),
        cli::Command::Probe(_) => println!("probe: not implemented until Task 10"),
    }
    Ok(())
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p headset-cli`
Expected: 10 passed.

- [ ] **Step 7: Commit**

```bash
git add crates/headset-cli
git commit -m "feat: add redaction and CLI argument parsing"
```

---

### Task 6: Candidate ranking

**Files:**
- Create: `crates/headset-device/src/select.rs`
- Modify: `crates/headset-device/src/lib.rs`

**Interfaces:**
- Consumes: `CollectionInfo` from Task 2, `FakeHidBackend` from Task 3.
- Produces: `rank_candidates(&[CollectionInfo]) -> Vec<Candidate>` where `Candidate { index: usize, score: u32, reasons: Vec<String>, disqualified: Option<String> }`, and `stable_sort_collections(&mut Vec<CollectionInfo>)`. Tasks 7 and 10 consume both.

- [ ] **Step 1: Write the failing test**

`crates/headset-device/src/select.rs` (test module at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FakeHidBackend, HidBackend};

    const FIXTURE: &str = include_str!("../tests/fixtures/blackshark-v3-pro-ps.json");

    fn collections() -> Vec<CollectionInfo> {
        let mut all = FakeHidBackend::from_fixture_str(FIXTURE).unwrap().enumerate().unwrap();
        stable_sort_collections(&mut all);
        all
    }

    #[test]
    fn sorting_is_deterministic() {
        let a: Vec<String> = collections().iter().map(|c| c.id.raw().to_string()).collect();
        let b: Vec<String> = collections().iter().map(|c| c.id.raw().to_string()).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn best_candidate_is_the_64_byte_vendor_collection() {
        let all = collections();
        let ranked = rank_candidates(&all);
        let best = ranked.first().expect("at least one candidate");
        assert_eq!(all[best.index].usage_page, 0xFF14);
        assert_eq!(all[best.index].output_report_len, 64);
    }

    #[test]
    fn audio_collection_is_disqualified() {
        let all = collections();
        let ranked = rank_candidates(&all);
        let audio_pos = all.iter().position(|c| c.is_audio_stack_collection()).unwrap();
        let entry = ranked.iter().find(|r| r.index == audio_pos);
        assert!(entry.is_none() || entry.unwrap().disqualified.is_some());
    }

    #[test]
    fn non_vendor_collections_are_disqualified() {
        let all = collections();
        for c in rank_candidates(&all) {
            if c.disqualified.is_none() {
                assert!(all[c.index].is_vendor_defined());
            }
        }
    }

    #[test]
    fn both_vendor_collections_are_reported() {
        let all = collections();
        let qualified: Vec<_> =
            rank_candidates(&all).into_iter().filter(|c| c.disqualified.is_none()).collect();
        assert_eq!(qualified.len(), 2, "0xFF13 and 0xFF14 both qualify");
    }

    #[test]
    fn equal_scores_produce_no_automatic_winner() {
        let mut all = collections();
        // Force a tie: make 0xFF13 the same width as 0xFF14.
        for c in all.iter_mut() {
            if c.usage_page == 0xFF13 {
                c.output_report_len = 64;
                c.input_report_len = 64;
            }
        }
        let ranked = rank_candidates(&all);
        assert!(!has_unambiguous_winner(&ranked));
    }

    #[test]
    fn distinct_scores_produce_a_winner() {
        assert!(has_unambiguous_winner(&rank_candidates(&collections())));
    }

    #[test]
    fn empty_input_yields_no_candidates() {
        assert!(rank_candidates(&[]).is_empty());
        assert!(!has_unambiguous_winner(&[]));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p headset-device`
Expected: FAIL, `rank_candidates` undefined.

- [ ] **Step 3: Write the implementation**

`crates/headset-device/src/select.rs` (above the test module):

```rust
use crate::model::CollectionInfo;

/// One ranked enumeration entry, with the reasoning that produced its score.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Index into the sorted collection slice.
    pub index: usize,
    pub score: u32,
    pub reasons: Vec<String>,
    /// `Some` when the collection may never be used as a control channel.
    pub disqualified: Option<String>,
}

/// Deterministic ordering so that a diagnostic index means the same thing
/// across runs on the same machine with the same devices attached.
pub fn stable_sort_collections(all: &mut [CollectionInfo]) {
    all.sort_by(|a, b| {
        a.vendor_id
            .cmp(&b.vendor_id)
            .then(a.product_id.cmp(&b.product_id))
            .then(a.interface_number.cmp(&b.interface_number))
            .then(a.collection_number.cmp(&b.collection_number))
            .then(a.usage_page.cmp(&b.usage_page))
            .then(a.usage.cmp(&b.usage))
            .then_with(|| a.id.raw().cmp(b.id.raw()))
    });
}

/// Ranks collections as control-channel candidates using descriptor evidence only.
///
/// Returns every collection, so callers can show why a collection was rejected.
/// Disqualified entries always sort last.
pub fn rank_candidates(all: &[CollectionInfo]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = all
        .iter()
        .enumerate()
        .map(|(index, c)| {
            let mut reasons = Vec::new();
            let mut score = 0u32;

            if c.is_audio_stack_collection() {
                return Candidate {
                    index,
                    score: 0,
                    reasons: vec![
                        "telephony headset collection bound by the Windows audio stack".into(),
                    ],
                    disqualified: Some("reserved for Windows audio".into()),
                };
            }

            if !c.is_vendor_defined() {
                return Candidate {
                    index,
                    score: 0,
                    reasons: vec![format!("usage page {:#06x} is not vendor-defined", c.usage_page)],
                    disqualified: Some("not a vendor-defined usage page".into()),
                };
            }
            score += 100;
            reasons.push(format!("vendor-defined usage page {:#06x}", c.usage_page));

            let width = c.output_report_len.max(c.feature_report_len);
            if width == 0 {
                return Candidate {
                    index,
                    score: 0,
                    reasons: vec!["no output or feature reports declared".into()],
                    disqualified: Some("no writable report path".into()),
                };
            }
            score += u32::from(width);
            reasons.push(format!("declared report width {width} bytes"));

            if c.input_report_len > 0 {
                score += 10;
                reasons.push(format!(
                    "bidirectional: input report width {} bytes",
                    c.input_report_len
                ));
            }

            Candidate { index, score, reasons, disqualified: None }
        })
        .collect();

    out.sort_by(|a, b| {
        a.disqualified
            .is_some()
            .cmp(&b.disqualified.is_some())
            .then(b.score.cmp(&a.score))
            .then(a.index.cmp(&b.index))
    });
    out
}

/// True when exactly one qualified candidate has the top score.
///
/// A tie is never broken automatically: two equally plausible vendor channels
/// mean the evidence is insufficient, and guessing is worse than asking.
pub fn has_unambiguous_winner(ranked: &[Candidate]) -> bool {
    let qualified: Vec<&Candidate> = ranked.iter().filter(|c| c.disqualified.is_none()).collect();
    match qualified.as_slice() {
        [] => false,
        [_only] => true,
        [first, second, ..] => first.score > second.score,
    }
}
```

- [ ] **Step 4: Export it**

Add to `crates/headset-device/src/lib.rs`:

```rust
pub mod select;
pub use select::{has_unambiguous_winner, rank_candidates, stable_sort_collections, Candidate};
```

Add `use crate::model::CollectionInfo;` to the test module import list if needed.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p headset-device`
Expected: 18 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/headset-device
git commit -m "feat: add descriptor-evidence candidate ranking"
```

---

### Task 7: `list` and `inspect` commands

**Files:**
- Create: `crates/headset-cli/src/render/mod.rs`, `render/human.rs`, `render/json.rs`
- Create: `crates/headset-cli/src/cmd/mod.rs`, `cmd/list.rs`, `cmd/inspect.rs`
- Modify: `crates/headset-cli/src/main.rs`
- Create: `crates/headset-cli/tests/render_snapshots.rs`

**Interfaces:**
- Consumes: `Redactor` from Task 5; `rank_candidates`, `stable_sort_collections`, `Candidate` from Task 6; `HidBackend`, `FakeHidBackend`, `CollectionInfo` from Tasks 2–3.
- Produces: `render_list(&[CollectionInfo], &[Candidate], &Redactor, bool) -> String` and `render_inspect(&CollectionInfo, &Redactor, bool) -> String`. Task 10 reuses `Redactor` and the JSON envelope shape.

- [ ] **Step 1: Write the failing snapshot test**

`crates/headset-cli/tests/render_snapshots.rs`:

```rust
use headset_cli::redact::Redactor;
use headset_cli::render::{render_inspect, render_list};
use headset_device::{
    rank_candidates, stable_sort_collections, CollectionInfo, FakeHidBackend, HidBackend,
};

const FIXTURE: &str =
    include_str!("../../headset-device/tests/fixtures/blackshark-v3-pro-ps.json");

fn sorted() -> Vec<CollectionInfo> {
    let mut all = FakeHidBackend::from_fixture_str(FIXTURE).unwrap().enumerate().unwrap();
    stable_sort_collections(&mut all);
    all
}

#[test]
fn list_human_output_is_stable() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let out = render_list(&all, &ranked, &Redactor::new(false), false);
    insta::assert_snapshot!("list_human", out);
}

#[test]
fn list_json_output_is_stable() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let out = render_list(&all, &ranked, &Redactor::new(false), true);
    insta::assert_snapshot!("list_json", out);
}

#[test]
fn list_json_has_the_documented_envelope() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let out = render_list(&all, &ranked, &Redactor::new(false), true);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert!(v["collections"].is_array());
    assert_eq!(v["collections"].as_array().unwrap().len(), 4);
    let first = &v["collections"][0];
    for key in [
        "index", "vendor_id", "product_id", "version", "interface_number",
        "collection_number", "usage_page", "usage", "input_report_len",
        "output_report_len", "feature_report_len", "product", "manufacturer",
        "serial", "path", "score", "disqualified",
    ] {
        assert!(first.get(key).is_some(), "missing key `{key}`");
    }
}

#[test]
fn no_output_contains_a_raw_path_by_default() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    for json in [false, true] {
        let out = render_list(&all, &ranked, &Redactor::new(false), json);
        assert!(!out.contains("fixture"), "raw path fragment leaked");
        assert!(!out.to_lowercase().contains("\\\\?\\hid#"), "raw path leaked");
    }
}

#[test]
fn inspect_human_output_is_stable() {
    let all = sorted();
    let control = all.iter().find(|c| c.usage_page == 0xFF14).unwrap();
    let out = render_inspect(control, &Redactor::new(false), false);
    insta::assert_snapshot!("inspect_human", out);
}

#[test]
fn inspect_json_reports_declared_report_ids() {
    let all = sorted();
    let control = all.iter().find(|c| c.usage_page == 0xFF14).unwrap();
    let out = render_inspect(control, &Redactor::new(false), true);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    let items = v["report_items"].as_array().unwrap();
    assert!(items.iter().any(|i| i["report_id"] == 2 && i["kind"] == "output"));
}
```

`headset-cli` must expose a library target for this test. Add to
`crates/headset-cli/Cargo.toml`:

```toml
[lib]
name = "headset_cli"
path = "src/lib.rs"
```

and create `crates/headset-cli/src/lib.rs`:

```rust
pub mod cli;
pub mod cmd;
pub mod redact;
pub mod render;
```

`main.rs` then uses `headset_cli::…` rather than declaring the modules itself.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p headset-cli`
Expected: FAIL, `render_list` undefined.

- [ ] **Step 3: Write the JSON renderer**

`crates/headset-cli/src/render/json.rs`:

```rust
use headset_device::{Candidate, CollectionInfo};
use serde_json::{json, Value};

use crate::redact::Redactor;

/// Bump when a field is removed or its meaning changes. Adding a field does not
/// require a bump; consumers must tolerate unknown fields.
pub const SCHEMA_VERSION: u32 = 1;

pub fn collection_value(
    index: usize,
    c: &CollectionInfo,
    cand: Option<&Candidate>,
    r: &Redactor,
) -> Value {
    json!({
        "index": index,
        "vendor_id": format!("{:#06x}", c.vendor_id),
        "product_id": format!("{:#06x}", c.product_id),
        "version": format!("{:#06x}", c.version),
        "interface_number": c.interface_number,
        "collection_number": c.collection_number,
        "usage_page": format!("{:#06x}", c.usage_page),
        "usage": format!("{:#06x}", c.usage),
        "input_report_len": c.input_report_len,
        "output_report_len": c.output_report_len,
        "feature_report_len": c.feature_report_len,
        "product": c.product,
        "manufacturer": c.manufacturer,
        "serial": r.serial(c.has_serial),
        "path": r.path(&c.id),
        "score": cand.map(|x| x.score),
        "disqualified": cand.and_then(|x| x.disqualified.clone()),
        "reasons": cand.map(|x| x.reasons.clone()).unwrap_or_default(),
    })
}

pub fn render_list(all: &[CollectionInfo], ranked: &[Candidate], r: &Redactor) -> String {
    let by_index: std::collections::HashMap<usize, &Candidate> =
        ranked.iter().map(|c| (c.index, c)).collect();
    let collections: Vec<Value> = all
        .iter()
        .enumerate()
        .map(|(i, c)| collection_value(i, c, by_index.get(&i).copied(), r))
        .collect();

    let best = ranked
        .iter()
        .find(|c| c.disqualified.is_none())
        .filter(|_| headset_device::has_unambiguous_winner(ranked))
        .map(|c| c.index);

    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "include_sensitive": r.include_sensitive(),
        "collections": collections,
        "best_candidate_index": best,
    }))
    .expect("serialization cannot fail")
}

pub fn render_inspect(c: &CollectionInfo, r: &Redactor) -> String {
    let items: Vec<Value> = c
        .report_items
        .iter()
        .map(|i| {
            json!({
                "kind": i.kind,
                "report_id": i.report_id,
                "usage_page": format!("{:#06x}", i.usage_page),
                "usage_min": format!("{:#06x}", i.usage_min),
                "usage_max": format!("{:#06x}", i.usage_max),
                "bit_size": i.bit_size,
                "report_count": i.report_count,
                "is_button": i.is_button,
            })
        })
        .collect();

    let mut root = collection_value(0, c, None, r);
    root["schema_version"] = json!(SCHEMA_VERSION);
    root["report_items"] = json!(items);
    root["opened_for_io"] = json!(false);
    serde_json::to_string_pretty(&root).expect("serialization cannot fail")
}
```

- [ ] **Step 4: Write the human renderer**

`crates/headset-cli/src/render/human.rs`:

```rust
use std::fmt::Write as _;

use headset_device::{Candidate, CollectionInfo, ReportKind};

use crate::redact::Redactor;

pub fn render_list(all: &[CollectionInfo], ranked: &[Candidate], r: &Redactor) -> String {
    let mut s = String::new();
    if let Some(w) = r.warning_banner() {
        let _ = writeln!(s, "{w}\n");
    }

    if all.is_empty() {
        let _ = writeln!(s, "No HID collections found.");
        return s;
    }

    let by_index: std::collections::HashMap<usize, &Candidate> =
        ranked.iter().map(|c| (c.index, c)).collect();

    for (i, c) in all.iter().enumerate() {
        let _ = writeln!(s, "[{i}] {}", c.product.as_deref().unwrap_or("<no product string>"));
        let _ = writeln!(s, "     vendor/product : {:#06x} / {:#06x}", c.vendor_id, c.product_id);
        let _ = writeln!(
            s,
            "     interface/coll : {} / {}",
            c.interface_number.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            c.collection_number.map(|v| v.to_string()).unwrap_or_else(|| "-".into())
        );
        let _ = writeln!(s, "     usage page/usg : {:#06x} / {:#06x}", c.usage_page, c.usage);
        let _ = writeln!(
            s,
            "     reports in/out/feat : {} / {} / {}",
            c.input_report_len, c.output_report_len, c.feature_report_len
        );
        let _ = writeln!(s, "     serial         : {}", r.serial(c.has_serial));
        let _ = writeln!(s, "     path           : {}", r.path(&c.id));
        match by_index.get(&i) {
            Some(cand) if cand.disqualified.is_none() => {
                let _ = writeln!(s, "     candidate      : score {} ({})", cand.score, cand.reasons.join("; "));
            }
            Some(cand) => {
                let _ = writeln!(
                    s,
                    "     candidate      : excluded - {}",
                    cand.disqualified.as_deref().unwrap_or("unknown")
                );
            }
            None => {}
        }
        let _ = writeln!(s);
    }

    if headset_device::has_unambiguous_winner(ranked) {
        if let Some(best) = ranked.iter().find(|c| c.disqualified.is_none()) {
            let _ = writeln!(s, "Best control candidate: index {}", best.index);
        }
    } else {
        let _ = writeln!(
            s,
            "No unambiguous control candidate. Pass --candidate <index> explicitly."
        );
    }
    s
}

pub fn render_inspect(c: &CollectionInfo, r: &Redactor) -> String {
    let mut s = String::new();
    if let Some(w) = r.warning_banner() {
        let _ = writeln!(s, "{w}\n");
    }
    let _ = writeln!(s, "{}", c.product.as_deref().unwrap_or("<no product string>"));
    let _ = writeln!(s, "  manufacturer   : {}", c.manufacturer.as_deref().unwrap_or("-"));
    let _ = writeln!(s, "  vendor/product : {:#06x} / {:#06x}", c.vendor_id, c.product_id);
    let _ = writeln!(s, "  version        : {:#06x}", c.version);
    let _ = writeln!(s, "  usage page/usg : {:#06x} / {:#06x}", c.usage_page, c.usage);
    let _ = writeln!(
        s,
        "  reports in/out/feat : {} / {} / {}",
        c.input_report_len, c.output_report_len, c.feature_report_len
    );
    let _ = writeln!(s, "  serial         : {}", r.serial(c.has_serial));
    let _ = writeln!(s, "  path           : {}", r.path(&c.id));
    let _ = writeln!(s, "  opened for I/O : no (descriptor access only)");

    for kind in [ReportKind::Input, ReportKind::Output, ReportKind::Feature] {
        let ids = c.report_ids(kind);
        let _ = writeln!(
            s,
            "  {:?} report ids : {}",
            kind,
            if ids.is_empty() {
                "-".to_string()
            } else {
                ids.iter().map(|i| format!("{i:#04x}")).collect::<Vec<_>>().join(", ")
            }
        );
    }

    if !c.report_items.is_empty() {
        let _ = writeln!(s, "  declared items :");
        for i in &c.report_items {
            let _ = writeln!(
                s,
                "    {:?}{} id={:#04x} page={:#06x} usage={:#06x}..{:#06x} bits={} count={}",
                i.kind,
                if i.is_button { " button" } else { " value " },
                i.report_id,
                i.usage_page,
                i.usage_min,
                i.usage_max,
                i.bit_size,
                i.report_count
            );
        }
    }
    s
}
```

- [ ] **Step 5: Write the dispatcher**

`crates/headset-cli/src/render/mod.rs`:

```rust
pub mod human;
pub mod json;

use headset_device::{Candidate, CollectionInfo};

use crate::redact::Redactor;

pub fn render_list(
    all: &[CollectionInfo],
    ranked: &[Candidate],
    r: &Redactor,
    as_json: bool,
) -> String {
    if as_json {
        json::render_list(all, ranked, r)
    } else {
        human::render_list(all, ranked, r)
    }
}

pub fn render_inspect(c: &CollectionInfo, r: &Redactor, as_json: bool) -> String {
    if as_json {
        json::render_inspect(c, r)
    } else {
        human::render_inspect(c, r)
    }
}
```

- [ ] **Step 6: Write the commands**

`crates/headset-cli/src/cmd/mod.rs`:

```rust
pub mod inspect;
pub mod list;
```

`crates/headset-cli/src/cmd/list.rs`:

```rust
use anyhow::Result;
use headset_device::{rank_candidates, stable_sort_collections, CollectionInfo, HidBackend};

use crate::cli::ListArgs;
use crate::redact::Redactor;
use crate::render;

/// Enumerates and filters. Sends nothing; opens nothing for I/O.
pub fn run(
    backend: &dyn HidBackend,
    args: &ListArgs,
    r: &Redactor,
    as_json: bool,
) -> Result<String> {
    let mut all: Vec<CollectionInfo> = backend.enumerate()?;
    if let Some(vid) = args.vendor_id {
        all.retain(|c| c.vendor_id == vid);
    }
    if let Some(pid) = args.product_id {
        all.retain(|c| c.product_id == pid);
    }
    stable_sort_collections(&mut all);
    let ranked = rank_candidates(&all);
    Ok(render::render_list(&all, &ranked, r, as_json))
}
```

`crates/headset-cli/src/cmd/inspect.rs`:

```rust
use anyhow::{bail, Result};
use headset_device::{stable_sort_collections, CollectionInfo, HidBackend, OpenMode};

use crate::cli::InspectArgs;
use crate::redact::Redactor;
use crate::render;

/// Opens the selected collection in `Descriptors` mode and closes it. Never writes.
pub fn run(
    backend: &dyn HidBackend,
    args: &InspectArgs,
    r: &Redactor,
    as_json: bool,
) -> Result<String> {
    let mut all: Vec<CollectionInfo> = backend.enumerate()?;
    stable_sort_collections(&mut all);

    let Some(c) = all.get(args.path_index) else {
        bail!(
            "index {} is out of range; `headsetctl list` reported {} collections",
            args.path_index,
            all.len()
        );
    };

    // Prove the handle can be acquired and released without I/O rights.
    // Failure here is informative, not fatal to reporting descriptors.
    match backend.open(&c.id, OpenMode::Descriptors) {
        Ok(_handle) => tracing::debug!("descriptor handle acquired and released"),
        Err(e) => tracing::warn!("descriptor open failed: {e}"),
    }

    Ok(render::render_inspect(c, r, as_json))
}
```

- [ ] **Step 7: Wire main.rs**

`crates/headset-cli/src/main.rs`:

```rust
use anyhow::Result;
use clap::Parser;
use headset_cli::{cli, cmd, redact::Redactor};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("HEADSETCTL_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = cli::Cli::parse();
    let r = Redactor::new(args.include_sensitive);

    #[cfg(windows)]
    let backend = headset_device::WindowsHidBackend::new();
    #[cfg(not(windows))]
    compile_error!("headsetctl targets Windows only");

    let out = match &args.command {
        cli::Command::List(a) => cmd::list::run(&backend, a, &r, args.json)?,
        cli::Command::Inspect(a) => cmd::inspect::run(&backend, a, &r, args.json)?,
        cli::Command::Probe(_) => "probe: not implemented until Task 10".to_string(),
    };
    print!("{out}");
    Ok(())
}
```

- [ ] **Step 8: Accept the snapshots**

Run: `cargo test -p headset-cli`
Expected: FAIL on first run with pending snapshots.

Run: `cargo insta accept` (install with `cargo install cargo-insta` if absent), or
review `crates/headset-cli/tests/snapshots/*.snap.new` and rename to `.snap`.

Then run: `cargo test -p headset-cli`
Expected: 6 passed.

- [ ] **Step 9: Verify against real hardware**

Run:
```powershell
cargo run -p headset-cli --target x86_64-pc-windows-gnu -- list
cargo run -p headset-cli --target x86_64-pc-windows-gnu -- list --vendor-id 0x1532
cargo run -p headset-cli --target x86_64-pc-windows-gnu -- list --json
```
Expected: four collections for vendor `0x1532`; best candidate is the `0xFF14`,
64-byte collection; no raw path appears in the output.

Confirm audio still works: play audio during the run and verify it is uninterrupted.

- [ ] **Step 10: Commit**

```bash
git add crates/headset-cli
git commit -m "feat: add safe Windows HID enumeration"
```

---

### Task 8: Record device research findings

**Files:**
- Modify: `docs/device-research.md`

**Interfaces:**
- Consumes: real output from Task 7.
- Produces: the written rationale that Task 10's candidate choice cites.

- [ ] **Step 1: Write the findings document**

`docs/device-research.md` must contain, at minimum:

1. The Verified Hardware Baseline table from this plan, with a note that it was
   produced by our own `HidP_GetCaps` and `HidP_GetValueCaps` readings.
2. A **Control collection selection** section stating that `COL04` is selected,
   citing: vendor usage page `0xFF14`; 64-byte input and output reports; declared
   report ID `0x02` on both directions; and that `COL02` (`0xFF13`, 62-byte,
   asymmetric report IDs `0x07` in / `0x06` out) is a qualified but lower-scoring
   alternative that remains reachable via `--candidate`.
3. A **Transport** section stating: feature report length is `0` on every
   collection, therefore the control transport is interrupt output plus interrupt
   input. Report ID `0x02` occupies byte 0 of a 64-byte buffer, leaving 63 payload
   bytes. This was determined from descriptors, not from prior art.
4. A **Hypothesis register** table with columns: hypothesis, request layout,
   expected response length, checksum hypothesis, confidence, evidence. Every row
   in Phase 1 has confidence `unverified` except the descriptor facts above.
5. An **Unknown bytes** policy statement: bytes whose meaning is not established
   are recorded as `unknown` and never assigned invented meanings.
6. A **Blocker** section, written honestly:

```markdown
## Blocker: no known-safe request exists yet

Phase 1B was specified as "exchange one known-safe request and response". On this
hardware no such request is known:

- The device's PID (`0x101B`) differs from the PID targeted by the public Linux
  prior art (`0x0577`), so its command set cannot be assumed to apply.
- We have no vendor documentation.
- Sending a guessed command identifier would violate the project rule against
  speculative HID writes.

Therefore Phase 1B implements a **passive probe only**: it opens the control
collection for reading and listens for unsolicited input reports. This is
genuinely read-only and cannot alter device state.

Three paths exist to obtain a known-safe request. Each requires an explicit
decision before any write is implemented:

1. **Observe the vendor software.** Capture the HID traffic the manufacturer's
   own configuration software sends (for example with USBPcap and Wireshark) and
   document the observed request/response pairs as behavioral facts. This is the
   standard interoperability method and keeps the clean-room posture intact,
   because it records observed behavior rather than copying code.
2. **Obtain vendor documentation.** See `docs/manufacturer-contact-draft.md`
   when it is written in the public-release-readiness phase.
3. **Accept risk on a hypothesis.** Not recommended, and not permitted under the
   current project rules without an explicit written decision recorded here.
```

- [ ] **Step 2: Commit**

```bash
git add docs/device-research.md
git commit -m "docs: record verified descriptor findings and probe blocker"
```

---

### Task 9: Read-only Windows transport

**Files:**
- Create: `crates/headset-device/src/windows/transport.rs`
- Modify: `crates/headset-device/src/windows/mod.rs`, `crates/headset-device/src/windows/ffi.rs`

**Interfaces:**
- Consumes: `HidTransport` from Task 3, `DeviceError` from Task 2.
- Produces: a working `WindowsHidBackend::open`. `WindowsTransport` implements `read_report` only; it has no write method by construction.

- [ ] **Step 1: Add the opening helper to ffi.rs**

Append the functions below to `crates/headset-device/src/windows/ffi.rs`, but move the
`use` statements up into the existing import block at the top of the file rather than
leaving them mid-file — `cargo fmt` will not do this for you and `clippy` will not
complain, so it is easy to miss.

```rust
use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_BUSY, WAIT_TIMEOUT};
use windows::Win32::Storage::FileSystem::{ReadFile, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::IO::{CancelIo, GetOverlappedResult, OVERLAPPED};

/// Opens a collection for reading only. Never requests write access.
pub fn open_for_read(path: &str) -> Result<HANDLE, DeviceError> {
    unsafe {
        let mut wide: Vec<u16> = path.encode_utf16().collect();
        wide.push(0);
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )
        .map_err(map_open_error)
    }
}

/// Opens with `dwDesiredAccess = 0`. Cannot perform I/O.
pub fn open_for_descriptors(path: &str) -> Result<HANDLE, DeviceError> {
    unsafe {
        let mut wide: Vec<u16> = path.encode_utf16().collect();
        wide.push(0);
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
        .map_err(map_open_error)
    }
}

fn map_open_error(e: windows::core::Error) -> DeviceError {
    match e.code().0 as u32 & 0xFFFF {
        c if c == ERROR_ACCESS_DENIED.0 => DeviceError::AccessDenied,
        c if c == ERROR_BUSY.0 => DeviceError::Busy,
        _ => DeviceError::Os(e.to_string()),
    }
}

pub fn close(handle: HANDLE) {
    unsafe {
        let _ = CloseHandle(handle);
    }
}

/// Overlapped read with a hard deadline. Cancels the I/O on timeout so the
/// handle can be closed cleanly rather than leaked.
pub fn read_with_timeout(
    handle: HANDLE,
    buf: &mut [u8],
    timeout: std::time::Duration,
) -> Result<usize, DeviceError> {
    unsafe {
        let event = CreateEventW(None, true, false, PCWSTR::null())
            .map_err(|e| DeviceError::Os(e.to_string()))?;
        let mut ov = OVERLAPPED { hEvent: event, ..Default::default() };

        let mut read: u32 = 0;
        let started = ReadFile(handle, Some(buf), Some(&mut read), Some(&mut ov));

        if started.is_ok() {
            let _ = CloseHandle(event);
            return Ok(read as usize);
        }

        let ms = timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        let wait = WaitForSingleObject(event, ms);
        if wait == WAIT_TIMEOUT {
            let _ = CancelIo(handle);
            let _ = CloseHandle(event);
            return Err(DeviceError::Timeout(timeout));
        }

        let mut transferred: u32 = 0;
        let ok = GetOverlappedResult(handle, &ov, &mut transferred, false);
        let _ = CloseHandle(event);
        match ok {
            Ok(()) => Ok(transferred as usize),
            Err(e) => Err(DeviceError::Os(e.to_string())),
        }
    }
}
```

- [ ] **Step 2: Write the transport**

`crates/headset-device/src/windows/transport.rs`:

```rust
use std::time::Duration;

use windows::Win32::Foundation::HANDLE;

use super::ffi;
use crate::backend::HidTransport;
use crate::error::DeviceError;

/// A read-only handle to one HID collection.
///
/// There is deliberately no write method. Adding one is a reviewed change that
/// belongs to the write phase, not to Phase 1.
pub struct WindowsTransport {
    handle: HANDLE,
    input_report_len: u16,
}

impl WindowsTransport {
    pub(super) fn new(handle: HANDLE, input_report_len: u16) -> Self {
        Self { handle, input_report_len }
    }
}

impl HidTransport for WindowsTransport {
    fn read_report(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, DeviceError> {
        // Windows requires the read buffer to be at least the declared input
        // report length. A short buffer yields ERROR_INVALID_USER_BUFFER, so
        // reject it here with a message that explains the real constraint.
        if buf.len() < self.input_report_len as usize {
            return Err(DeviceError::UnexpectedDescriptor(format!(
                "read buffer is {} bytes; the collection declares {}",
                buf.len(),
                self.input_report_len
            )));
        }
        ffi::read_with_timeout(self.handle, buf, timeout)
    }

    fn input_report_len(&self) -> u16 {
        self.input_report_len
    }
}

impl Drop for WindowsTransport {
    fn drop(&mut self) {
        ffi::close(self.handle);
    }
}
```

A descriptor-only handle needs the same deterministic close. Add to the same file:

```rust
/// A handle opened with no I/O rights. Exists to prove the collection can be
/// opened and released; it cannot read or write.
pub struct DescriptorHandle {
    handle: HANDLE,
    input_report_len: u16,
}

impl DescriptorHandle {
    pub(super) fn new(handle: HANDLE, input_report_len: u16) -> Self {
        Self { handle, input_report_len }
    }
}

impl HidTransport for DescriptorHandle {
    fn read_report(&self, _buf: &mut [u8], _timeout: Duration) -> Result<usize, DeviceError> {
        Err(DeviceError::AccessDenied)
    }

    fn input_report_len(&self) -> u16 {
        self.input_report_len
    }
}

impl Drop for DescriptorHandle {
    fn drop(&mut self) {
        ffi::close(self.handle);
    }
}
```

- [ ] **Step 3: Implement `open`**

Replace the stub in `crates/headset-device/src/windows/mod.rs`:

```rust
mod ffi;
mod transport;

use crate::backend::{HidBackend, HidTransport};
use crate::error::DeviceError;
use crate::model::{CollectionInfo, DeviceId, OpenMode};
use transport::{DescriptorHandle, WindowsTransport};

// ... enumerate() unchanged ...

    fn open(
        &self,
        id: &DeviceId,
        mode: OpenMode,
    ) -> Result<Box<dyn HidTransport>, DeviceError> {
        let info = self
            .enumerate()?
            .into_iter()
            .find(|c| c.id == *id)
            .ok_or(DeviceError::DongleNotFound)?;

        if mode == OpenMode::ReadWrite && info.is_audio_stack_collection() {
            return Err(DeviceError::RefusedAudioCollection);
        }

        match mode {
            OpenMode::Descriptors => {
                let h = ffi::open_for_descriptors(info.id.raw())?;
                Ok(Box::new(DescriptorHandle::new(h, info.input_report_len)))
            }
            OpenMode::ReadWrite => {
                // Phase 1 grants read access only, never write access, even for
                // OpenMode::ReadWrite. Write access arrives with the write phase.
                let h = ffi::open_for_read(info.id.raw())?;
                Ok(Box::new(WindowsTransport::new(h, info.input_report_len)))
            }
        }
    }
```

- [ ] **Step 4: Write a hardware-gated read test**

`crates/headset-device/tests/hardware_transport.rs`:

```rust
//! Hardware test. Read-only. Excluded from CI.

#![cfg(windows)]

use std::time::Duration;

use headset_device::{HidBackend, OpenMode, WindowsHidBackend};

fn hardware_enabled() -> bool {
    std::env::var("HEADSET_HARDWARE_TESTS").as_deref() == Ok("1")
}

#[test]
#[ignore = "requires attached hardware"]
fn descriptor_handle_opens_and_closes() {
    if !hardware_enabled() {
        eprintln!("skipping: set HEADSET_HARDWARE_TESTS=1 to run");
        return;
    }
    let backend = WindowsHidBackend::new();
    let all = backend.enumerate().unwrap();
    let target = all.iter().find(|c| c.usage_page == 0xFF14).expect("control collection present");
    let h = backend.open(&target.id, OpenMode::Descriptors).expect("descriptor open succeeds");
    assert_eq!(h.input_report_len(), 64);
}

#[test]
#[ignore = "requires attached hardware"]
fn read_times_out_cleanly_when_device_is_silent() {
    if !hardware_enabled() {
        eprintln!("skipping: set HEADSET_HARDWARE_TESTS=1 to run");
        return;
    }
    let backend = WindowsHidBackend::new();
    let all = backend.enumerate().unwrap();
    let target = all.iter().find(|c| c.usage_page == 0xFF14).unwrap();
    let t = backend.open(&target.id, OpenMode::ReadWrite).expect("read open succeeds");
    let mut buf = vec![0u8; t.input_report_len() as usize];
    match t.read_report(&mut buf, Duration::from_millis(500)) {
        Ok(n) => assert!(n <= buf.len()),
        Err(headset_device::DeviceError::Timeout(_)) => {}
        Err(e) => panic!("unexpected error: {e}"),
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --workspace`
Expected: PASS with hardware tests ignored.

Run with hardware:
```powershell
$env:HEADSET_HARDWARE_TESTS=1
cargo test -p headset-device --target x86_64-pc-windows-gnu -- --ignored --nocapture
```
Expected: PASS. Confirm audio playback is uninterrupted during the run.

- [ ] **Step 6: Commit**

```bash
git add crates/headset-device
git commit -m "feat: add read-only Windows HID transport with bounded timeout"
```

---

### Task 10: Read-only `probe` command

**Files:**
- Create: `crates/headset-protocol/src/frame.rs`, `crates/headset-protocol/src/error.rs`
- Modify: `crates/headset-protocol/src/lib.rs`
- Create: `crates/headset-cli/src/cmd/probe.rs`
- Modify: `crates/headset-cli/src/cmd/mod.rs`, `crates/headset-cli/src/main.rs`

**Interfaces:**
- Consumes: `HidTransport`, `rank_candidates`, `has_unambiguous_winner`, `Redactor`.
- Produces: `ControlFrame::parse(&[u8]) -> Result<ControlFrame, ProtocolError>`, `ProbeOp` allowlist enum, `cmd::probe::run(...)`.

- [ ] **Step 1: Write the failing protocol test**

`crates/headset-protocol/src/frame.rs` (test module at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn buf(id: u8, len: usize) -> Vec<u8> {
        let mut v = vec![0u8; len];
        v[0] = id;
        v
    }

    #[test]
    fn parses_a_well_formed_control_frame() {
        let mut raw = buf(CONTROL_REPORT_ID, CONTROL_REPORT_LEN);
        raw[1] = 0xAB;
        let f = ControlFrame::parse(&raw).unwrap();
        assert_eq!(f.report_id, CONTROL_REPORT_ID);
        assert_eq!(f.payload.len(), CONTROL_PAYLOAD_LEN);
        assert_eq!(f.payload[0], 0xAB);
    }

    #[test]
    fn rejects_a_short_buffer() {
        let raw = buf(CONTROL_REPORT_ID, 10);
        assert!(matches!(
            ControlFrame::parse(&raw),
            Err(ProtocolError::UnexpectedLength { expected: CONTROL_REPORT_LEN, actual: 10 })
        ));
    }

    #[test]
    fn rejects_an_oversized_buffer() {
        let raw = buf(CONTROL_REPORT_ID, CONTROL_REPORT_LEN + 1);
        assert!(matches!(ControlFrame::parse(&raw), Err(ProtocolError::UnexpectedLength { .. })));
    }

    #[test]
    fn rejects_an_empty_buffer() {
        assert!(matches!(ControlFrame::parse(&[]), Err(ProtocolError::UnexpectedLength { .. })));
    }

    #[test]
    fn rejects_an_unexpected_report_id() {
        let raw = buf(0x07, CONTROL_REPORT_LEN);
        assert!(matches!(
            ControlFrame::parse(&raw),
            Err(ProtocolError::UnexpectedReportId { expected: CONTROL_REPORT_ID, actual: 0x07 })
        ));
    }

    #[test]
    fn every_payload_byte_starts_unknown() {
        let raw = buf(CONTROL_REPORT_ID, CONTROL_REPORT_LEN);
        let f = ControlFrame::parse(&raw).unwrap();
        assert_eq!(f.known_fields().len(), 0, "no payload semantics are established yet");
    }

    #[test]
    fn hex_dump_covers_the_whole_payload() {
        let mut raw = buf(CONTROL_REPORT_ID, CONTROL_REPORT_LEN);
        raw[CONTROL_REPORT_LEN - 1] = 0xFF;
        let f = ControlFrame::parse(&raw).unwrap();
        let dump = f.hex_payload();
        assert!(dump.ends_with("ff"));
        assert_eq!(dump.split(' ').count(), CONTROL_PAYLOAD_LEN);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p headset-protocol`
Expected: FAIL, `ControlFrame` undefined.

- [ ] **Step 3: Write the protocol types**

`crates/headset-protocol/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("expected a {expected}-byte report, got {actual}")]
    UnexpectedLength { expected: usize, actual: usize },

    #[error("expected report id {expected:#04x}, got {actual:#04x}")]
    UnexpectedReportId { expected: u8, actual: u8 },
}
```

`crates/headset-protocol/src/frame.rs` (above the test module):

```rust
use crate::error::ProtocolError;

/// Report ID declared by the control collection for both input and output.
/// Measured from the device's own report descriptor, not assumed.
pub const CONTROL_REPORT_ID: u8 = 0x02;

/// Total report length in bytes, including the leading report-ID byte.
pub const CONTROL_REPORT_LEN: usize = 64;

/// Payload length after the report-ID byte.
pub const CONTROL_PAYLOAD_LEN: usize = CONTROL_REPORT_LEN - 1;

/// A validated 64-byte control report.
///
/// This models the *container* only. No payload semantics are established for
/// this hardware, so no field is interpreted. Bytes are surfaced verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlFrame {
    pub report_id: u8,
    pub payload: [u8; CONTROL_PAYLOAD_LEN],
}

impl ControlFrame {
    /// Validates length and report id before touching the payload.
    pub fn parse(raw: &[u8]) -> Result<Self, ProtocolError> {
        if raw.len() != CONTROL_REPORT_LEN {
            return Err(ProtocolError::UnexpectedLength {
                expected: CONTROL_REPORT_LEN,
                actual: raw.len(),
            });
        }
        if raw[0] != CONTROL_REPORT_ID {
            return Err(ProtocolError::UnexpectedReportId {
                expected: CONTROL_REPORT_ID,
                actual: raw[0],
            });
        }
        let mut payload = [0u8; CONTROL_PAYLOAD_LEN];
        payload.copy_from_slice(&raw[1..]);
        Ok(Self { report_id: raw[0], payload })
    }

    /// Payload fields whose meaning is established. Empty by design: nothing is
    /// known yet, and inventing meanings would corrupt the research record.
    pub fn known_fields(&self) -> Vec<(&'static str, u8)> {
        Vec::new()
    }

    /// Space-separated lowercase hex of every payload byte.
    pub fn hex_payload(&self) -> String {
        self.payload.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
    }
}
```

`crates/headset-protocol/src/lib.rs`:

```rust
//! Pure protocol logic. No operating-system access.
#![forbid(unsafe_code)]

pub mod error;
pub mod frame;

pub use error::ProtocolError;
pub use frame::{ControlFrame, CONTROL_PAYLOAD_LEN, CONTROL_REPORT_ID, CONTROL_REPORT_LEN};
```

- [ ] **Step 4: Run protocol tests**

Run: `cargo test -p headset-protocol`
Expected: 7 passed.

- [ ] **Step 5: Write the probe command**

`crates/headset-cli/src/cmd/probe.rs`:

```rust
use std::time::Duration;

use anyhow::{bail, Result};
use headset_device::{
    has_unambiguous_winner, rank_candidates, stable_sort_collections, CollectionInfo, DeviceError,
    HidBackend, OpenMode,
};
use headset_protocol::ControlFrame;
use serde_json::json;

use crate::cli::ProbeArgs;
use crate::redact::Redactor;
use crate::render::json::SCHEMA_VERSION;

/// The allowlist of operations `probe` may perform.
///
/// Exactly one variant exists, and it performs no write. Adding a variant that
/// writes requires a documented, reviewed decision. See `docs/device-research.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeOp {
    /// Open the control collection for reading and listen for an unsolicited
    /// input report. Sends nothing to the device.
    PassiveListen,
}

/// Minimum spacing between repeated device requests, enforced even though the
/// current allowlist contains no request-emitting operation.
pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(250);

pub fn run(
    backend: &dyn HidBackend,
    args: &ProbeArgs,
    r: &Redactor,
    as_json: bool,
) -> Result<String> {
    let op = ProbeOp::PassiveListen;

    let mut all: Vec<CollectionInfo> = backend.enumerate()?;
    if all.is_empty() {
        bail!("no HID collections found; is the dongle connected?");
    }
    stable_sort_collections(&mut all);
    let ranked = rank_candidates(&all);

    let index = match args.candidate {
        Some(i) => i,
        None => {
            if !has_unambiguous_winner(&ranked) {
                bail!(
                    "no unambiguous control candidate; re-run with --candidate <index> \
                     after reviewing `headsetctl list`"
                );
            }
            ranked
                .iter()
                .find(|c| c.disqualified.is_none())
                .expect("a winner exists")
                .index
        }
    };

    let Some(target) = all.get(index) else {
        bail!("candidate index {index} is out of range; {} collections enumerated", all.len());
    };

    if let Some(entry) = ranked.iter().find(|c| c.index == index) {
        if let Some(reason) = &entry.disqualified {
            bail!("candidate {index} is disqualified: {reason}");
        }
    }
    if target.is_audio_stack_collection() {
        bail!("refusing to open the Windows audio collection");
    }

    // Bound the listen window so a silent device cannot hang the process.
    let timeout = Duration::from_millis(args.listen_ms.clamp(100, 30_000));

    let transport = backend.open(&target.id, OpenMode::ReadWrite)?;
    let declared = transport.input_report_len() as usize;
    if declared == 0 || declared > 1024 {
        bail!("collection declares an implausible input report length of {declared}");
    }
    let mut buf = vec![0u8; declared];

    let outcome = match transport.read_report(&mut buf, timeout) {
        Ok(n) => match ControlFrame::parse(&buf[..n]) {
            Ok(frame) => Outcome::Frame { bytes: n, hex: frame.hex_payload() },
            Err(e) => Outcome::Malformed { bytes: n, error: e.to_string() },
        },
        Err(DeviceError::Timeout(_)) => Outcome::Silent,
        Err(e) => return Err(e.into()),
    };
    drop(transport); // release the handle before rendering

    Ok(if as_json {
        render_json(op, index, target, &outcome, r, timeout)
    } else {
        render_human(op, index, target, &outcome, r, timeout)
    })
}

enum Outcome {
    Frame { bytes: usize, hex: String },
    Malformed { bytes: usize, error: String },
    Silent,
}

fn render_json(
    op: ProbeOp,
    index: usize,
    c: &CollectionInfo,
    outcome: &Outcome,
    r: &Redactor,
    timeout: Duration,
) -> String {
    let result = match outcome {
        Outcome::Frame { bytes, hex } => json!({
            "status": "frame_received",
            "bytes": bytes,
            "payload_hex": hex,
            "interpreted_fields": {},
            "note": "no payload semantics are established for this hardware"
        }),
        Outcome::Malformed { bytes, error } => json!({
            "status": "unexpected_frame", "bytes": bytes, "error": error
        }),
        Outcome::Silent => json!({
            "status": "silent",
            "note": "no unsolicited input report within the listen window"
        }),
    };

    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "operation": format!("{op:?}"),
        "wrote_to_device": false,
        "candidate_index": index,
        "usage_page": format!("{:#06x}", c.usage_page),
        "input_report_len": c.input_report_len,
        "path": r.path(&c.id),
        "listen_ms": timeout.as_millis(),
        "result": result,
    }))
    .expect("serialization cannot fail")
}

fn render_human(
    op: ProbeOp,
    index: usize,
    c: &CollectionInfo,
    outcome: &Outcome,
    r: &Redactor,
    timeout: Duration,
) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    if let Some(w) = r.warning_banner() {
        let _ = writeln!(s, "{w}\n");
    }
    let _ = writeln!(s, "probe operation : {op:?}");
    let _ = writeln!(s, "wrote to device : no");
    let _ = writeln!(s, "candidate       : [{index}] usage page {:#06x}", c.usage_page);
    let _ = writeln!(s, "path            : {}", r.path(&c.id));
    let _ = writeln!(s, "listen window   : {} ms", timeout.as_millis());
    match outcome {
        Outcome::Frame { bytes, hex } => {
            let _ = writeln!(s, "result          : received {bytes} bytes");
            let _ = writeln!(s, "payload         : {hex}");
            let _ = writeln!(s, "interpretation  : none - no payload semantics established");
        }
        Outcome::Malformed { bytes, error } => {
            let _ = writeln!(s, "result          : {bytes} bytes, failed validation");
            let _ = writeln!(s, "error           : {error}");
        }
        Outcome::Silent => {
            let _ = writeln!(s, "result          : silent (no unsolicited report)");
            let _ = writeln!(
                s,
                "note            : this is expected. The device may only emit reports in\n\
                 \x20                 response to a request, and no request is known to be safe\n\
                 \x20                 on this hardware yet. See docs/device-research.md."
            );
        }
    }
    s
}
```

- [ ] **Step 6: Wire it up**

Add `pub mod probe;` to `crates/headset-cli/src/cmd/mod.rs`, and in `main.rs` replace
the probe arm with:

```rust
        cli::Command::Probe(a) => cmd::probe::run(&backend, a, &r, args.json)?,
```

Make `SCHEMA_VERSION` reachable by ensuring `render::json` is `pub mod json;` in
`render/mod.rs`.

- [ ] **Step 7: Add a probe test over the fake backend**

`crates/headset-cli/tests/probe_fake.rs`:

```rust
use headset_cli::cli::ProbeArgs;
use headset_cli::cmd::probe;
use headset_cli::redact::Redactor;
use headset_device::{FakeHidBackend, HidBackend};

const FIXTURE: &str =
    include_str!("../../headset-device/tests/fixtures/blackshark-v3-pro-ps.json");

#[test]
fn probe_reports_silence_when_the_device_sends_nothing() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let args = ProbeArgs { candidate: None, listen_ms: 100 };
    let out = probe::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["wrote_to_device"], false);
    assert_eq!(v["result"]["status"], "silent");
}

#[test]
fn probe_parses_a_queued_control_frame() {
    let mut backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let all = backend.enumerate().unwrap();
    let control = all.iter().find(|c| c.usage_page == 0xFF14).unwrap();
    let mut report = vec![0u8; 64];
    report[0] = 0x02;
    report[1] = 0xDE;
    report[2] = 0xAD;
    backend.push_read(&control.id, report);

    let args = ProbeArgs { candidate: None, listen_ms: 100 };
    let out = probe::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["result"]["status"], "frame_received");
    assert!(v["result"]["payload_hex"].as_str().unwrap().starts_with("de ad"));
    assert_eq!(v["result"]["interpreted_fields"], serde_json::json!({}));
}

#[test]
fn probe_refuses_a_disqualified_candidate() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let all = backend.enumerate().unwrap();
    let mut sorted = all.clone();
    headset_device::stable_sort_collections(&mut sorted);
    let audio_index = sorted.iter().position(|c| c.is_audio_stack_collection()).unwrap();

    let args = ProbeArgs { candidate: Some(audio_index), listen_ms: 100 };
    let err = probe::run(&backend, &args, &Redactor::new(false), true).unwrap_err();
    assert!(err.to_string().contains("disqualified"));
}

#[test]
fn probe_rejects_an_out_of_range_candidate() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let args = ProbeArgs { candidate: Some(99), listen_ms: 100 };
    let err = probe::run(&backend, &args, &Redactor::new(false), true).unwrap_err();
    assert!(err.to_string().contains("out of range"));
}
```

- [ ] **Step 8: Run the full gate**

Run:
```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```
Expected: all pass.

- [ ] **Step 9: Verify against real hardware**

Run, with audio playing and a microphone recording:
```powershell
cargo run -p headset-cli --target x86_64-pc-windows-gnu -- probe
cargo run -p headset-cli --target x86_64-pc-windows-gnu -- probe --json
cargo run -p headset-cli --target x86_64-pc-windows-gnu -- probe --candidate 1 --listen-ms 5000
```
Expected: exits cleanly. `wrote_to_device` is `false`. Audio and microphone remain
uninterrupted. Record the observed result in `docs/device-research.md`, including
whether the device emits unsolicited reports on `0xFF14`, on `0xFF13`, or on neither.

- [ ] **Step 10: Commit**

```bash
git add crates docs
git commit -m "feat: add read-only HID protocol probe"
```

---

## Phase 1 Acceptance Criteria

- [ ] `headsetctl list` enumerates real hardware with paths and serials redacted.
- [ ] `headsetctl list --json` emits a stable, documented envelope.
- [ ] `headsetctl list --vendor-id 0x1532` filters correctly.
- [ ] `headsetctl inspect --path-index N` reports report lengths and declared report IDs without opening any handle for I/O.
- [ ] `headsetctl probe` opens the control collection read-only, validates any response, and exits cleanly.
- [ ] Windows playback and microphone capture remain functional throughout.
- [ ] No administrator rights required. No driver installed. No HID write performed.
- [ ] `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, and `cargo build --release` all pass.
- [ ] `docs/device-research.md` records the descriptor evidence, the selection rationale, and the known-safe-request blocker.

## Known Risks Carried Into the Next Phase

1. **No known-safe request exists.** The probe is passive by necessity. Writing anything requires first obtaining a request from traffic observation or vendor documentation. This blocks Prompt 4 (sidetone) until resolved, and is the single largest open item.
2. **The `windows` crate is pinned to 0.58.** Bumping it breaks the GNU target until MinGW binutils are installed or the project migrates to MSVC.
3. **`0xFF13` versus `0xFF14`.** `0xFF14` scores higher, but if it proves silent and unresponsive, `0xFF13` is the next candidate and is reachable via `--candidate`.
4. **PID `0x101B` is not the PID any public prior art targets.** Protocol transfer is unproven.
5. **"Headset powered off" and "wireless link unavailable" cannot be distinguished yet.** Both
   `DeviceError` variants exist and are wired, but nothing in Phase 1 can populate them: the
   dongle keeps presenting its HID collections whether or not the headset is awake, and
   detecting link state requires a protocol we do not have. Until then, a powered-off headset
   looks identical to a silent probe. Verify this explicitly by running `headsetctl list` and
   `headsetctl probe` with the headset switched off, and record the observed behavior in
   `docs/device-research.md` rather than assuming it.
