# Device Research

## Verified Hardware Baseline

Measured on our own hardware with `HidP_GetCaps` and `HidP_GetValueCaps` (development
machine, 2026-08-01, read-only spike). Treat as ground truth; re-verify if hardware
changes.

| Collection | Usage page | Usage | Input len | Output len | Feature len | In report ID | Out report ID |
| ---------- | ---------- | ----- | --------- | ---------- | ----------- | ------------ | ------------- |
| `COL01`    | `0x000C`   | `0x01`| 2         | 0          | 0           | `0x0C`       | —             |
| `COL02`    | `0xFF13`   | `0x01`| 62        | 62         | 0           | `0x07`       | `0x06`        |
| `COL03`    | `0x000B`   | `0x05`| 2         | 2          | 0           | `0x05`       | `0x05`        |
| `COL04`    | `0xFF14`   | `0x01`| 64        | 64         | 0           | `0x02`       | `0x02`        |

VID `0x1532`, PID `0x101B`, version `0x0100`, product string `BlackShark V3 Pro PS HID`,
manufacturer `Razer Inc`, serial present on all four.

**Feature report length is 0 on every collection**, so the control transport is interrupt
output plus interrupt input. `COL04` is the presumptive control collection.

Task 8 populates the rest of this document: candidate ranking rationale, the transport
behavior actually observed for the control collection, and the read-only probe result.
