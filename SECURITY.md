# Security Policy

## Reporting

This project is private and experimental. Report suspected vulnerabilities privately
to the repository owner. Do not open public issues.

## Design constraints

- All USB/HID input is treated as untrusted. Response lengths are validated before parsing.
- No unbounded reads or allocations.
- No administrator privileges. No driver installation. No service installation.
- No firmware read, write, or modification.
- Phase 1 performs no HID writes at all: `HidTransport` exposes no write method anywhere
  in the codebase. When a write phase is designed and approved, HID writes will be gated
  behind an explicit allowlist. Broad command scanning is prohibited.
- Serial numbers and device paths are redacted from output by default.
- No telemetry. No runtime network access.
- Signing material is never committed. See `docs/release-signing.md`.
