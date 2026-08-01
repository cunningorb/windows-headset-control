# Contributing

## Before every push

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

## Hard rules

- Never copy source, comments, assets, or structure from an unlicensed third-party project.
- Never commit `.pfx`, `.p12`, private keys, passwords, or signing tokens.
- Never add a HID write without adding it to the allowlist and documenting the rationale.
- Never send speculative or brute-forced HID command identifiers.
- Never publish or change repository visibility without explicit instruction.
- Redact serial numbers and device paths in issues, logs, and commit messages.
