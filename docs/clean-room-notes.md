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
| Control on USB interface 5 | public discussion | Consistent, but not distinguishing: interface 5 carries all four collections on this device (both vendor-defined ones and both standard ones), so interface number alone cannot identify the control channel. See `docs/device-research.md`, "Interface number is deliberately not counted as a third leg of this corroboration." |
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
`docs/superpowers/specs/2026-07-31-windows-headset-control-design.md` and from
descriptor data measured on our own hardware.
