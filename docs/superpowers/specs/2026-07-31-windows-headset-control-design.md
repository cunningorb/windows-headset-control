# Design: windows-headset-control

**Date:** 2026-07-31
**Status:** Approved
**Scope of this spec:** Phase 1 groundwork through the read-only protocol probe (project prompts 1–3). Sidetone writes, packaging/signing, and the tray application are explicitly out of scope here and will get their own specs.

---

## 1. Purpose

A native Windows user-mode utility that reads and (later) controls supported settings of a wireless gaming headset over its proprietary HID control interface, without requiring the vendor's configuration software.

This is an unofficial interoperability utility. It does not imply endorsement, authorization, or affiliation with any manufacturer.

### 1.1 Non-goals

- No kernel driver, filter driver, or Windows service.
- No firmware reading, writing, or modification.
- No pairing-state modification.
- No telemetry and no runtime network access.
- No administrator privileges for any normal operation.
- No attempt to stop, disable, or interfere with vendor software that may be installed.

---

## 2. Verified hardware context

Enumerated on the development machine on 2026-07-31 via `Get-PnpDevice`, before any code was written. These are **observations**, not assumptions inherited from prior art:

| Device instance                  | Class     | Description                      |
| -------------------------------- | --------- | -------------------------------- |
| `USB\VID_1532&PID_101B`          | USB       | USB Composite Device (the dongle)|
| `USB\VID_1532&PID_101B&MI_00`    | MEDIA     | `BlackShark V3 Pro PS - Chat`    |
| `USB\VID_1532&PID_101B&MI_03`    | MEDIA     | `BlackShark V3 Pro PS - Game`    |
| `USB\VID_1532&PID_101B&MI_05`    | HIDClass  | USB Input Device (parent)        |
| `HID\...&MI_05&COL01`            | HIDClass  | HID-compliant consumer control   |
| `HID\...&MI_05&COL02`            | HIDClass  | **HID-compliant vendor-defined** |
| `HID\...&MI_05&COL03`            | HIDClass  | HID-compliant headset            |
| `HID\...&MI_05&COL04`            | HIDClass  | **HID-compliant vendor-defined** |

### 2.1 Consequences for the original hypotheses

| Starting hypothesis                 | Status after verification                                              |
| ----------------------------------- | ---------------------------------------------------------------------- |
| Vendor ID `0x1532`                  | **Confirmed** on this hardware.                                        |
| Product ID `0x0577`                 | **Refuted** on this hardware. Observed `0x101B` (a PlayStation-variant product). |
| Control collection on interface 5   | **Consistent so far.** Interface 5 hosts the vendor-defined collections, but it hosts *two* of them, so the interface number alone is insufficient. |
| 64-byte reports                     | **Unverified.** To be read from `HidP_GetCaps`, not assumed.           |
| Report ID `0x02`                    | **Unverified.**                                                        |
| Sidetone range 0–15                 | **Unverified.** Out of scope for this spec.                            |
| Firmware 1.3.x or newer required    | **Unverified.** Reading a firmware version is a candidate probe operation. |

Because the observed product ID differs from the one targeted by the publicly known Linux prior art, **no protocol detail from that prior art may be treated as applicable to this hardware.** Every protocol claim carries an explicit confidence level in `docs/device-research.md`.

### 2.2 Toolchain context

- `git` 2.55.0, `gh` 2.97.0 (authenticated) present.
- Visual Studio Community 2026 is installed **without** the C++ workload; no MSVC linker is available.
- Therefore the project targets **`x86_64-pc-windows-gnu`**, using the MinGW linker bundled with rustup. This avoids a multi-gigabyte, administrator-elevated build-tools install. Authenticode signing is unaffected — `signtool` signs a GNU-produced PE identically. Version-resource embedding will use a `windres`-compatible path.

---

## 3. Architecture

Four crates in a Cargo workspace, with the operating system confined to exactly one of them.

```
windows-headset-control/
├─ crates/
│  ├─ headset-protocol/   pure logic, zero OS access
│  ├─ headset-device/     Windows HID access behind a trait
│  ├─ headset-cli/        headsetctl.exe
│  └─ headset-tray/       Phase 2 placeholder
```

### 3.1 `headset-protocol`

No operating-system access whatsoever, and no dependency on `headset-device`. Contains report models, framing, checksum computation and verification, serialization, and parsing. This is where the unit-test mass lives, because every behavior here is testable without hardware.

Written from an original specification. No implementation is transcribed from any third-party source.

### 3.2 `headset-device`

Owns all Windows HID access, behind a mockable boundary:

```rust
pub trait HidBackend {
    fn enumerate(&self) -> Result<Vec<CollectionInfo>, DeviceError>;
    fn open(&self, id: &DeviceId, mode: OpenMode)
        -> Result<Box<dyn HidTransport>, DeviceError>;
}

pub enum OpenMode {
    /// CreateFile with dwDesiredAccess = 0. Descriptor queries only.
    /// Cannot perform I/O, therefore cannot contend with the audio stack.
    Descriptors,
    /// Read/write access. Only permitted for an identified control collection.
    ReadWrite,
}

pub trait HidTransport {
    fn write_report(&self, request: &[u8]) -> Result<(), DeviceError>;
    fn read_report(&self, buffer: &mut [u8], timeout: Duration)
        -> Result<usize, DeviceError>;
}
```

Two implementations: `WindowsHidBackend` (behind `#[cfg(windows)]`) and `FakeHidBackend` (fixture-driven, for tests).

`OpenMode` makes the central safety property a type-level fact rather than a convention: `inspect` is only permitted to request `Descriptors`, so it *cannot* disturb playback or capture.

### 3.3 `headset-cli`

`headsetctl.exe`. All filtering, ranking, redaction, and rendering live above the `HidBackend` trait, so they are fully testable with no hardware attached and CI can exercise them on any runner.

### 3.4 `headset-tray`

Phase 1 creates a placeholder crate only: a documented stub with no tray implementation. Phase 2 implements it.

---

## 4. HID access approach

**Decision: use the `windows` crate (windows-rs) directly. Do not use `hidapi`.**

Enumeration via SetupAPI (`SetupDiGetClassDevs` with `GUID_DEVINTERFACE_HID`); capabilities via `hid.dll` (`HidD_GetAttributes`, `HidD_GetPreparsedData`, `HidP_GetCaps`, `HidD_GetProductString`, `HidD_GetManufacturerString`, `HidD_GetSerialNumberString`); transport via `ReadFile`/`WriteFile` and `HidD_SetFeature`/`HidD_GetFeature`.

Rationale:

1. **`hidapi` cannot produce the required output.** The enumeration report must include input, output, and feature report lengths. The `hidapi` Rust binding exposes vendor ID, product ID, interface number, usage, usage page, serial number, release number, and strings — but not report lengths. Those come from `HidP_GetCaps`.
2. **`hidapi` reintroduces the C-toolchain dependency** we just designed around, since it compiles a C library through `cc`.
3. **`hidapi` opens devices with read/write access.** Windows permits opening a HID interface with `dwDesiredAccess = 0` and still reading every descriptor property. That is a categorically safer `inspect`, because it requests no I/O rights at all.

windows-rs is pure Rust bindings, needs no C compiler, and is the "Windows native HID backend" the project brief already preferred.

---

## 5. Candidate control-collection selection

Interface number alone cannot identify the control collection, because interface 5 exposes two vendor-defined collections (`COL02` and `COL04`) alongside a consumer-control collection and a HID headset collection.

`COL03` (HID-compliant headset) is the collection the Windows audio stack uses for headset controls. **It is never opened in `ReadWrite` mode.**

Candidates are ranked by evidence read from the descriptor:

1. Usage page in the vendor-defined range `0xFF00`–`0xFFFF`.
2. Non-zero feature report length **or** non-zero output report length.
3. Report length consistent with a substantial payload rather than a few control bytes.
4. Not a telephony, consumer, or generic-desktop usage page.

`headsetctl list` prints the ranking together with the reason for each score. `headsetctl probe --candidate <index>` overrides the automatic choice. The observed capabilities of both vendor-defined collections are recorded in `docs/device-research.md` as evidence, with the selection rationale written out.

### 5.1 Transport strategy is determined empirically

The descriptor settles the transport question rather than assumption: if `FeatureReportByteLength > 0` and `OutputReportByteLength == 0`, the control path is feature reports; if the reverse, it is interrupt output plus interrupt input. Whether a report-ID prefix byte is required, and whether the wire buffer differs in size from the application payload, is read from `HidP_GetCaps` and recorded. Transport selection sits behind `HidTransport` so protocol logic is unaffected by the answer.

---

## 6. Stable diagnostic index

Users refer to enumerated collections by a small integer, never by pasting a raw Windows HID path.

The index is assigned by sorting enumerated collections on a deterministic key: vendor ID, product ID, interface number, collection number, usage page, usage, then device path. This makes index *N* reproducible across runs on the same machine with the same devices attached.

The documentation states plainly that the index is **not** guaranteed stable across a replug into a different USB port, or when the set of attached devices changes.

---

## 7. Redaction

Default output redacts unique identifiers:

- Serial number renders as `<present, redacted>` or `<absent>` — presence is reported, value is not.
- Device paths render as `path:sha256:1a2b3c4d` — the first 8 hex characters of an unsalted SHA-256 of the full path. Unsalted so that records correlate across runs and across bug reports; truncated so it is not trivially reversible into a machine identifier.

`--include-sensitive` reveals full values and prints a prominent warning header stating that the output contains machine-identifying data.

Redaction is applied at the rendering layer for both human-readable and JSON output, and is unit-tested directly.

---

## 8. Error model

`headset-device` defines a `thiserror` enum whose variants match the distinctions the CLI must report separately, so each maps to a distinct actionable message rather than a generic failure:

- `DongleNotFound` — no matching USB device present.
- `WirelessLinkUnavailable` — dongle present, headset not reachable.
- `AccessDenied` — another process holds the collection, or ACLs deny access.
- `Busy` — device temporarily unavailable.
- `DisconnectedDuringOp` — device removed mid-operation.
- `Timeout` — no response within the deadline.
- `AmbiguousDevice` — more than one matching device; user must disambiguate.
- `ProtocolMismatch` — response failed length or checksum validation.
- `UnsupportedFirmware` — device identified but reports an unsupported version.
- `UnexpectedDescriptor` — descriptor values outside the expected shape.

`anyhow` is used only at executable boundaries (`headset-cli`'s `main`), never in libraries.

---

## 9. Commands (this spec's scope)

```powershell
headsetctl list
headsetctl list --json
headsetctl list --vendor-id 0x1532
headsetctl inspect --path-index 0
headsetctl inspect --path-index 0 --json
headsetctl probe
headsetctl probe --candidate <index>
headsetctl probe --json
```

- `list` performs enumeration only. It sends no reports and opens no handles for I/O.
- `inspect` may open a candidate in `Descriptors` mode and close it. It never writes.
- `probe` opens the control collection, exchanges exactly one allowlisted request, validates the response, and closes cleanly.

All handles are released deterministically via `Drop`.

### 9.1 Conditions that must be handled gracefully

Dongle absent; headset powered off; dongle present but wireless link unavailable; access denied; device busy; device disconnected during enumeration; more than one matching device; unexpected HID descriptor values.

---

## 10. Probe safety rules

- An explicit `enum ProbeOp` allowlist, containing exactly **one** variant initially (a read-only identification or version query).
- One request per process invocation by default.
- An enforced minimum delay between repeated requests.
- Response length validated before any parsing.
- Bytes whose meaning is not established are recorded literally as `unknown`. Meanings are never invented.
- No brute-force or batched scanning of guessed command identifiers, ever.
- This phase never alters firmware, pairing, EQ, ANC, THX, power settings, sidetone, or volume.

---

## 11. Testing strategy

| Layer                              | Approach                                                        |
| ---------------------------------- | --------------------------------------------------------------- |
| Filtering, ranking, redaction      | Unit tests over `FakeHidBackend`                                 |
| VID/PID parsing (`0x1532`, `1532`, decimal) | Unit tests including malformed input                     |
| JSON output                        | Schema-stability tests; field presence and type assertions       |
| Human-readable output              | `insta` snapshots, with no machine-specific identifiers captured |
| Protocol encode/decode             | Dense unit tests in `headset-protocol`                           |
| Hardware paths                     | `#[ignore]` **and** gated on `HEADSET_HARDWARE_TESTS=1`          |

Hardware integration tests never run on GitHub-hosted runners.

---

## 12. CI

`.github/workflows/ci.yml` on a pinned Windows runner:

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

Minimum required workflow permissions. No secrets exposed to pull requests.

---

## 13. Legal and provenance posture

- Repository stays **private**. Visibility changes only on explicit instruction.
- No open-source license file. `Copyright © 2026. All rights reserved.`
- Repository name, executable names, application icon, and publisher identity stay neutral. Manufacturer and product names appear only where needed to describe compatibility.
- `docs/clean-room-notes.md` records which public behavioral facts were consulted and affirms that implementation code was written independently.
- The publicly known Linux prior art is treated as read-only research material. It is not forked, and no source, comments, assets, README text, or project structure is copied from it.
- `THIRD_PARTY_NOTICES.md` is maintained from the start.

---

## 14. Definition of done for this spec

1. Workspace builds clean on `x86_64-pc-windows-gnu`.
2. `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test --workspace` all pass.
3. `headsetctl list` enumerates the real dongle, with serials and paths redacted by default.
4. `headsetctl inspect --path-index N` reports capabilities including report lengths, without opening any handle for I/O.
5. `headsetctl probe` completes one allowlisted read-only exchange and exits cleanly.
6. Windows playback and microphone capture remain functional throughout.
7. No administrator rights required; no driver installed.
8. `docs/device-research.md` records observed capabilities for both vendor-defined collections, the selection rationale, and the verified transport behavior.

---

## 15. Open questions carried forward

- Which of `COL02` / `COL04` is the control collection. Resolved by descriptor evidence during implementation.
- Whether the control path uses feature reports or interrupt output/input.
- Whether a report-ID prefix byte is required, and the true report length.
- Whether this PlayStation-variant product (`0x101B`) speaks the same protocol as the product the prior art targeted (`0x0577`). Assume not until demonstrated.
- The firmware version of the attached hardware, and whether any version gate is real.
