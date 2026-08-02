# Phase 2 Design: Write Path, Parameter Protocol, and Tray

Status: approved 2026-08-02. Supersedes nothing; extends the Phase 1 design
(`2026-07-31-windows-headset-control-design.md`).

## Why this phase exists

Phase 1 deliberately closed three gates:

- `HidTransport` exposes no `write_report`.
- `ProbeOp` is an allowlist with no request-emitting variant.
- `ControlFrame::known_fields()` returns empty, and `docs/device-research.md`
  records every payload semantic as `unverified` behind a Blocker.

The Blocker named three routes to a known-safe request. **Route 1 was taken**:
the vendor software's HID traffic was captured with USBPcap and Wireshark
against our own hardware, and the observed request/response pairs are recorded
in `docs/device-research.md` as behavioural facts. No third-party source,
comment, or command table was consulted. Every command this phase implements was
observed on the wire; none is guessed.

## Goals

1. Record the observed protocol and clear the Blocker.
2. Add a write path, gated by an allowlist containing only observed identifiers.
3. Expose read and write through `headsetctl` so the device layer is usable and
   verifiable without a GUI.
4. Ship a tray application with battery, sidetone, game/chat balance, and mic
   mute.

## Non-goals

- Identifying the eleven observed-but-unidentified parameters. They are
  reachable through `param get` for future research; none is named or
  interpreted.
- EQ, lighting, or any feature not in the goal list.
- Cross-platform support. Windows only.
- Any command identifier that was not observed on the wire.

## Overriding constraint: footprint

Replacing Synapse is pointless if the replacement is also heavy. Synapse runs a
persistent multi-process engine; this must not.

- **Zero new crate dependencies for the entire phase.** `HidD_SetOutputReport`
  and the Core Audio interfaces are reached through the already-pinned
  `windows 0.58` crate. The tray uses raw `Shell_NotifyIcon` rather than a
  tray-icon crate. New `windows` *features* are permitted; new crates are not.
- No async runtime. No polling loop. The CLI exits; the tray blocks on a Win32
  message loop and a single reader thread, both idle at rest.
- The existing release profile (`strip`, thin LTO, `codegen-units = 1`,
  `panic = "abort"`) is unchanged.

This constraint is why the tray takes on a confined `unsafe` module rather than
a dependency. See "Unsafe confinement" below.

## Protocol

Established by observation; full evidence in `docs/device-research.md`.

### Framing

A control report is 64 bytes including the leading report-ID byte `0x02`.

```
byte  0      report ID (0x02)
byte  1      status        0x00 command / 0x02 response or event
byte  2      transaction   0x60 on every observed frame
bytes 3-5    zero on every observed frame
byte  6      data_size     = 4 + payload length
bytes 7-8    class / command id (0x00 / 0x00 for the parameter family)
bytes 9-10   command       high byte 0x80 = host-originated, 0x00 = device event
                           low byte: bit 7 set = write, clear = read
                           low 7 bits = parameter id
byte  11     role          0x00 request | 0x01 response | 0x02 event
byte  12     payload length
bytes 13..   payload
byte  62     checksum      XOR of bytes 0..61
byte  63     reserved (zero)
```

Checksum verified against all 200+ captured reports in both directions.

Frames whose class/command-id bytes are non-zero (`0x84`, `0x04` observed) are
**not** part of this family and are not decoded. They are surfaced as opaque
bytes.

### Result codes

A write's response carries a result byte. `0x00` is success. `0xFF` was observed
when writing sidetone while the mic is hardware-muted, and is treated as
**refused** — not as a transport failure. Only these two values have been
observed; any other value is surfaced verbatim as an unknown result rather than
assumed to be success.

### Parameters

Named only where evidence exists, per the Unknown bytes policy.

| Id | Name | Access | Range | Notes |
| --- | --- | --- | --- | --- |
| `0x20` | link state | read, event | 2 bytes | `01 00` observed on headset connect; `00 00` while off |
| `0x21` | battery | read, event | 0–100 | percent; confirmed against the vendor UI at 52 and tracked down to 49 |
| `0x19` | sidetone | read, event | 0–15 | |
| `0x5C` | game/chat balance | read, event | 0–20 | centre 10 |
| `0x55` | mic mute | read, event | 0/1 | hardware switch state; no write exists |
| `0x6A` | slider function | read | ≥3 states | selects which parameter the onboard wheel drives |

Writes observed: `0x98` (sidetone enable, always `01`), `0x99` (sidetone level),
`0x9E`, `0xDC` (game/chat balance), `0xEA` (slider function).

Observed-but-unidentified reads, reachable only through `param get`, never
named: `0x12 0x15 0x16 0x17 0x2A 0x2C 0x5D 0x5F 0x60 0x65 0x66`. `0x15`, `0x60`,
and `0x65` take an index operand.

### Sidetone writes carry a preamble

The vendor software writes `0x98 = 01` immediately before every `0x99` level
write, in both directions of a mute transition. `set sidetone` reproduces that
pair exactly, because matching observed behaviour is the only claim we can
defend. Game/chat balance has no preamble and is written as a single command.

### Transport

Writes go out as a HID **output report** via `HidD_SetOutputReport`, which is
what the vendor software was observed doing (`bmRequestType 0x21`,
`bRequest 9` = SET_REPORT, `wValue 0x0202`, `wIndex 5`). Responses and events
arrive as interrupt input reports on the same collection. This corrects Phase 1's
conclusion that interrupt output was the only viable path.

## Architecture

Crate responsibilities are unchanged from `docs/architecture.md`; each gains
code, none gains a new role.

### `headset-protocol` — pure codec

No OS access, `#![forbid(unsafe_code)]` retained.

- `ParamId(u8)` with `READ_ALLOWLIST` and `WRITE_ALLOWLIST` as explicit const
  arrays of observed identifiers.
- `Request::read(param, operand)` / `Request::write(param, value)` producing a
  64-byte buffer, refusing any identifier outside the matching allowlist.
- `Response` / `Event` parsing with checksum verification, role dispatch, and
  payload extraction bounded by `data_size`.
- `ResultCode { Ok, Refused, Unknown(u8) }`.

Every rule is unit-tested against byte sequences taken verbatim from the
captures.

### `headset-device` — OS access and correlation

- `ffi::write_output_report` — `HidD_SetOutputReport`. The only new `unsafe`,
  added to the existing confined module.
- `HidTransport::write_report`, with a default implementation returning
  `DeviceError::WriteNotSupported` so `DescriptorHandle` and the fake cannot
  silently gain write ability.
- `resolve_control_device(backend)` — identity-based resolution: enumerate,
  filter `is_supported_device`, require usage page `0xFF14`, 64-byte input and
  output, report ID `0x02` on both. Zero matches, or more than one, is an error
  with an actionable message. **No candidate index and no device path is ever
  persisted or reused across invocations.**
- `ControlSession` — owns the transport, paces requests at
  `MIN_REQUEST_INTERVAL` (250 ms, already reserved in Phase 1), and correlates:
  write request, then read until a frame arrives whose command matches and whose
  role is `Response`. Frames with role `Event` are handed to a sink and do not
  satisfy a pending request. A bounded deadline applies to the whole exchange,
  not to each read, so a stream of events cannot extend it indefinitely.

The descriptor-shape gate lives in `resolve_control_device`: a collection that
does not match the measured shape is refused before any write, so a differently
shaped supported PID fails loudly rather than being written to on the assumption
that our framing applies.

### `headset-cli` — surface

```
headsetctl get <battery|sidetone|game-chat|mic-mute|slider-function|link>
headsetctl set <sidetone|game-chat> <value>
headsetctl param get <id> [--index <n>]
headsetctl param set <id> <value>
headsetctl watch [--seconds <n>]
```

`get`/`set` accept only the six evidenced names. `param` accepts only allowlisted
identifiers and reports the allowlist on rejection. All commands honour the
existing `--json` and redaction flags and reuse the established renderer split.

`set` re-reads the parameter after writing and reports the value the device
actually holds. This is the concrete expression of the device-is-source-of-truth
decision: a successful write is not evidence of a changed value.

`watch` prints decoded events until its deadline. It exists to validate the
event path headlessly before the tray depends on it.

### `headset-tray` — the application

Single process, single window (message-only), one reader thread.

- **Icon**: a headset glyph rendered from an embedded ICO resource, with the
  battery percentage as the tooltip.
- **Menu** (left or right click): battery percentage, mic mute status, a
  sidetone submenu (0–15), a game/chat submenu (0–20), and Exit.
- **Mute item**: displays the OR of the hardware switch (`0x55`) and the Core
  Audio capture endpoint mute, because audio is silenced if either is set.
  Clicking it toggles **only** the Core Audio endpoint — the hardware switch
  cannot be moved by software, and no set-mute command exists to find.
- **State**: never cached authoritatively. Read once on start, refreshed from
  events, and re-read after every write.
- **Synapse coexistence**: if `RazerAppEngine` is detected, warn once in the
  tooltip that it will contend for settings. Do not refuse to run.
- **Headset off**: link state `0x20` drives a disabled/greyed presentation
  rather than stale values.

Core Audio is reached through `windows` features `Win32_Media_Audio` and
`Win32_System_Com` — features, not new crates.

### Unsafe confinement

`docs/architecture.md` currently states that all workspace `unsafe` lives in
`crates/headset-device/src/windows/ffi.rs` and that `headset-tray` is safe Rust.
Choosing raw `Shell_NotifyIcon` and Core Audio over a tray-icon crate changes
that, and the trade is deliberate: a dependency-free tray is worth a second
confined `unsafe` module.

`headset-tray` gains exactly one such module, `src/win32/mod.rs`, holding the
shell-notify-icon, message-loop, menu, and Core Audio calls. Every other module
in the crate stays safe. `docs/architecture.md` is updated in this phase to say
so, rather than being left contradicting the code.

## Error handling

- Identifier outside the allowlist — rejected in `headset-protocol` before a
  buffer exists. Never reaches the wire.
- Checksum mismatch on a received frame — rejected, reported with the offending
  bytes, not repaired.
- `0xFF` result — reported as refused, with the observed correlation to a muted
  mic offered as a likely cause rather than asserted as the rule.
- No device / several devices / wrong descriptor shape — distinct, actionable
  errors; never a silent fallback to a different collection.
- Handle invalidated mid-session (replug) — distinct error; the tray
  re-resolves, the CLI exits non-zero.
- Response timeout — reports how long it waited and how many unrelated events
  arrived meanwhile.

## Testing

- **Unit** (`headset-protocol`): encoding, checksum, role dispatch, allowlist
  enforcement, result codes, bounds. Fixtures are real captured frames.
- **Integration** (`headset-cli` via `FakeHidBackend`): every command against
  scripted responses, including refusal, timeout, event interleaving, and
  malformed frames. Snapshot tests for both renderers, matching the existing
  `insta` pattern.
- **Hardware** (`#[ignore]`, `HEADSET_HARDWARE_TESTS=1`, excluded from CI):
  round-trip reads of every evidenced parameter, and a sidetone write that
  restores the value it read first, so the test is state-neutral.
- The `CONTRIBUTING.md` gate — `fmt --check`, `clippy -D warnings`,
  `test --workspace`, `build --release` — must pass before the phase is done.

## Decisions made without further input

Recorded because the user delegated execution:

1. Raw Win32 tray rather than a tray-icon crate, to hold the zero-new-dependency
   line; costs one confined `unsafe` module in `headset-tray`.
2. `set sidetone` reproduces the observed `0x98`/`0x99` pair rather than writing
   the level alone.
3. Mute status is the OR of two independent sources; the click toggles only the
   one software can move.
4. `write_report` is a defaulted trait method that errors, so read-only
   transports cannot silently acquire write ability.
5. The eleven unidentified parameters are reachable but unnamed, and no
   interpretation of their payloads is recorded anywhere.
