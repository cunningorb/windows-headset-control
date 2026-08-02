# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities through GitHub's
[private vulnerability reporting](https://github.com/cunningorb/windows-headset-control/security/advisories/new)
rather than a public issue, so a fix can be prepared before details are public.

Expect an acknowledgement within a week. This is a hobby project maintained by one person,
so please size your expectations accordingly — there is no on-call rotation behind it.

**When reporting, redact serial numbers and device paths.** Both identify your machine, and
`headsetctl` redacts them by default for that reason; `--include-sensitive` reveals them
and prints a warning header when it does.

## Supported versions

The `main` branch is the only supported version. There are no maintained release branches.

## Design constraints

These are properties the code is built to hold, not aspirations:

- All USB/HID input is treated as untrusted. Response lengths are validated before parsing,
  and a frame whose declared length disagrees with its implied length is refused rather
  than reconciled.
- No unbounded reads or allocations.
- No administrator privileges. No driver installation. No service installation.
- No firmware read, write, or modification.
- **HID writes are gated behind an allowlist of identifiers observed on the wire.**
  `headset-protocol` cannot encode a command outside it, so a speculative or brute-forced
  identifier has no path to the device from anywhere in the workspace. Broad command
  scanning is prohibited by `CONTRIBUTING.md` and prevented by the allowlist.
- Serial numbers and device paths are redacted from output by default.
- No telemetry. No runtime network access.
- Signing material is never committed. See `docs/release-signing.md`.

## Scope

This project talks to a USB HID device as a normal user. The realistic risks are to the
machine running it and to the attached headset, not to a network service — there is no
network surface at all. `docs/threat-model.md` has the full analysis.
