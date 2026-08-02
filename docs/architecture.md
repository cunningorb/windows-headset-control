# Architecture

## Crates

| Crate | Responsibility | May touch the OS |
| ----- | -------------- | ----------------- |
| `headset-protocol` | Pure protocol logic: report models, framing, serialization, and parsing, plus the parameter-access codec and its byte-62 XOR checksum (established by observation — see docs/device-research.md). Holds the observed-identifier allowlists, so an unobserved command cannot be encoded anywhere in the workspace. No dependency on `headset-device`. | No |
| `headset-device` | All Windows HID access behind a mockable `HidBackend` trait. Real windows-rs implementation (`WindowsHidBackend`) and a fixture-driven fake (`FakeHidBackend`) for tests. Also owns `ControlSession`: identity-based device resolution and request/response correlation. Depends on `headset-protocol`; the reverse dependency stays forbidden. | Yes |
| `headset-cli` | `headsetctl.exe`. Command dispatch, filtering, ranking, redaction, and rendering, all built above the `HidBackend` trait so they are testable without hardware. | No (delegates to `headset-device`) |
| `headset-tray` | `headset-tray.exe`. The Windows tray application: a safe state model, a device thread owning the one `ControlSession`, and a confined `unsafe` Win32 module for the notification icon, menus, and Core Audio microphone mute. | Yes — see Unsafe confinement |

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
    /// FILE_GENERIC_READ only. Can receive input reports; cannot write.
    Read,
    /// FILE_GENERIC_READ | FILE_GENERIC_WRITE. Granted only to the control
    /// session, and only after the descriptor-shape gate has passed.
    ReadWrite,
}
```

Phase 2 added `write_report` and `output_report_len` to `HidTransport`. Both are
**defaulted**: `write_report` returns `DeviceError::WriteNotSupported` unless a transport
overrides it. A read-only handle therefore cannot acquire write ability by inheriting an
implementation, which makes the guarantee a property of the type rather than of caller
discipline. `WindowsTransport` overrides it only when constructed from a handle opened
with `OpenMode::ReadWrite`.

`probe` uses `OpenMode::Read`, so its documented read-only contract is now enforced by
the access rights Windows granted, not merely by the absence of a call.

## Control device resolution

`ControlSession` never resolves the device by candidate index or by a stored device path.
Both are machine-local: the control collection sits at a different absolute index on a
different PC, and the path embeds an instance id that changes when the dongle is
replugged. Resolution runs on every session: enumerate, filter to `is_supported_device`,
require usage page `0xFF14` with 64-byte input and output reports. Zero matches and more
than one match are both errors rather than a guess.

The descriptor-shape check is a safety gate, not a sanity check. The protocol was derived
from one PID's measured framing; a second supported PID whose control collection differs
is refused rather than written to on the assumption that our framing applies.

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
access and needs none.

`unsafe` is confined to exactly **two** modules in the workspace:

- `crates/headset-device/src/windows/ffi.rs` — raw SetupAPI and `hid.dll`, including
  `HidD_SetOutputReport` for the write path.
- `crates/headset-tray/src/win32/mod.rs` — the notification icon, popup menus, message
  loop, process check, and Core Audio.

The second one is a deliberate Phase 2 trade. Phase 1 stated that `headset-tray` would be
entirely safe Rust, which assumed it would take a tray-icon crate. Phase 2's footprint
rule is zero new crate dependencies — replacing a heavyweight vendor application with
another heavyweight application defeats the purpose — so the tray calls `Shell_NotifyIcon`
and Core Audio directly instead. A second confined `unsafe` module was judged the better
side of that trade. Every other module in every crate is safe Rust, and `headset-cli`
contains none at all.

Microphone mute lives in the tray's Win32 module rather than in `headset-device` because
it is a USB Audio Class control, not a vendor HID command. The vendor protocol can only
*report* the headset's hardware mute switch; it cannot set mute, and no such command
exists to find. See `docs/device-research.md`.
