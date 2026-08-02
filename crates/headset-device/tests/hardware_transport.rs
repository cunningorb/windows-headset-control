//! Hardware test. Read-only. Excluded from CI.

#![cfg(windows)]

use std::time::Duration;

use headset_device::{HidBackend, OpenMode, WindowsHidBackend};

fn hardware_enabled() -> bool {
    std::env::var("HEADSET_HARDWARE_TESTS").as_deref() == Ok("1")
}

#[test]
#[ignore = "requires attached hardware"]
fn descriptor_handle_opens_and_closes() {
    if !hardware_enabled() {
        eprintln!("skipping: set HEADSET_HARDWARE_TESTS=1 to run");
        return;
    }
    let backend = WindowsHidBackend::new();
    let all = backend.enumerate().unwrap();
    let target = all
        .iter()
        .find(|c| c.usage_page == 0xFF14)
        .expect("control collection present");
    let h = backend
        .open(&target.id, OpenMode::Descriptors)
        .expect("descriptor open succeeds");
    assert_eq!(h.input_report_len(), 64);
}

#[test]
#[ignore = "requires attached hardware"]
fn read_times_out_cleanly_when_device_is_silent() {
    if !hardware_enabled() {
        eprintln!("skipping: set HEADSET_HARDWARE_TESTS=1 to run");
        return;
    }
    let backend = WindowsHidBackend::new();
    let all = backend.enumerate().unwrap();
    let target = all.iter().find(|c| c.usage_page == 0xFF14).unwrap();
    let t = backend
        .open(&target.id, OpenMode::ReadWrite)
        .expect("read open succeeds");
    let mut buf = vec![0u8; t.input_report_len() as usize];
    match t.read_report(&mut buf, Duration::from_millis(500)) {
        Ok(n) => assert!(n <= buf.len()),
        Err(headset_device::DeviceError::Timeout(_)) => {}
        Err(e) => panic!("unexpected error: {e}"),
    }
}
