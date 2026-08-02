use headset_cli::cli::ProbeArgs;
use headset_cli::cmd::probe;
use headset_cli::redact::Redactor;
use headset_device::{FakeHidBackend, HidBackend};

const FIXTURE: &str = include_str!("../../headset-device/tests/fixtures/blackshark-v3-pro-ps.json");

#[test]
fn probe_reports_silence_when_the_device_sends_nothing() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let args = ProbeArgs {
        candidate: None,
        listen_ms: 100,
    };
    let out = probe::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["wrote_to_device"], false);
    assert_eq!(v["result"]["status"], "silent");
}

#[test]
fn probe_parses_a_queued_control_frame() {
    let mut backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let all = backend.enumerate().unwrap();
    let control = all.iter().find(|c| c.usage_page == 0xFF14).unwrap();
    let mut report = vec![0u8; 64];
    report[0] = 0x02;
    report[1] = 0xDE;
    report[2] = 0xAD;
    backend.push_read(&control.id, report);

    let args = ProbeArgs {
        candidate: None,
        listen_ms: 100,
    };
    let out = probe::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["result"]["status"], "frame_received");
    assert!(v["result"]["payload_hex"]
        .as_str()
        .unwrap()
        .starts_with("de ad"));
    assert_eq!(v["result"]["interpreted_fields"], serde_json::json!({}));
}

#[test]
fn probe_refuses_a_disqualified_candidate() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let all = backend.enumerate().unwrap();
    let mut sorted = all.clone();
    headset_device::stable_sort_collections(&mut sorted);
    let audio_index = sorted
        .iter()
        .position(|c| c.is_audio_stack_collection())
        .unwrap();

    let args = ProbeArgs {
        candidate: Some(audio_index),
        listen_ms: 100,
    };
    let err = probe::run(&backend, &args, &Redactor::new(false), true).unwrap_err();
    assert!(err.to_string().contains("disqualified"));
}

#[test]
fn probe_rejects_an_out_of_range_candidate() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let args = ProbeArgs {
        candidate: Some(99),
        listen_ms: 100,
    };
    let err = probe::run(&backend, &args, &Redactor::new(false), true).unwrap_err();
    assert!(err.to_string().contains("out of range"));
}
