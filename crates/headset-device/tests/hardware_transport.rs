//! Hardware test. Read-only. Excluded from CI.

#![cfg(windows)]

use std::time::{Duration, Instant};

use headset_device::{DeviceError, HidBackend, OpenMode, WindowsHidBackend};

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

    // A zero-access-rights handle must not be able to read. This is the
    // property `OpenMode::Descriptors` exists to guarantee (see
    // `docs/threat-model.md`'s audio-stack-contention row), but until now
    // nothing asserted it directly.
    let mut buf = vec![0u8; h.input_report_len() as usize];
    let err = h
        .read_report(&mut buf, Duration::from_millis(200))
        .expect_err("a zero-access handle must not be able to read");
    assert!(
        matches!(err, headset_device::DeviceError::AccessDenied),
        "expected AccessDenied, got: {err:?}"
    );
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
        .open(&target.id, OpenMode::Read)
        .expect("read open succeeds");
    let mut buf = vec![0u8; t.input_report_len() as usize];
    let window = Duration::from_millis(500);
    let start = Instant::now();
    match t.read_report(&mut buf, window) {
        // Accepting any `n <= buf.len()` here would also accept `Ok(0)`,
        // which `read_report` never legitimately returns for this device (a
        // real report always carries the report-ID byte). Pin down the shape
        // an actual received report must have instead.
        Ok(n) => {
            assert_eq!(
                n,
                buf.len(),
                "a real report fills the declared report length"
            );
            assert_eq!(buf[0], 0x02, "unexpected report id");
        }
        Err(DeviceError::Timeout(_)) => {
            // The property this test exists to prove is that the
            // cancel-and-synchronize path in `ffi::read_with_timeout`
            // actually returns control to the caller rather than hanging
            // indefinitely on `GetOverlappedResult(..., bWait = true)`.
            // Bounding elapsed wall time against the requested window is what
            // proves that, not merely that `Timeout` was the variant returned.
            let elapsed = start.elapsed();
            assert!(
                elapsed < window * 2,
                "read_with_timeout took {elapsed:?} to return for a {window:?} timeout; \
                 the cancel-and-synchronize path may be hanging"
            );
        }
        Err(e) => panic!("unexpected error: {e}"),
    }
}
