# Architecture

## Crates

| Crate | Responsibility | May touch the OS |
| ----- | -------------- | ----------------- |
| `headset-protocol` | Pure protocol logic: report models, framing, serialization, and parsing. Checksum computation and verification is planned; no checksum scheme is established for this hardware — see docs/device-research.md. No dependency on `headset-device`. | No |
| `headset-device` | All Windows HID access behind a mockable `HidBackend` trait. Real windows-rs implementation (`WindowsHidBackend`) and a fixture-driven fake (`FakeHidBackend`) for tests. | Yes — the only crate that may |
| `headset-cli` | `headsetctl.exe`. Command dispatch, filtering, ranking, redaction, and rendering, all built above the `HidBackend` trait so they are testable without hardware. | No (delegates to `headset-device`) |
| `headset-tray` | Phase 2 placeholder for the Windows tray application. Intentionally empty in Phase 1. | No |

## `HidBackend` / `HidTransport` / `OpenMode`

These are the trait and type signatures `headset-device` implements, as designed for
Task 3:

```rust
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

pub enum OpenMode {
    /// CreateFile with dwDesiredAccess = 0. Descriptor queries only.
    /// Cannot perform I/O, therefore cannot contend with the audio stack.
    Descriptors,
    /// Named `ReadWrite` for the mode this variant will eventually grant, but
    /// Phase 1 never requests write access for it: `WindowsHidBackend::open`
    /// calls `ffi::open_for_read`, which requests `FILE_GENERIC_READ` only,
    /// for this variant regardless of the name. Write access arrives with
    /// the write phase, alongside a `HidTransport::write_report` method that
    /// does not exist yet.
    ReadWrite,
}
```

`HidTransport` deliberately exposes no write method in Phase 1: no caller can perform a
HID write before the write phase is designed and approved.

## Candidate selection and the supported-device allowlist

`crates/headset-device/src/select.rs` provides two independent pieces of logic that
`headset-cli`'s commands compose:

- `rank_candidates` scores every enumerated collection purely from descriptor shape
  (vendor-defined usage page, declared report width, bidirectionality) and disqualifies
  collections that can never be a control channel (the Windows audio-stack collection,
  and anything with no writable report path). This scoring has no idea what device it is
  looking at — it will happily rank an unrelated vendor's HID collection above every
  collection belonging to the headset this project actually supports, if that unrelated
  collection happens to declare a wider report.
- `is_supported_device` / `SUPPORTED_VENDOR_ID` / `SUPPORTED_PRODUCT_IDS` positively
  identify the one device (`0x1532` / `0x101B`) this project has been tested against.

**Automatic candidate selection must always be scoped to `is_supported_device`, never to
shape-based ranking alone.** `headsetctl probe`'s default (no `--candidate`) path applies
this scope, and `headsetctl list`'s "Best control candidate" line applies the same scope
to the collections it is showing. This is a safety-relevant rule, not a style
preference: without it, the highest-scoring collection on the machine — which can belong
to any vendor's HID device, not just the headset — gets automatically selected as the
control candidate. This exact failure mode was found and fixed twice: once in `probe`
(Task 10 fix round) and once in `list` (Task 11 fix round, `docs/device-research.md` /
the fix-round ledger), where `list` printed `Best control candidate: index 14` for an
MSI motherboard EC controller instead of the headset. `list` continues to show every
collection it enumerates, unfiltered — the allowlist affects only which one, if any, is
labelled the control candidate. An explicit `--candidate <index>` bypasses the allowlist
deliberately (it is the documented escape hatch for investigating other hardware) and is
flagged with `supported_device: false` plus a stderr warning when it does.

## Unsafe confinement

`headset-protocol` declares `#![forbid(unsafe_code)]`: it has zero operating-system
access and needs none. All `unsafe` code in the entire workspace is confined to
`crates/headset-device/src/windows/ffi.rs`, the single module that calls raw SetupAPI
and `hid.dll` functions. Every other module in `headset-device`, and every module in
`headset-cli` and `headset-tray`, is safe Rust.
