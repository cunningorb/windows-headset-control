# Device Research

## Verified Hardware Baseline

Measured on our own hardware with `HidP_GetCaps` and `HidP_GetValueCaps` (development
machine, 2026-08-01, read-only spike). Treat as ground truth; re-verify if hardware
changes.

| Collection | Usage page | Usage    | Input len | Output len | Feature len | In report ID | Out report ID |
| ---------- | ---------- | -------- | --------- | ---------- | ----------- | ------------ | ------------- |
| `COL01`    | `0x000C`   | `0x0001` | 2         | 0          | 0           | `0x0C`       | —              |
| `COL02`    | `0xFF13`   | `0x0001` | 62        | 62         | 0           | `0x07`       | `0x06`         |
| `COL03`    | `0x000B`   | `0x0005` | 2         | 2          | 0           | `0x05`       | `0x05`         |
| `COL04`    | `0xFF14`   | `0x0001` | 64        | 64         | 0           | `0x02`       | `0x02`         |

VID `0x1532`, PID `0x101B`, version `0x0100`, product string `BlackShark V3 Pro PS HID`,
manufacturer `Razer Inc`, serial present on all four. All four collections are exposed
on USB interface 5.

**Feature report length is 0 on every collection**, so the control transport is interrupt
output plus interrupt input. `COL04` is the presumptive control collection.

This confirms the live-hardware run recorded in `list --vendor-id 0x1532`
(`.superpowers/sdd/2026-08-01-phase1-enumeration-and-probe/task-7-report.md`), which
enumerated the same four collections at absolute indices 10–13 with identical usage
pages, report widths, and report IDs:

| index | usage page | usage | in/out/feature | report ids | ranking outcome |
| --- | --- | --- | --- | --- | --- |
| 10 | `0x000c` | `0x0001` | 2 / 0 / 0 | in `0x0c` | excluded — not vendor-defined |
| 11 | `0xff13` | `0x0001` | 62 / 62 / 0 | in `0x07`, out `0x06` | score 172 |
| 12 | `0x000b` | `0x0005` | 2 / 2 / 0 | in/out `0x05` | excluded — reserved for Windows audio |
| 13 | `0xff14` | `0x0001` | 64 / 64 / 0 | in/out `0x02` | score 174 — best candidate |

All four descriptor reads were taken with Windows' own descriptor APIs
(`HidD_GetAttributes`, `HidP_GetCaps`, `HidP_GetValueCaps`) against our own attached
hardware. Nothing in this table was taken from, or checked against, any third party.

## Control collection selection

**`COL04` (usage page `0xFF14`) is selected as the control collection.**

Evidence:

- `COL04`'s usage page, `0xFF14`, is vendor-defined (`>= 0xFF00`), so `headsetctl`'s
  ranking (`crates/headset-device/src/select.rs::rank_candidates`) admits it as a
  candidate at all.
- `COL04` declares 64-byte input and 64-byte output reports — the widest writable
  report path of any vendor-defined collection on this device.
- `COL04` declares report ID `0x02` on **both** the input and output direction, so a
  request written with ID `0x02` and a response read with ID `0x02` refer to the same
  logical channel.
- Under the ranking formula (100 for a vendor-defined usage page, `+` the declared
  report width in bytes, `+10` if the collection is bidirectional), `COL04` scores
  100 + 64 + 10 = **174**, the highest of any candidate on this device.

`COL02` (usage page `0xFF13`, 62-byte input and output, but with **asymmetric** report
IDs — `0x07` in, `0x06` out) is a qualified alternative. It also scores above zero
(100 + 62 + 10 = **172**) and remains a legitimate vendor channel by the same criteria.
It loses to `COL04` only on the margin above (172 vs. 174: two fewer report bytes), not
because it is disqualified. It remains reachable through `headsetctl probe --candidate
<index>` for anyone who wants to investigate it instead of, or in addition to, `COL04`.

`headsetctl list --vendor-id 0x1532` on real hardware reports `has_unambiguous_winner`
as true for this device: `COL04`'s score (174) strictly exceeds the next-best qualified
candidate's score (172), so the ranking does not need to guess between two tied
channels here. (`has_unambiguous_winner` would return `false`, and `headsetctl` would
refuse to pick automatically, only if two qualified candidates tied on score — that is
not the case on this hardware, but the code path exists for hardware where it is.)

`COL01` (`0x000C`, keyboard/consumer-control usage page) and `COL03` (`0x000B`,
telephony/headset usage page, usage `0x0005`) are excluded outright, not merely
outscored:

- `COL01` is excluded because `0x000C` is a standard (non-vendor) usage page — it is
  the consumer-control page used for media keys, not a proprietary channel.
- `COL03` is excluded because usage page `0x000B` / usage `0x0005` is the telephony
  headset collection that the Windows audio stack itself binds to. This project's
  global constraint forbids ever opening it in `ReadWrite` mode, so it can never be a
  control candidate regardless of its descriptor shape.

## Transport

**Feature report length is `0` on every one of the four collections**, on `COL04`
specifically and on all others. This was read directly from `HIDP_CAPS`'s
`FeatureReportByteLength` field via `HidP_GetCaps`. A length of zero is not "feature
reports are unlikely" — the Windows HID stack reports that no feature report is
declared for this device at all, so `HidD_GetFeature` / `HidD_SetFeature` have no
report to operate on here.

Therefore the only viable control transport for this device is **interrupt output
paired with interrupt input**: a request is written to the collection's output report
(report ID `0x02`, 64 bytes total) and a response, if any, arrives as an unsolicited or
correlated input report (also report ID `0x02`, 64 bytes total) on the same collection.

Report ID `0x02` occupies byte 0 of the 64-byte buffer in both directions (this is how
HID report-ID framing works: the leading byte of a report buffer carries the report ID
when a device declares more than one ID on that report type). That leaves **63 payload
bytes** per report for whatever request or response structure the device actually
uses — a structure that is currently unknown (see Hypothesis register and Unknown
bytes policy below).

This transport conclusion — feature reports unavailable, interrupt in/out only, 1 ID
byte + 63 payload bytes — was determined entirely from our own descriptor reads. No
part of it was taken from, or needed to be checked against, prior art.

## Hypothesis register

Every row below that concerns protocol *semantics* (what a byte or request means) is
`unverified` at this stage of the project. Only the descriptor facts captured in the
Verified Hardware Baseline and the Control collection selection / Transport sections
above carry real confidence, because those were read directly from Windows HID
descriptor APIs against our own hardware. This register is deliberately mostly empty:
we do not have a known-safe request to exercise yet (see Blocker, below), so there is
nothing honest to fill in for request/response semantics.

| Hypothesis | Request byte layout | Expected response length | Checksum hypothesis | Confidence | Evidence |
| --- | --- | --- | --- | --- | --- |
| `COL04` (usage page `0xFF14`, report ID `0x02`) is the control channel | n/a | n/a | n/a | **confirmed** (descriptor-level; not protocol-level) | `HidP_GetCaps`/`HidP_GetValueCaps` on our hardware: 64-byte in/out, report ID `0x02` both directions, highest ranking score (174) |
| Feature reports are unavailable on this device | n/a | n/a | n/a | **confirmed** | `feature_report_len == 0` on all four collections, read via `HidP_GetCaps` |
| A byte-0 report-ID framing byte precedes 63 payload bytes | byte 0 = `0x02`, bytes 1-63 = unknown | n/a | n/a | **confirmed** (framing only, not content) | Declared report length is 64 bytes total including the report-ID byte; declared report ID is `0x02` |
| Any specific request byte layout that elicits a response | unknown | unknown | unknown | **unverified** | none — no known-safe request exists for this hardware (see Blocker) |
| Any specific response byte layout or field meaning | unknown | unknown | unknown | **unverified** | none |
| Any checksum or validation scheme over request/response payloads | unknown | unknown | unknown | **unverified** | none |
| `COL02` (`0xFF13`) carries a usable secondary or legacy control channel | unknown | unknown | unknown | **unverified** | descriptor shape only (62-byte, asymmetric report IDs `0x07`/`0x06`); no request/response exchanged |

No plausible-looking request/response rows have been invented to make this table look
more complete than the evidence supports. When a real request/response pair is
observed (see Blocker, route 1), it should be added here as a new row with its own
evidence citation, not merged into the placeholders above.

## Unknown bytes policy

Any byte, bit, or field whose meaning has not been established by direct observation
on our own hardware is recorded as `unknown`, not assigned a plausible-sounding label.
Concretely:

- The 63 payload bytes in every `COL04` request and response are `unknown` until a
  specific byte's role is confirmed by an observed request/response exchange (see
  Blocker, route 1) or equivalent first-party evidence.
- "Unknown" is not a placeholder to be filled with a guess for convenience. A field
  stays `unknown` — in this document, in code comments, and in any parser — until
  there is direct evidence for what it means on this hardware. Prior-art command
  tables for a different PID (see Blocker, below) are not sufficient evidence on
  their own.
- Any future code that parses or constructs `COL04` payloads must treat unknown byte
  ranges as opaque and must not assign field names, offsets, or interpretations to
  them speculatively.

## Blocker: no known-safe request exists yet

Phase 1B was specified as "exchange one known-safe request and response." **On this
hardware, no such request is known.**

- Our device's PID is `0x101B`. The public Linux prior art
  (`https://github.com/RiskRunner0/blackshark-linux`, see
  `docs/clean-room-notes.md`) targets PID `0x0577` — a different product. Its command
  set cannot be assumed to apply here, and nothing in this repository assumes it does.
- We have no vendor documentation for this device's control protocol.
- Sending a guessed command identifier onto the wire would violate this project's
  standing rule against speculative HID writes (`CONTRIBUTING.md`: "Never send
  speculative or brute-forced HID command identifiers"; global constraint: "This
  entire plan performs zero HID writes").

Therefore Task 10 implements a **passive probe only**: it opens `COL04` (or, via
`--candidate`, an explicitly chosen alternative such as `COL02`) for reading and
listens for unsolicited input reports for a bounded window. It sends nothing to the
device and cannot alter device state. A **silent result — no unsolicited input report
observed — is a legitimate, expected outcome of this probe, not a failure of it.** The
probe's job is to observe, not to provoke a response.

### Routes to a known-safe request

Obtaining an actual known-safe request requires an explicit decision before any HID
write is implemented. None of the three routes below is authorized by this document;
each needs its own explicit go-ahead when it is acted on.

1. **Observe the vendor software.** Capture the HID traffic the manufacturer's own
   configuration software sends to the device (for example with USBPcap plus
   Wireshark) and record the observed request/response pairs here as behavioral facts
   — what was sent, what came back, under what conditions. This is the standard
   interoperability method (observing behavior of software you're allowed to run,
   against hardware you own) and it preserves the clean-room posture described in
   `docs/clean-room-notes.md`, because it records observed device behavior rather than
   copying anyone's source code, comments, or command tables.
2. **Obtain vendor documentation.** Slower, and out of scope for this phase; it
   belongs to the later public-release-readiness phase (see
   `docs/clean-room-notes.md` / a future `docs/manufacturer-contact-draft.md`).
3. **Accept risk on a hypothesis** — i.e., assume the `0x0577` command set (or any
   other guessed byte layout) applies to this `0x101B` device and try it. **Not
   recommended, and not permitted under this project's current rules** without an
   explicit written decision recorded in this document, because it is exactly the
   kind of speculative HID write `CONTRIBUTING.md` prohibits.

Sidetone control, or any other write-capable feature, is **blocked** on one of these
three routes being taken and recorded here. It is not close to working; no request
format is known, and none should be implied to be known or "probably fine to try."

## Prior-art hypotheses: confirmed and refuted

`docs/clean-room-notes.md` records the hypotheses taken from public discussion before
any hardware was measured. Cross-referencing them against the Verified Hardware
Baseline above:

**Confirmed on our hardware:**

- VID `0x1532` — confirmed by our own enumeration.
- 64-byte reports on the control collection — confirmed for `COL04` by our own
  `HidP_GetCaps` reading (64-byte input, 64-byte output).
- Report ID `0x02` on the control collection — confirmed for `COL04` by our own
  `HidP_GetValueCaps` reading (report ID `0x02` declared on both input and output).
- Control functionality on USB interface 5 — consistent; interface 5 carries all four
  vendor-plus-standard collections on this device, including both vendor-defined ones.

**Refuted on our hardware:**

- PID `0x0577` — **refuted**. Our device reports `0x101B`. The public prior art's PID
  hypothesis simply does not hold for this product.

**What the corroboration does, and does not, license us to conclude.**

Report ID `0x02` and the 64-byte report size both matching the public prior art's
hypotheses, *despite* the PID being wrong, is a striking corroboration — two
independent structural details (report ID and report size) lining up with a
different product's publicly discussed behavior is unlikely to be pure coincidence.
It is reasonable to read this as evidence that the BlackShark V3 Pro PS (`0x101B`)
and the product with PID `0x0577` plausibly share a firmware or protocol lineage at
the framing level (report ID, report size).

Interface number is deliberately not counted as a third leg of this corroboration.
Every collection on this device — including `COL01` and `COL03`, both excluded
outright as candidates — sits on interface 5 (see Verified Hardware Baseline,
above), so interface number does not distinguish the control channel from anything
else on this device. It therefore cannot serve as independent evidence that this
device's control framing shares lineage with the `0x0577` product's control
channel; it is merely consistent with the prior-art hypothesis, nothing more.

It does **not** license us to conclude that the *payload* semantics — what any of the
63 unknown bytes inside that report mean, what command values are valid, what a
response looks like, or whether any command from the other product's command set would
even be accepted rather than ignored or misinterpreted — also carry over. Framing
match is not protocol match. Sending a command byte from the `0x0577` command set to
this `0x101B` device on the strength of this corroboration alone would still be
exactly the kind of speculative, unverified HID write this project's rules prohibit
(see Blocker, above). The corroboration is a reason to consider route 1 (observing
vendor software) a promising next step, not a reason to skip straight to a write.
