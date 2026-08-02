# Threat Model

| Asset | Threat | Mitigation |
| ----- | ------ | ---------- |
| Untrusted HID input | Malformed or hostile report | Length and report-ID validated before parsing, fixed-size buffers, no unbounded allocation |
| Machine-identifying data | Path or serial leaking into a bug report | Redaction on by default; `--include-sensitive` prints a warning banner |
| Audio-stack contention | Opening the telephony collection breaks playback | `OpenMode::Descriptors` requests zero access rights, and `COL03` is refused for `ReadWrite` |
| Privilege | Elevation of privilege, driver/service installation | `asInvoker` only, no driver, no service |
| Auto-start persistence | The tray installs itself into a logon-persistence mechanism, which is behaviour malware also uses and which a user must be able to see and revoke | Per-user `HKCU\...\CurrentVersion\Run` only — never `HKLM`, never a service, never a scheduled task, and never elevated. The entry is named `HeadsetTray`, so it appears in Task Manager's Startup tab and can be disabled there. The tray reads that same value to render its checkbox, so the UI cannot claim startup is on after Windows has disabled it. `--uninstall` removes the entry, and so does the Add/Remove Programs entry it registers |
| Install location | Writing an executable somewhere a user does not expect, or somewhere shared | `%LOCALAPPDATA%\Programs\HeadsetTray` only. Never `Program Files`, which would need elevation and would put a binary where other users execute it. A unit test asserts the path is neither |
| Control-channel targeting | Shape-only ranking selects an unrelated vendor's HID collection | `is_supported_device` scoping on automatic selection; explicit `--candidate` warns on stderr and reports `supported_device: false` |
| Supply chain | Compromised signing material or dependency | No `.pfx`/key ever committed; dependency licenses tracked in `THIRD_PARTY_NOTICES.md` |
