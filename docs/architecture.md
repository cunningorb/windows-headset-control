# Architecture

## Crates

| Crate | Responsibility | May touch the OS |
| ----- | -------------- | ----------------- |
| `headset-protocol` | Pure protocol logic: report models, framing, checksum computation and verification, serialization, and parsing. No dependency on `headset-device`. | No |
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
    /// Read/write access. Only permitted for an identified control collection.
    ReadWrite,
}
```

`HidTransport` deliberately exposes no write method in Phase 1: no caller can perform a
HID write before the write phase is designed and approved.

## Unsafe confinement

`headset-protocol` declares `#![forbid(unsafe_code)]`: it has zero operating-system
access and needs none. All `unsafe` code in the entire workspace is confined to
`crates/headset-device/src/windows/ffi.rs`, the single module that calls raw SetupAPI
and `hid.dll` functions. Every other module in `headset-device`, and every module in
`headset-cli` and `headset-tray`, is safe Rust.
