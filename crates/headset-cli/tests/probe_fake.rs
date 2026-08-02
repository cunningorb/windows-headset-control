use headset_cli::cli::ProbeArgs;
use headset_cli::cmd::probe;
use headset_cli::redact::Redactor;
use headset_device::{FakeHidBackend, HidBackend};

const FIXTURE: &str = include_str!("../../headset-device/tests/fixtures/blackshark-v3-pro-ps.json");
const FIXTURE_WITH_INTERLOPER: &str =
    include_str!("../../headset-device/tests/fixtures/blackshark-plus-interloper.json");

fn no_candidate(listen_ms: u64) -> ProbeArgs {
    ProbeArgs {
        candidate: None,
        vendor_id: None,
        product_id: None,
        listen_ms,
    }
}

#[test]
fn probe_reports_silence_when_the_device_sends_nothing() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let args = no_candidate(100);
    let out = probe::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["wrote_to_device"], false);
    assert_eq!(v["result"]["status"], "silent");
}

#[test]
fn probe_selects_the_control_collection_by_default_on_the_standard_fixture() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let args = no_candidate(100);
    let out = probe::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["usage_page"], "0xff14");
}

#[test]
fn probe_scopes_automatic_selection_to_the_supported_device_and_ignores_a_higher_scoring_interloper(
) {
    // The fixture contains a fifth, unrelated vendor-defined collection
    // (0xFFA0, 257-byte feature report) that outscores every headset
    // collection under `rank_candidates`'s shape-only formula. Bare `probe`
    // must still land on the headset's own 0xFF14 collection, not the
    // interloper: this is the exact failure mode the supported-device scope
    // in `cmd::probe::run` exists to prevent.
    let backend = FakeHidBackend::from_fixture_str(FIXTURE_WITH_INTERLOPER).unwrap();

    // Sanity check the fixture actually reproduces the bug shape: the
    // interloper must outrank the headset under plain `rank_candidates`.
    let mut all = backend.enumerate().unwrap();
    headset_device::stable_sort_collections(&mut all);
    let ranked = headset_device::rank_candidates(&all);
    let best = ranked.first().unwrap();
    assert_eq!(
        all[best.index].usage_page, 0xFFA0,
        "fixture setup check: the interloper should outscore the headset"
    );

    let args = no_candidate(100);
    let out = probe::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["usage_page"], "0xff14");
}

#[test]
fn probe_vendor_id_filter_matching_nothing_produces_a_clear_not_found_error() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let args = ProbeArgs {
        candidate: None,
        vendor_id: Some(0x9999),
        product_id: None,
        listen_ms: 100,
    };
    let err = probe::run(&backend, &args, &Redactor::new(false), true).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("no supported device found") || msg.contains("0x9999"),
        "expected a clear not-found message, got: {msg}"
    );
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

    let args = no_candidate(100);
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
        vendor_id: None,
        product_id: None,
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
        vendor_id: None,
        product_id: None,
        listen_ms: 100,
    };
    let err = probe::run(&backend, &args, &Redactor::new(false), true).unwrap_err();
    assert!(err.to_string().contains("out of range"));
}

#[test]
fn probe_candidate_index_still_means_the_same_collection_as_list() {
    // An explicit --candidate keeps its absolute-index meaning even when the
    // fixture contains collections that automatic selection would skip: the
    // headset's 0xFF13 collection (index 1 in this sorted fixture) is
    // reachable explicitly, exactly as it is via `headsetctl list`.
    let backend = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let mut all = backend.enumerate().unwrap();
    headset_device::stable_sort_collections(&mut all);
    let ff13_index = all.iter().position(|c| c.usage_page == 0xFF13).unwrap();

    let args = ProbeArgs {
        candidate: Some(ff13_index),
        vendor_id: None,
        product_id: None,
        listen_ms: 100,
    };
    let out = probe::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["usage_page"], "0xff13");
}
