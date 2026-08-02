# Third-Party Notices

This product bundles no third-party source code. It links Rust crates whose licenses
are reproduced below. Regenerate when dependencies change:

```powershell
cargo tree --workspace --target x86_64-pc-windows-gnu --prefix none --format "{p} {l}"
```

`--target` is required, not decorative. Without it `cargo tree` resolves for whichever
host you happen to be on, which decides whether `windows_x86_64_gnu` or
`windows_x86_64_msvc` appears. This list describes the target this project actually
builds and ships — see `rust-toolchain.toml`. CI checks it with the same command.

| Crate | License |
| ----- | ------- |
| anstream v1.0.0 | MIT OR Apache-2.0 |
| anstyle v1.0.14 | MIT OR Apache-2.0 |
| anstyle-parse v1.0.0 | MIT OR Apache-2.0 |
| anstyle-query v1.1.5 | MIT OR Apache-2.0 |
| anstyle-wincon v3.0.11 | MIT OR Apache-2.0 |
| anyhow v1.0.104 | MIT OR Apache-2.0 |
| block-buffer v0.10.4 | MIT OR Apache-2.0 |
| cfg-if v1.0.4 | MIT OR Apache-2.0 |
| clap v4.6.5 | MIT OR Apache-2.0 |
| clap_builder v4.6.5 | MIT OR Apache-2.0 |
| clap_derive v4.6.4 | MIT OR Apache-2.0 |
| clap_lex v1.1.0 | MIT OR Apache-2.0 |
| colorchoice v1.0.5 | MIT OR Apache-2.0 |
| console v0.16.0 | MIT |
| cpufeatures v0.2.17 | MIT OR Apache-2.0 |
| crypto-common v0.1.7 | MIT OR Apache-2.0 |
| digest v0.10.7 | MIT OR Apache-2.0 |
| encode_unicode v1.0.0 | Apache-2.0 OR MIT |
| fastrand v2.5.0 | Apache-2.0 OR MIT |
| generic-array v0.14.7 | MIT |
| getrandom v0.4.3 | MIT OR Apache-2.0 |
| heck v0.5.0 | MIT OR Apache-2.0 |
| insta v1.48.0 | Apache-2.0 |
| is_terminal_polyfill v1.70.2 | MIT OR Apache-2.0 |
| itoa v1.0.18 | MIT OR Apache-2.0 |
| lazy_static v1.5.0 | MIT OR Apache-2.0 |
| libc v0.2.189 | MIT OR Apache-2.0 |
| log v0.4.33 | MIT OR Apache-2.0 |
| matchers v0.2.0 | MIT |
| memchr v2.8.3 | Unlicense OR MIT |
| nu-ansi-term v0.50.3 | MIT |
| once_cell v1.21.4 | MIT OR Apache-2.0 |
| once_cell_polyfill v1.70.2 | MIT OR Apache-2.0 |
| pin-project-lite v0.2.17 | Apache-2.0 OR MIT |
| proc-macro2 v1.0.107 | MIT OR Apache-2.0 |
| quote v1.0.47 | MIT OR Apache-2.0 |
| regex-automata v0.4.16 | MIT OR Apache-2.0 |
| regex-syntax v0.8.11 | MIT OR Apache-2.0 |
| serde v1.0.229 | MIT OR Apache-2.0 |
| serde_core v1.0.229 | MIT OR Apache-2.0 |
| serde_derive v1.0.229 | MIT OR Apache-2.0 |
| serde_json v1.0.151 | MIT OR Apache-2.0 |
| sha2 v0.10.9 | MIT OR Apache-2.0 |
| sharded-slab v0.1.7 | MIT |
| similar v2.7.0 | Apache-2.0 |
| smallvec v1.15.2 | MIT OR Apache-2.0 |
| strsim v0.11.1 | MIT |
| syn v2.0.119 | MIT OR Apache-2.0 |
| syn v3.0.3 | MIT OR Apache-2.0 |
| tempfile v3.27.0 | MIT OR Apache-2.0 |
| thiserror v2.0.19 | MIT OR Apache-2.0 |
| thiserror-impl v2.0.19 | MIT OR Apache-2.0 |
| thread_local v1.1.10 | MIT OR Apache-2.0 |
| tracing v0.1.44 | MIT |
| tracing-attributes v0.1.31 | MIT |
| tracing-core v0.1.36 | MIT |
| tracing-log v0.2.0 | MIT |
| tracing-subscriber v0.3.23 | MIT |
| typenum v1.20.1 | MIT OR Apache-2.0 |
| unicode-ident v1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| utf8parse v0.2.2 | Apache-2.0 OR MIT |
| version_check v0.9.5 | MIT/Apache-2.0 |
| windows v0.58.0 | MIT OR Apache-2.0 |
| windows-core v0.58.0 | MIT OR Apache-2.0 |
| windows-implement v0.58.0 | MIT OR Apache-2.0 |
| windows-interface v0.58.0 | MIT OR Apache-2.0 |
| windows-result v0.2.0 | MIT OR Apache-2.0 |
| windows-strings v0.1.0 | MIT OR Apache-2.0 |
| windows-sys v0.60.2 | MIT OR Apache-2.0 |
| windows-targets v0.52.6 | MIT OR Apache-2.0 |
| windows-targets v0.53.2 | MIT OR Apache-2.0 |
| windows_x86_64_gnu v0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnu v0.53.1 | MIT OR Apache-2.0 |
| zmij v1.0.23 | MIT |

All licenses above are permissive (MIT, Apache-2.0, or Unlicense/MIT dual). None are
copyleft. `unicode-ident` additionally carries the Unicode-3.0 data license, which is
also permissive.

No source code from any third-party reverse-engineering project is included.
See `docs/clean-room-notes.md`.
