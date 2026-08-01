//! Hardware test. Requires a real device and is excluded from CI.
//! Run with: $env:HEADSET_HARDWARE_TESTS=1; cargo test -p headset-device -- --ignored

#![cfg(windows)]

use headset_device::{HidBackend, WindowsHidBackend};

fn hardware_enabled() -> bool {
    std::env::var("HEADSET_HARDWARE_TESTS").as_deref() == Ok("1")
}

#[test]
#[ignore = "requires attached hardware"]
fn enumerates_at_least_one_collection() {
    if !hardware_enabled() {
        eprintln!("skipping: set HEADSET_HARDWARE_TESTS=1 to run");
        return;
    }
    let all = WindowsHidBackend::new()
        .enumerate()
        .expect("enumeration succeeds");
    assert!(!all.is_empty(), "expected at least one HID collection");
}
