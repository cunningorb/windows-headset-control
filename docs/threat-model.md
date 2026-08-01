# Threat Model

| Asset | Threat | Mitigation |
| ----- | ------ | ---------- |
| Untrusted HID input | Malformed or hostile report | Length and report-ID validated before parsing, fixed-size buffers, no unbounded allocation |
| Machine-identifying data | Path or serial leaking into a bug report | Redaction on by default; `--include-sensitive` prints a warning banner |
| Audio-stack contention | Opening the telephony collection breaks playback | `OpenMode::Descriptors` requests zero access rights, and `COL03` is refused for `ReadWrite` |
| Privilege | Elevation of privilege, driver/service installation | `asInvoker` only, no driver, no service |
| Supply chain | Compromised signing material or dependency | No `.pfx`/key ever committed; dependency licenses tracked in `THIRD_PARTY_NOTICES.md` |
