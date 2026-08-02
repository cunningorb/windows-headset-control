//! Embeds the application icon.
//!
//! Uses `windres` from the GNU toolchain this project targets rather than a
//! resource crate, which keeps the dependency count at zero. A missing
//! `windres` is a warning and not an error: losing the icon is not a reason to
//! be unable to build.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=headset-tray.rc");
    println!("cargo:rerun-if-changed=assets/headset.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("headset-tray-rc.o");

    match Command::new("windres")
        .args(["headset-tray.rc", "-O", "coff", "-o"])
        .arg(&out)
        .status()
    {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg-bins={}", out.display());
        }
        Ok(s) => println!("cargo:warning=windres failed ({s}); building without an icon"),
        Err(e) => println!("cargo:warning=windres unavailable ({e}); building without an icon"),
    }
}
