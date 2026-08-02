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
