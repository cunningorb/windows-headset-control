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

**Phase 1 performs zero HID writes anywhere.** `list`, `inspect`, and `probe` — the only
three commands that exist right now — are entirely read-only: they enumerate, read
descriptors, and listen for unsolicited reports, and nothing else. `HidTransport` has no
write method for any command to call. Writing to the device (for example, changing
sidetone) is out of scope until a later phase is explicitly designed and approved; see
`docs/device-research.md` for why that has not happened yet.

## Commands

`headsetctl` is a Windows-only command-line tool. All three commands accept `--json` for
machine-readable output and `--include-sensitive` to reveal device paths (redacted by
default).

### `list`

Enumerates every present HID collection on the machine — not just this project's
supported headset — and, if exactly one collection belonging to a supported device
scores highest, names it the best control candidate.

```
> headsetctl list --vendor-id 0x1532
[13] BlackShark V3 Pro PS HID
     vendor/product : 0x1532 / 0x101b
     usage page/usg : 0xff14 / 0x0001
     reports in/out/feat : 64 / 64 / 0
     candidate      : score 174 (vendor-defined usage page 0xff14; declared report width 64 bytes; bidirectional: input report width 64 bytes)
...
Best control candidate: index 13
```

### `inspect`

Opens one collection, by the absolute index reported by `list`, with zero I/O access
rights and reports its parsed report descriptor.

```
> headsetctl inspect --path-index 13
BlackShark V3 Pro PS HID
  usage page/usg : 0xff14 / 0x0001
  reports in/out/feat : 64 / 64 / 0
  opened for I/O : no (descriptor access only)
  Input report ids : 0x02
  Output report ids : 0x02
```

### `probe`

Opens the selected control candidate for reading only and listens for an unsolicited
input report within a bounded window. Sends nothing to the device; a silent result is a
normal, expected outcome (see `docs/device-research.md`, "Blocker: no known-safe request
exists yet").

```
> headsetctl probe
probe operation : PassiveListen
wrote to device : no
candidate       : [13] usage page 0xff14
result          : silent (no unsolicited report)
```

## Non-affiliation

This is an unofficial community interoperability utility. It is not affiliated with,
authorized by, endorsed by, or sponsored by Razer Inc. or any other manufacturer.
Product names are used only to describe hardware compatibility.

Copyright © 2026. All rights reserved.
