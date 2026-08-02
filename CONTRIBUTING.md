# Contributing

Thanks for looking. This is a small, opinionated project: it talks to one headset over a
protocol reconstructed by observation, and most of its rules exist to keep that honest.

## Before you start

**Read [`docs/device-research.md`](docs/device-research.md).** It is the record of what was
observed on the wire and what deliberately remains unnamed. Nearly every rule below follows
from it.

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
panel layout, protocol codec, and placement geometry are pure and fully unit-tested. The
tests that do need a headset are `#[ignore]`d *and* gated behind an environment variable,
so they are skipped twice over by default:

```powershell
$env:HEADSET_HARDWARE_TESTS = "1"
cargo test -p headset-device -- --ignored
```

To see the tray panel without a headset:

```powershell
cargo run --release -p headset-tray -- --render-panel .\out
```

That renders every panel state to PNG through the same Direct2D path the live window uses,
which is how appearance changes are reviewed.

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
  project.** See [`docs/clean-room-notes.md`](docs/clean-room-notes.md) for what has been
  consulted and on what terms. If you consult something new, record it there in the same
  pull request.
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

Contributions are dual licensed under MIT OR Apache-2.0; see the README.
