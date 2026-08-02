# Open-Source Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make this repository publishable as open source — licensed, accurate, contributor-ready, and free of statements that only made sense while it was private.

**Architecture:** Almost entirely documentation and repository metadata. The one piece of code is a test that pins an invariant the README asserts in prose, so the two cannot drift apart again. Tasks are ordered so the blocker lands first and each subsequent task leaves the repo in a coherent state.

**Tech Stack:** Markdown, GitHub repository configuration (`.github/`), Cargo manifest metadata, one Rust test in `headset-protocol`.

## Global Constraints

- **Licence is `MIT OR Apache-2.0`**, dual, user's choice. Every file added or edited in this plan uses that exact SPDX expression. If this decision changes, Task 1 is the only place it is encoded.
- **Do not change repository visibility.** `CONTRIBUTING.md` forbids it without explicit instruction, and that rule survives this plan. Making the repo public is the owner's action, taken after this plan is merged — not a step in it.
- **No claim in a document may outrun the evidence.** This repo's existing standard: `docs/device-research.md` records what was observed and refuses to name what was not. Documentation written here holds to the same line — including about the project's own maturity.
- **Do not weaken the existing hard rules.** The six rules in `CONTRIBUTING.md` are load-bearing (HID write allowlist, no speculative identifiers, redaction, no committed signing material). They are reorganised and explained, never dropped.
- **`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace --release` must pass before every commit.**

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `LICENSE-MIT` | **New.** Verbatim MIT text. | 1 |
| `LICENSE-APACHE` | **New.** Verbatim Apache-2.0 text. | 1 |
| `Cargo.toml` | Gains `license`, `description`, `repository`; loses the "proprietary" comment; MSRV reconciled. | 1, 6 |
| `README.md` | Rewritten for an outside reader: what it is, what hardware, how to get it, licence, trademarks, risk. | 2 |
| `crates/headset-protocol/src/param.rs` | A test pinning the unnamed-parameter count the README quotes. | 2 |
| `SECURITY.md` | A reporting channel that exists, and a current description of the write posture. | 3 |
| `CONTRIBUTING.md` | Rewritten for contributors; the GNU-toolchain trap documented where people will look. | 4 |
| `CODE_OF_CONDUCT.md` | **New.** Contributor Covenant 2.1. | 5 |
| `.github/ISSUE_TEMPLATE/*.yml`, `.github/PULL_REQUEST_TEMPLATE.md` | **New.** Enforce redaction and the allowlist rule at the point of contribution. | 5 |
| `docs/superpowers/` → `docs/history/` | Renamed, with an explainer; dangling reference fixed. | 6 |
| `crates/headset-tray/src/ui/theme.rs`, phase-3 spec | Dangling mockup path reworded. | 6 |
| `.github/workflows/ci.yml` | Third-party notices freshness check. | 6 |
| `CHANGELOG.md` | **New.** Keep a Changelog format. | 7 |

---

### Task 1: Licence the project

**Problem being fixed:** there is no licence. No `LICENSE` file, `licenseInfo: null` on GitHub, `Cargo.toml` deliberately omits the key with a comment calling the workspace proprietary, and `README.md` ends "All rights reserved." Published in that state the code is visible but not open source: default copyright reserves every right, so nobody may use, modify, or redistribute it, and no contributor has terms to contribute under.

**Files:**
- Create: `LICENSE-MIT`, `LICENSE-APACHE`
- Modify: `Cargo.toml` (the `[workspace.package]` block and its licence comment)
- Modify: `README.md` (the trailing copyright line — the rest of the README is Task 2)

**Interfaces:**
- Produces: the SPDX expression `MIT OR Apache-2.0`, which Tasks 2, 4, and 5 reference in prose and in the PR template.

- [ ] **Step 1: Write `LICENSE-MIT`**

Verbatim MIT, with the copyright line filled in. Replace `<OWNER>` with the name the project should be attributed to (a personal name or a handle — both are normal; use the same string in `LICENSE-APACHE`):

```
MIT License

Copyright (c) 2026 <OWNER>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 2: Write `LICENSE-APACHE`**

Do **not** retype this one. Fetch the canonical text — retyping an 11 KB legal document is how subtle divergences get introduced:

```powershell
Invoke-WebRequest -Uri https://www.apache.org/licenses/LICENSE-2.0.txt -OutFile LICENSE-APACHE
```

Then verify it starts with `Apache License` and `Version 2.0, January 2004`, and that it is roughly 11 KB. The appendix ("APPENDIX: How to apply the Apache License to your work") stays as-is; the boilerplate inside it is instructional text, not something to fill in for a whole-project licence.

- [ ] **Step 3: Record the licence in the workspace manifest**

In `Cargo.toml`, replace the entire `[workspace.package]` licence comment block — the one beginning `# No license key: this is a private, proprietary, unpublished workspace` — with the key itself, plus the metadata a published crate needs:

```toml
[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.85"
publish = false
license = "MIT OR Apache-2.0"
description = "Experimental native Windows HID controller for supported wireless headset settings."
repository = "https://github.com/cunningorb/windows-headset-control"
```

`publish = false` stays: these crates are not intended for crates.io yet, and a licence is required for publishing, not for being open source. Changing that is a separate decision.

- [ ] **Step 4: Replace the README's rights reservation**

In `README.md`, delete the line `Copyright © 2026. All rights reserved.` and put a licence section in its place. The contribution paragraph is the standard Rust wording, and it is what makes a CLA unnecessary:

```markdown
## Licence

Licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT licence ([`LICENSE-MIT`](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 licence, shall be dual licensed
as above, without any additional terms or conditions.
```

- [ ] **Step 5: Verify the licence is machine-readable**

```powershell
cargo metadata --format-version 1 --no-deps | ConvertFrom-Json | Select-Object -ExpandProperty packages | Select-Object name, license
```
Expected: all four crates report `MIT OR Apache-2.0`. A crate reporting `null` means it does not inherit `license.workspace`; check its own `Cargo.toml` for a `[package]` block that overrides.

- [ ] **Step 6: Run the gate and commit**

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

```bash
git add LICENSE-MIT LICENSE-APACHE Cargo.toml README.md
git commit -m "docs: dual-licence the project under MIT OR Apache-2.0"
```

---

### Task 2: Rewrite the README for someone who has never seen this project

**Problem being fixed:** three separate things.

1. **It says the project is private** (`README.md:5`), which will be false.
2. **Two claims are now factually wrong.** `README.md:21` says "eleven observed-but-unidentified parameters remain deliberately unnamed" — it is ten since `0x12` was identified. `README.md:127` cites a `docs/device-research.md` heading, "Blocker: no known-safe request exists yet", that now reads "Blocker: RESOLVED 2026-08-02 via route 1" — the citation points at a section saying the opposite of the sentence citing it.
3. **It is written for someone who already knows the project.** It narrates internal phase numbers ("Phase 2 adds a write path"), and never plainly states which hardware this works with — the model appears only incidentally inside CLI output examples.

**Files:**
- Modify: `README.md`
- Modify: `crates/headset-protocol/src/param.rs` (the drift-catching test)

**Interfaces:**
- Consumes: the licence section from Task 1, which stays where Task 1 put it.
- Produces: nothing other tasks depend on.

- [ ] **Step 1: Write the failing test that pins the count**

The README quotes a number that only prose kept in sync, and prose lost. Make it an invariant instead. Add to the `tests` module in `crates/headset-protocol/src/param.rs`:

```rust
    #[test]
    fn the_number_of_observed_but_unnamed_parameters_is_pinned() {
        // The README states this count in prose, and `docs/device-research.md`
        // lists them by identifier. It was wrong for exactly as long as it took
        // to identify 0x12 and not notice. Identifying another parameter should
        // fail here, which is the prompt to update both documents.
        let named = READ_ALLOWLIST
            .iter()
            .filter(|id| Param::ALL.iter().any(|p| p.id() == **id))
            .count();
        let unnamed = READ_ALLOWLIST.len() - named;
        assert_eq!(
            unnamed, 10,
            "the count of unnamed readable parameters changed; update README.md \
             and the 'Reads observed whose meaning is unknown' list in \
             docs/device-research.md to match"
        );
    }
```

- [ ] **Step 2: Run it and confirm it passes for the right reason**

Run: `cargo test -p headset-protocol the_number_of_observed`
Expected: PASS with `unnamed == 10`.

This one is written after the fact rather than before it, because the behaviour already exists and the test is documenting an invariant rather than driving new code. Confirm it is not vacuous by temporarily changing `10` to `11`, re-running, and checking it fails with the guidance message — then change it back.

- [ ] **Step 3: Fix the two wrong claims**

In `README.md`, change `eleven observed-but-unidentified parameters` to `ten observed-but-unidentified parameters`.

In the `### probe` section, the parenthetical citing the old Blocker heading is stale in substance, not just in wording: a silent probe is still a normal outcome, but not for the reason given. Replace:

```
normal, expected outcome (see `docs/device-research.md`, "Blocker: no known-safe request
exists yet").
```

with:

```
normal, expected outcome: the device pushes reports when something changes, and nothing
may have changed during the window.
```

- [ ] **Step 4: Replace the status line and the phase framing**

Replace `README.md:5` with a status that is true and useful to a stranger:

```markdown
**Status:** experimental. It works on the author's hardware and is tested against a
fixture-driven fake device, but it speaks a protocol reconstructed by observation rather
than from documentation. Expect rough edges, and read the risk note below before running
it against a headset you care about.
```

Then rewrite the "What this is" section so it does not lead with a phase number. Keep every factual claim; only the framing changes:

```markdown
## What this is

A user-mode Windows utility that reads and controls settings of a Razer BlackShark V3 Pro
wireless headset over its vendor HID interface — battery, sidetone, game/chat balance,
microphone mute state, and noise control — from a tray application and a command line.

- Runs as a normal user. No administrator rights.
- Installs no driver and no service.
- Reads and writes no firmware.
- Makes no network requests and collects no telemetry.

**The set of commands it can send is deliberately narrow.** Every command identifier the
project can put on the wire was observed there while the manufacturer's own software drove
this hardware; the allowlists in `headset-protocol` contain nothing else, so a speculative
or brute-forced identifier has no path to the device. `docs/device-research.md` records the
evidence for each one, and ten observed-but-unidentified parameters remain deliberately
unnamed rather than guessed at.

`list`, `inspect`, and `probe` are read-only. `probe` opens the device with read-only
access rights, so that is enforced by Windows rather than by the absence of a call.
```

- [ ] **Step 5: Add a supported-hardware section, immediately after "What this is"**

The single most useful thing for a stranger, and currently absent:

```markdown
## Supported hardware

One device:

| | |
| --- | --- |
| Product | Razer BlackShark V3 Pro (wireless, via its USB dongle) |
| USB vendor id | `0x1532` |
| USB product id | `0x101B` |

**Other products are not supported and are not assumed compatible**, including other
BlackShark models. The protocol here was reconstructed by watching this specific device;
a different product id is a different device until someone captures it and records the
evidence. See `docs/device-research.md`.

To check what you have:

```
> headsetctl list --vendor-id 0x1532
```

If that prints nothing, this project cannot talk to your headset.
```

- [ ] **Step 6: Add the trademark and risk notes**

Extend the existing "Non-affiliation" section — keep its current wording and add these two paragraphs after it:

```markdown
Razer, BlackShark, and Synapse are trademarks of Razer Inc. They are used here only to
identify the hardware this utility interoperates with. This project's licence grants no
rights in any trademark.

## Risk

This utility sends vendor-specific commands to hardware, using a protocol reconstructed by
observation rather than from a specification. It performs no firmware access of any kind,
and every command it can send was observed being sent by the manufacturer's own software —
but no warranty is offered, by this project's licence or otherwise. Running it may
void your hardware warranty. Use it at your own risk.
```

- [ ] **Step 7: Run the gate and commit**

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Also confirm no other document still cites the retired Blocker heading:

```bash
git grep -n "no known-safe request exists yet" -- ':!docs/device-research.md'
```
Expected: no output. (`device-research.md` keeps the phrase inside its preserved original-Blocker section, which is deliberate.)

```bash
git add README.md crates/headset-protocol/src/param.rs
git commit -m "docs(readme): write for an outside reader, and pin the count that drifted"
```

---

### Task 3: A security policy a stranger can act on

**Problem being fixed:** `SECURITY.md:5` says "This project is private and experimental. Report suspected vulnerabilities privately to the repository owner. Do not open public issues." — with no address, no form, and no alternative. Issues are enabled on the repo, so a reporter is told not to use the only channel they can see. `SECURITY.md:14` also still describes Phase 1's "no HID writes anywhere in the codebase" posture, which two phases of work have made false.

**Files:**
- Modify: `SECURITY.md`

- [ ] **Step 1: Enable GitHub private vulnerability reporting**

This gives a real channel without publishing an email address:

```powershell
gh api --method PATCH repos/cunningorb/windows-headset-control -f security_and_analysis[secret_scanning][status]=enabled 2>$null
gh api --method PUT repos/cunningorb/windows-headset-control/private-vulnerability-reporting
```

Verify:
```powershell
gh api repos/cunningorb/windows-headset-control | ConvertFrom-Json | Select-Object -ExpandProperty security_and_analysis
```

If the API rejects it because the repository is still private, note that and enable it in Settings → Security after publication. Record which happened in the commit message rather than assuming it worked.

- [ ] **Step 2: Rewrite the file**

```markdown
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
```

- [ ] **Step 3: Commit**

```bash
git add SECURITY.md
git commit -m "docs(security): give reporters a channel that exists, and describe the current write posture"
```

---

### Task 4: A contributing guide someone else can follow

**Problem being fixed:** `CONTRIBUTING.md` is 19 lines — a personal pre-push checklist plus six hard rules. The rules are good and stay. What is missing is everything a newcomer needs, and one omission is a genuine trap: **the workspace targets `x86_64-pc-windows-gnu`**, and the reason (`windows` 0.59+ needs `dlltool.exe`, which this toolchain lacks; `cargo update` past the pins re-breaks it) is recorded only in a comment inside `Cargo.toml`, where nobody debugging a build failure would look.

**Files:**
- Modify: `CONTRIBUTING.md`

- [ ] **Step 1: Rewrite the file, keeping all six hard rules intact**

```markdown
# Contributing

Thanks for looking. This is a small, opinionated project: it talks to one headset over a
protocol reconstructed by observation, and most of its rules exist to keep that honest.

## Before you start

**Read `docs/device-research.md`.** It is the record of what was observed on the wire and
what deliberately remains unnamed. Nearly every rule below follows from it.

If you are proposing support for a **different device**, open an issue first. It is not a
small change: it needs its own capture evidence, and the answer to "does the protocol carry
over?" is not assumed to be yes.

## Building

Windows only. The workspace targets **`x86_64-pc-windows-gnu`**, not the more common MSVC
target, and this is not incidental:

- `rust-toolchain.toml` pins the toolchain and the target, so `rustup` selects both
  automatically when you build inside the repo.
- The `windows` crate is pinned to 0.58 because 0.59+ uses `raw-dylib` linkage, which needs
  `dlltool.exe`; the GNU toolchain used here does not ship one. `Cargo.lock` pins
  `windows-sys`, `windows-targets`, and `console` for the same reason, and
  `.cargo/config.toml` sets `getrandom_backend = "windows_legacy"` to keep the rest of the
  tree off it too.
- **A routine `cargo update` will break the build** by bumping one of those past its pin.
  If you need to update a dependency, check that the whole tree stays off `raw-dylib`
  before proposing it.

```powershell
cargo build --workspace
cargo test --workspace
```

Most tests need no hardware: the device layer has a fixture-driven fake backend, and the
panel layout, protocol codec, and placement geometry are pure and fully unit-tested.
Hardware tests are gated behind the `HEADSET_HARDWARE_TESTS` environment variable and are
skipped by default.

To see the tray panel without a headset:

```powershell
cargo run --release -p headset-tray -- --render-panel .\out
```

That renders every panel state to PNG through the same Direct2D path the live window uses.

## Before every push

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

CI runs exactly this. Clippy warnings are errors here.

## Hard rules

These are not style preferences. A change that breaks one of them will not be merged
regardless of how good the rest of it is.

- **Never copy source, comments, assets, or structure from an unlicensed third-party
  project.** See `docs/clean-room-notes.md` for what has been consulted and on what terms.
  If you consult something new, record it there in the same pull request.
- **Never add a HID write without adding it to the allowlist and documenting the
  rationale.** "Rationale" means capture evidence in `docs/device-research.md`: what was
  sent, what came back, under what conditions. Not inference, and not a pattern that holds
  for other identifiers.
- **Never send speculative or brute-forced HID command identifiers.** Not in a branch, not
  behind a flag, not "just to see". The allowlist exists to make this impossible by
  construction; do not route around it.
- **Never commit `.pfx`, `.p12`, private keys, passwords, or signing tokens.**
- **Redact serial numbers and device paths** in issues, logs, pull requests, and commit
  messages.
- **Never publish or change repository visibility without explicit instruction.**

## Style

- Follow the surrounding code. `rustfmt` decides formatting; taste decides the rest.
- **Comments explain why, not what.** The existing code is dense with the reasoning behind
  non-obvious decisions — why the panel holds its bottom edge, why a re-entrancy guard is
  not optional, why a value is refused rather than clamped. Match that.
- **Do not name what has not been observed.** A byte whose meaning is unestablished stays
  unnamed, in code and in documentation. `docs/device-research.md` explains the policy.
- Tests are written first where practical, and named after the behaviour they pin rather
  than the function they call.

## Pull requests

- One concern per pull request.
- Say what you verified and how — including what you could not verify. "Tested on my
  hardware" and "not tested against hardware" are both useful; silence is not.
- Update `docs/` in the same pull request when behaviour or evidence changes.
```

- [ ] **Step 2: Verify every command in the guide actually works**

Run each, from a clean checkout if possible:

```powershell
cargo build --workspace
cargo test --workspace
cargo run --release -p headset-tray -- --render-panel .\out
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

Confirm `--render-panel` produced PNGs. A contributing guide whose commands do not run is
worse than none.

- [ ] **Step 3: Confirm the hardware-test claim is true**

```bash
git grep -n "HEADSET_HARDWARE_TESTS"
```
Expected: the variable is read by the hardware-gated tests and set to empty in CI. If the
gating works differently, fix the guide to match the code rather than the reverse.

- [ ] **Step 4: Commit**

```bash
git add CONTRIBUTING.md
git commit -m "docs(contributing): write for contributors, and document the GNU-target trap"
```

---

### Task 5: Code of conduct and contribution templates

**Problem being fixed:** `.github/` contains only `workflows/ci.yml`. There is no code of conduct, no issue template, and no pull-request template. Beyond the community-profile box-ticking, this project has two rules strangers are especially likely to break — pasting an unredacted device path into an issue, and proposing a HID identifier nobody captured — and templates are where those get caught.

**Files:**
- Create: `CODE_OF_CONDUCT.md`
- Create: `.github/ISSUE_TEMPLATE/bug_report.yml`
- Create: `.github/ISSUE_TEMPLATE/config.yml`
- Create: `.github/PULL_REQUEST_TEMPLATE.md`

- [ ] **Step 1: Add the Contributor Covenant**

Fetch version 2.1 rather than paraphrasing it:

```powershell
Invoke-WebRequest -Uri https://www.contributor-covenant.org/version/2/1/code_of_conduct.txt -OutFile CODE_OF_CONDUCT.md
```

Then replace the `[INSERT CONTACT METHOD]` placeholder in the Enforcement section with a real contact — the same GitHub security-advisory link used in `SECURITY.md` is acceptable if you would rather not publish an email address. Verify no placeholder survives:

```bash
grep -n "INSERT CONTACT METHOD" CODE_OF_CONDUCT.md
```
Expected: no output.

- [ ] **Step 2: Add the bug report template**

`.github/ISSUE_TEMPLATE/bug_report.yml`:

```yaml
name: Bug report
description: Something behaves differently from what the documentation says
labels: [bug]
body:
  - type: markdown
    attributes:
      value: |
        **Before you paste any output:** `headsetctl` redacts serial numbers and device
        paths by default. If you ran it with `--include-sensitive`, re-run it without that
        flag and paste that output instead — both identify your machine.

  - type: input
    id: device
    attributes:
      label: Product id
      description: "From `headsetctl list --vendor-id 0x1532`. This project supports 0x101B only."
      placeholder: "0x101b"
    validations:
      required: true

  - type: textarea
    id: what
    attributes:
      label: What happened, and what you expected instead
    validations:
      required: true

  - type: textarea
    id: repro
    attributes:
      label: Steps to reproduce
      description: The exact commands, or the exact sequence of clicks in the tray.
    validations:
      required: true

  - type: textarea
    id: output
    attributes:
      label: Output
      description: "Redacted. `--json` output is welcome. Tray logs: set HEADSET_TRAY_LOG=1."
      render: text

  - type: input
    id: windows
    attributes:
      label: Windows version
      placeholder: "Windows 11 24H2"

  - type: checkboxes
    id: redacted
    attributes:
      label: Confirmation
      options:
        - label: I have not included serial numbers or device paths.
          required: true
```

- [ ] **Step 3: Point people away from the wrong channels**

`.github/ISSUE_TEMPLATE/config.yml`:

```yaml
blank_issues_enabled: true
contact_links:
  - name: Security vulnerability
    url: https://github.com/cunningorb/windows-headset-control/security/advisories/new
    about: Report privately instead of opening an issue, so a fix can land before details are public.
  - name: Support for a different headset
    url: https://github.com/cunningorb/windows-headset-control/blob/main/docs/device-research.md
    about: Read how this device's protocol was established first. Other products need their own capture evidence and are not assumed compatible.
```

- [ ] **Step 4: Add the pull-request template**

`.github/PULL_REQUEST_TEMPLATE.md`:

```markdown
## What this changes

<!-- One concern per pull request. What, and why. -->

## How it was verified

<!-- Commands run, tests added, and what you checked by hand. Say plainly what you could
     NOT verify - "not tested against hardware" is useful; silence is not. -->

## Checklist

- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace`, and `cargo build --workspace --release` all pass.
- [ ] No serial numbers or device paths appear in the diff, the description, or the commit
      messages.
- [ ] No source, comments, assets, or structure copied from a third-party project. If I
      consulted anything new, I recorded it in `docs/clean-room-notes.md` in this pull
      request.

### If this touches HID commands

- [ ] Every identifier added to an allowlist was **observed on the wire**, and the capture
      evidence is recorded in `docs/device-research.md` in this pull request.
- [ ] I have not added an identifier by inference, by pattern, or by analogy to another
      device — including patterns that hold for existing identifiers.

By submitting this pull request I agree that my contribution is dual licensed under
MIT OR Apache-2.0, as described in the README.
```

- [ ] **Step 5: Verify GitHub parses the templates**

YAML issue forms fail silently and fall back to a blank issue if malformed. Check the syntax
before trusting it:

```powershell
python -c "import yaml,sys; [yaml.safe_load(open(f, encoding='utf-8')) for f in ['.github/ISSUE_TEMPLATE/bug_report.yml', '.github/ISSUE_TEMPLATE/config.yml']]; print('both parse')"
```

If Python is unavailable, paste each file into GitHub's issue-form preview after pushing,
and confirm the form renders with all fields.

- [ ] **Step 6: Commit**

```bash
git add CODE_OF_CONDUCT.md .github/ISSUE_TEMPLATE .github/PULL_REQUEST_TEMPLATE.md
git commit -m "docs: add a code of conduct and contribution templates"
```

---

### Task 6: Housekeeping — dangling references, internal naming, MSRV, notices

**Problem being fixed:** four small things that each make the repo look unfinished to an outside reader.

1. **`docs/superpowers/` names a tool the reader does not have.** Six planning documents open with `**For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development`, which reads as instructions for software the reader cannot obtain. The documents themselves are valuable design history and should stay.
2. **A reference to a path that is not in the repository.** `docs/device-research.md:24` cites `.superpowers/sdd/2026-08-01-phase1-enumeration-and-probe/task-7-report.md` — never committed, so the citation cannot be followed.
3. **A dangling personal path.** `crates/headset-tray/src/ui/theme.rs:4` and the phase-3 spec cite `Documents\ShareX\Screenshots\2026-08\opera_*.png` as the source of truth for every colour — files nobody else has.
4. **An MSRV that is claimed but never tested.** `rust-version = "1.85"` while `rust-toolchain.toml` pins 1.97.1, which is what CI actually uses. The claim is untested and therefore unreliable.

**Files:**
- Rename: `docs/superpowers/` → `docs/history/`
- Create: `docs/history/README.md`
- Modify: `docs/device-research.md`, `docs/clean-room-notes.md`
- Modify: `crates/headset-tray/src/ui/theme.rs`, `docs/history/specs/2026-08-02-phase3-panel-ui-design.md`
- Modify: `Cargo.toml`, `.github/workflows/ci.yml`

- [ ] **Step 1: Rename the directory and fix the references**

```bash
git mv docs/superpowers docs/history
git grep -ln "docs/superpowers" | ForEach-Object { (Get-Content $_) -replace 'docs/superpowers', 'docs/history' | Set-Content $_ }
git grep -n "docs/superpowers"
```
Expected: the final grep prints nothing.

- [ ] **Step 2: Explain what the directory is**

Create `docs/history/README.md`:

```markdown
# Design history

Specifications and implementation plans, kept as written rather than tidied afterwards.

They are a record of how this project was designed and what was known at each point — not
current documentation. Where a plan and the code disagree, the code is right and the plan
is a historical artifact.

The preamble at the top of each plan addresses the tooling used to execute it and can be
ignored.

For current documentation see `docs/architecture.md`, `docs/device-research.md`, and
`docs/threat-model.md`.
```

- [ ] **Step 3: Remove the reference to the uncommitted report**

In `docs/device-research.md`, the sentence citing `.superpowers/sdd/...` should cite what a
reader can actually see. Replace the parenthetical

```
(`.superpowers/sdd/2026-08-01-phase1-enumeration-and-probe/task-7-report.md`)
```

with

```
(recorded during the Phase 1 enumeration work)
```

Keep the surrounding sentence and the table it introduces unchanged — the data is
first-party and still correct; only the unresolvable citation goes.

- [ ] **Step 4: Reword the mockup references**

The mockups are the genuine source of truth for the palette and that fact should survive,
but the path should not imply a file the reader can open. In
`crates/headset-tray/src/ui/theme.rs`, change the module comment's second line from

```
//! `Documents\ShareX\Screenshots\2026-08\opera_*.png`, not estimated. Text
```

to

```
//! from the design mockups (not committed to this repository), not estimated. Text
```

Apply the equivalent change to the "Source of truth for appearance" section of
`docs/history/specs/2026-08-02-phase3-panel-ui-design.md`, keeping the sentence that says
the mockups win where the document and they disagree.

- [ ] **Step 5: Reconcile the MSRV with what is actually tested**

`rust-version` is a promise. Either test it or do not make it. Testing 1.85 is awkward here
because `rust-toolchain.toml` overrides toolchain selection, so state the truth instead —
in `Cargo.toml`:

```toml
# Matches the toolchain pinned in rust-toolchain.toml, which is what CI builds and tests
# with. Not an independently verified floor: no job builds against an older compiler, so
# claiming one would be a promise nothing checks.
rust-version = "1.97"
```

- [ ] **Step 6: Make the third-party notices self-checking**

`THIRD_PARTY_NOTICES.md` says to regenerate it when dependencies change, and nothing
verifies anyone did. Add a CI step after the existing `Test` step in
`.github/workflows/ci.yml`:

```yaml
      - name: Third-party notices are current
        shell: pwsh
        run: |
          $current = cargo tree --workspace --prefix none --format "{p} {l}" |
            Sort-Object -Unique |
            Where-Object { $_ -match '\S' }
          $missing = $current | Where-Object {
            $name = ($_ -split ' ')[0]
            -not (Select-String -Path THIRD_PARTY_NOTICES.md -SimpleMatch $name -Quiet)
          }
          if ($missing) {
            Write-Host "Dependencies missing from THIRD_PARTY_NOTICES.md:"
            $missing | ForEach-Object { Write-Host "  $_" }
            exit 1
          }
          Write-Host "All dependencies are listed."
```

- [ ] **Step 7: Verify and commit**

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

Confirm no dangling references remain:

```bash
git grep -n "superpowers/sdd\|ShareX"
```
Expected: no output.

```bash
git add -A
git commit -m "docs: rename the design history, drop dangling references, reconcile the MSRV"
```

---

### Task 7: A changelog and a way to get a binary

**Problem being fixed:** the README documents `headset-tray.exe --install` without ever saying where that executable comes from. For a Windows tray utility, "clone the repo and build a GNU-target Rust workspace" is a steep first step, and `docs/release-signing.md` shows releases were always intended. There is also no changelog, so a returning user cannot tell what changed.

**Files:**
- Create: `CHANGELOG.md`
- Modify: `README.md` (an "Installing" preamble)

- [ ] **Step 1: Write the changelog**

Keep a Changelog format, seeded from the merge history. `git log --oneline --merges main` gives the shape:

```markdown
# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Noise control: off, ANC, or ambient, with an ANC level of 1–4. Exposed as `headsetctl
  noise` and as a segmented control in the tray panel. Parameter `0x12`, identified by
  capture — see `docs/device-research.md`.
- The tray icon can be operated from the keyboard: focus it and press Enter or Space.
- A high-contrast palette, applied when Windows high contrast is on.
- A stable notification-icon identity, so a pin-to-taskbar choice survives reinstalling.

### Fixed

- The tray icon no longer disappears permanently when Explorer restarts.
- The panel opens on the monitor its icon is on, rather than being clamped to the primary
  display's work area.
- The panel renders at the display's real pixel density instead of being stretched.
- A second launch raises the running instance instead of adding a duplicate tray icon.
- The panel holds its bottom edge when its height changes, instead of growing downward
  over the taskbar.
- Changing a noise setting updates the panel immediately rather than a beat later.

### Changed

- Dual-licensed under MIT OR Apache-2.0.
```

- [ ] **Step 2: Say where the binary comes from**

Insert before the existing `> headset-tray.exe --install` block in `README.md`:

```markdown
## Installing

There are no prebuilt binaries yet: releases will be signed, and the signing setup
(`docs/release-signing.md`) is not in place. Until then, build from source — see
[`CONTRIBUTING.md`](CONTRIBUTING.md) for the toolchain requirements, which are more
specific than usual:

```powershell
cargo build --release
.\target\release\headset-tray.exe --install
```
```

Adjust the wording if signed releases do exist by the time this runs; do not describe a
release process that has not happened.

- [ ] **Step 3: Verify the build instructions work as written**

```powershell
cargo build --release
Test-Path .\target\release\headset-tray.exe
Test-Path .\target\release\headsetctl.exe
```
Expected: both `True`. Do **not** run `--install` as part of verification: it replaces the
running tray and modifies the user's startup registry.

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md README.md
git commit -m "docs: add a changelog and say where the binary comes from"
```

---

## Done when

- `LICENSE-MIT` and `LICENSE-APACHE` exist, and every crate reports `MIT OR Apache-2.0`
  from `cargo metadata`.
- No document describes the project as private, and none cites a heading, file, or path
  that does not exist.
- The unnamed-parameter count is pinned by a test, so README prose cannot silently drift
  from the protocol again.
- `SECURITY.md` names a reporting channel that works and describes the current write
  posture rather than Phase 1's.
- `CONTRIBUTING.md` documents the `x86_64-pc-windows-gnu` requirement and the `cargo
  update` hazard, and every command in it has been run.
- A code of conduct, an issue form, and a pull-request template exist, and the templates
  ask for redaction and for capture evidence.
- `CHANGELOG.md` exists and the README says where a binary comes from.
- The full gate passes.

## Explicitly not in this plan

- **Making the repository public.** That is the owner's action, and `CONTRIBUTING.md`
  forbids doing it without explicit instruction. This plan prepares for it.
- **Publishing to crates.io.** `publish = false` stays. A licence is required for
  publishing but publishing is not required by a licence, and the crates have no stable
  API to promise.
- **Signed releases.** `docs/release-signing.md` describes the intent; setting up the
  signing material and a release workflow is separate work.
