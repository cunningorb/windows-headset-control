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

**Phase 2 adds a write path, and it is deliberately narrow.** Every command identifier the
project can send was observed on the wire while the manufacturer's own software drove this
hardware; the allowlists in `headset-protocol` contain nothing else, so a speculative or
brute-forced identifier has no path to the device. `docs/device-research.md` records the
evidence for each one, and eleven observed-but-unidentified parameters remain deliberately
unnamed.

`list`, `inspect`, and `probe` remain read-only. `probe` now opens the device with
read-only access rights, so that is enforced by Windows rather than by the absence of a
call.

## Installing

```
> headset-tray.exe --install
```

Copies itself to `%LOCALAPPDATA%\Programs\HeadsetTray`, starts at sign-in, and registers
an Add/Remove Programs entry so it uninstalls like any other application. Per-user
throughout: no administrator rights, no service, no scheduled task, nothing written to
`HKEY_LOCAL_MACHINE`. `--uninstall` reverses it, and so does Windows Settings.

The tray's **Settings** submenu toggles "Run on Windows startup" and "Warn when Synapse is
running". The startup checkbox reads the same registry value Windows reads at sign-in, so
disabling the entry from Task Manager's Startup tab is reflected there rather than
contradicted.

## The tray

`headset-tray.exe` shows battery, microphone mute state, sliders for sidetone (0–15) and
game/chat balance (0–20), and noise control: off, ANC, or ambient, with an ANC level of
1–4. The level track is live only in ANC — ambient has no level — but stays visible in the
other modes, because the headset retains the level and returns to it.

State is never cached authoritatively: a value the device refuses shows as unknown rather
than as a number, and losing the wireless link clears the readings instead of leaving
stale ones on screen. Mute status is the union of the headset's hardware switch and the
Windows capture endpoint, because audio is silenced if either is set — clicking it toggles
only the endpoint, since no software can move a hardware switch.

## Commands

`headsetctl` is a Windows-only command-line tool. Every command accepts `--json` for
machine-readable output and `--include-sensitive` to reveal device paths (redacted by
default).

| Command | Writes to the device |
| --- | --- |
| `list`, `inspect`, `probe`, `watch` | no |
| `get <name>` | sends a read request |
| `set <name> <value>` | yes |
| `noise` | reads; writes only when given `--mode` or `--level` |
| `param get/set <id>` | yes, allowlisted identifiers only |

```
> headsetctl get battery
battery: 49
> headsetctl set sidetone 7
sidetone: 7
> headsetctl noise
noise-cancellation: anc level 4
> headsetctl noise --level 2        # keeps the current mode
noise-cancellation: anc level 2
> headsetctl noise --mode ambient   # ambient has no level; the ANC level is retained
noise-cancellation: ambient (anc level 2)
> headsetctl param get 0x2c        # observed but unidentified; no meaning is claimed
0x2c: 0f
```

`get` reports `unavailable` rather than a number when the device refuses a read — which is
what happens when the headset is powered off. `set` re-reads the parameter afterwards and
reports what the device actually holds, because a write being acknowledged is not evidence
that the value changed.

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
