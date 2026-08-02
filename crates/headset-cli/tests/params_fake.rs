//! `get` / `set` / `param` / `watch` against the fixture-driven fake backend.
//! No hardware, no writes to a real device.

use headset_cli::cli::{GetArgs, ParamAction, ParamArgs, SetArgs, WatchArgs};
use headset_cli::cmd::params;
use headset_device::{resolve_control_device, FakeHidBackend};
use headset_protocol::{checksum, Param, CONTROL_REPORT_LEN};

const FIXTURE: &str = include_str!("../../headset-device/tests/fixtures/blackshark-v3-pro-ps.json");

/// Builds a response frame the way the device was observed building one.
fn response(param: u8, is_write: bool, payload: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; CONTROL_REPORT_LEN];
    b[0] = 0x02;
    b[1] = 0x02;
    b[2] = 0x60;
    b[6] = 4 + payload.len() as u8;
    b[9] = 0x80;
    b[10] = if is_write { param | 0x80 } else { param };
    b[11] = 0x01;
    b[12] = payload.len() as u8;
    b[13..13 + payload.len()].copy_from_slice(payload);
    b[62] = checksum(&b);
    b
}

fn event(param: u8, payload: &[u8]) -> Vec<u8> {
    let mut b = response(param, false, payload);
    b[9] = 0x00;
    b[11] = 0x02;
    b[62] = 0;
    b[62] = checksum(&b);
    b
}

fn backend_with(reports: Vec<Vec<u8>>) -> FakeHidBackend {
    let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
    let info = resolve_control_device(&b).unwrap();
    for r in reports {
        b.push_read(&info.id, r);
    }
    b
}

#[test]
fn get_battery_reports_the_value_the_device_returned() {
    let b = backend_with(vec![response(Param::Battery.id(), false, &[49])]);
    let args = GetArgs {
        name: "battery".into(),
    };
    let out = params::run_get(&b, &args, false).unwrap();
    assert_eq!(out, "battery: 49\n");
}

#[test]
fn get_json_carries_the_id_and_raw_payload() {
    let b = backend_with(vec![response(Param::Battery.id(), false, &[49])]);
    let args = GetArgs {
        name: "battery".into(),
    };
    let out = params::run_get(&b, &args, true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["parameter"], "battery");
    assert_eq!(v["id"], "0x21");
    assert_eq!(v["value"], 49);
    assert_eq!(v["payload_hex"], "31");
}

#[test]
fn a_refused_read_is_reported_as_unavailable_not_as_255() {
    // Observed on real hardware with the headset powered off: link reports
    // 00 00 and every proxied parameter answers with a lone 0xff.
    let b = backend_with(vec![
        response(Param::Battery.id(), false, &[0xFF]),
        response(Param::LinkState.id(), false, &[0x00, 0x00]),
    ]);
    let args = GetArgs {
        name: "battery".into(),
    };
    let out = params::run_get(&b, &args, false).unwrap();
    assert!(out.contains("unavailable"), "{out}");
    assert!(out.contains("powered off"), "{out}");
    assert!(!out.contains("255"), "255 is not a battery level: {out}");
}

#[test]
fn a_refused_read_reports_null_in_json_rather_than_a_wrong_number() {
    let b = backend_with(vec![
        response(Param::Battery.id(), false, &[0xFF]),
        response(Param::LinkState.id(), false, &[0x00, 0x00]),
    ]);
    let args = GetArgs {
        name: "battery".into(),
    };
    let out = params::run_get(&b, &args, true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["value"].is_null());
    assert_eq!(v["available"], false);
    assert_eq!(v["headset_connected"], false);
    assert_eq!(v["payload_hex"], "ff");
}

#[test]
fn link_state_reads_as_words_not_raw_bytes() {
    let b = backend_with(vec![response(Param::LinkState.id(), false, &[0x01, 0x00])]);
    let args = GetArgs {
        name: "link".into(),
    };
    assert_eq!(
        params::run_get(&b, &args, false).unwrap(),
        "link: connected\n"
    );

    let b = backend_with(vec![response(Param::LinkState.id(), false, &[0x00, 0x00])]);
    assert_eq!(
        params::run_get(&b, &args, false).unwrap(),
        "link: not connected\n"
    );
}

#[test]
fn a_healthy_read_costs_one_exchange() {
    // The link probe exists to explain a refusal; it must not be paid for on
    // every successful read, since each exchange is paced at 250 ms.
    let b = backend_with(vec![response(Param::Battery.id(), false, &[49])]);
    let args = GetArgs {
        name: "battery".into(),
    };
    params::run_get(&b, &args, false).unwrap();
    assert_eq!(b.writes().len(), 1);
}

#[test]
fn raw_param_get_does_not_claim_ff_means_unavailable() {
    // An unidentified parameter might legitimately hold 0xff; the raw path
    // prints the byte rather than interpreting it.
    let b = backend_with(vec![response(0x2C, false, &[0xFF])]);
    let args = ParamArgs {
        action: ParamAction::Get {
            id: 0x2C,
            index: None,
        },
    };
    let out = params::run_param(&b, &args, false).unwrap();
    assert_eq!(out, "0x2c: ff\n");
}

#[test]
fn get_rejects_an_unknown_name_and_lists_the_known_ones() {
    let b = backend_with(vec![]);
    let args = GetArgs {
        name: "volume".into(),
    };
    let err = params::run_get(&b, &args, false).unwrap_err().to_string();
    assert!(err.contains("unknown parameter `volume`"), "{err}");
    assert!(err.contains("battery"), "{err}");
}

#[test]
fn set_sidetone_sends_the_observed_enable_preamble_then_the_level() {
    let b = backend_with(vec![
        response(0x18, true, &[0x00]),                 // enable ack
        response(Param::Sidetone.id(), true, &[0x00]), // level ack
        response(Param::Sidetone.id(), false, &[7]),   // read-back
    ]);
    let args = SetArgs {
        name: "sidetone".into(),
        value: 7,
    };
    let out = params::run_set(&b, &args, false).unwrap();
    assert_eq!(out, "sidetone: 7\n");

    let writes = b.writes();
    assert_eq!(writes.len(), 3, "enable, level, read-back");
    assert_eq!(writes[0][10], 0x98, "0x18 | write bit");
    assert_eq!(writes[1][10], 0x99, "0x19 | write bit");
    assert_eq!(writes[1][13], 7);
    assert_eq!(writes[2][10], 0x19, "read-back has the write bit clear");
}

#[test]
fn set_game_chat_sends_no_preamble() {
    let b = backend_with(vec![
        response(Param::GameChatBalance.id(), true, &[0x00]),
        response(Param::GameChatBalance.id(), false, &[10]),
    ]);
    let args = SetArgs {
        name: "game-chat".into(),
        value: 10,
    };
    params::run_set(&b, &args, false).unwrap();

    let writes = b.writes();
    assert_eq!(
        writes.len(),
        2,
        "game/chat was never observed with a preamble"
    );
    assert_eq!(writes[0][10], 0xDC);
}

#[test]
fn a_refused_write_is_an_error_not_a_success() {
    let b = backend_with(vec![
        response(0x18, true, &[0xFF]), // the device refuses while mic-muted
    ]);
    let args = SetArgs {
        name: "sidetone".into(),
        value: 7,
    };
    let err = params::run_set(&b, &args, false).unwrap_err().to_string();
    assert!(err.contains("refused"), "{err}");
    assert!(err.contains("0xff"), "{err}");
}

#[test]
fn an_unrecognised_result_code_is_not_treated_as_success() {
    let b = backend_with(vec![
        response(0x18, true, &[0x00]),
        response(Param::Sidetone.id(), true, &[0x42]),
    ]);
    let args = SetArgs {
        name: "sidetone".into(),
        value: 7,
    };
    let err = params::run_set(&b, &args, false).unwrap_err().to_string();
    assert!(err.contains("unrecognised result"), "{err}");
}

#[test]
fn set_reports_a_readback_that_disagrees_with_what_was_written() {
    // The device accepting a write is not evidence it applied it.
    let b = backend_with(vec![
        response(0x18, true, &[0x00]),
        response(Param::Sidetone.id(), true, &[0x00]),
        response(Param::Sidetone.id(), false, &[3]),
    ]);
    let args = SetArgs {
        name: "sidetone".into(),
        value: 7,
    };
    let out = params::run_set(&b, &args, false).unwrap();
    assert!(out.contains("wrote 7 but the device reports 3"), "{out}");
}

#[test]
fn set_rejects_a_value_outside_the_observed_range() {
    let b = backend_with(vec![]);
    let args = SetArgs {
        name: "sidetone".into(),
        value: 16,
    };
    let err = params::run_set(&b, &args, false).unwrap_err().to_string();
    assert!(err.contains("0..=15"), "{err}");
    assert!(b.writes().is_empty(), "nothing may reach the wire");
}

#[test]
fn set_refuses_a_parameter_no_write_was_observed_for() {
    let b = backend_with(vec![]);
    for name in ["battery", "mic-mute", "link"] {
        let args = SetArgs {
            name: name.into(),
            value: 1,
        };
        let err = params::run_set(&b, &args, false).unwrap_err().to_string();
        assert!(err.contains("read-only"), "{name}: {err}");
    }
    assert!(b.writes().is_empty());
}

#[test]
fn param_get_reaches_an_unidentified_parameter_without_naming_it() {
    let b = backend_with(vec![response(0x2C, false, &[0x0F])]);
    let args = ParamArgs {
        action: ParamAction::Get {
            id: 0x2C,
            index: None,
        },
    };
    let out = params::run_param(&b, &args, true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["id"], "0x2c");
    assert_eq!(v["payload_hex"], "0f");
    assert!(
        v["interpretation"].is_null(),
        "an unidentified parameter must not be given a meaning"
    );
}

#[test]
fn param_get_refuses_an_identifier_never_seen_on_the_wire() {
    let b = backend_with(vec![]);
    let args = ParamArgs {
        action: ParamAction::Get {
            id: 0x33,
            index: None,
        },
    };
    let err = params::run_param(&b, &args, false).unwrap_err().to_string();
    assert!(err.contains("not in the observed read allowlist"), "{err}");
    assert!(b.writes().is_empty(), "a speculative id must not be sent");
}

#[test]
fn param_set_refuses_an_identifier_never_seen_being_written() {
    let b = backend_with(vec![]);
    // 0x21 (battery) is readable, but no write for it was ever observed.
    let args = ParamArgs {
        action: ParamAction::Set { id: 0x21, value: 1 },
    };
    let err = params::run_param(&b, &args, false).unwrap_err().to_string();
    assert!(err.contains("not in the observed write allowlist"), "{err}");
    assert!(b.writes().is_empty());
}

#[test]
fn param_get_rejects_an_index_on_a_non_indexed_parameter() {
    let b = backend_with(vec![]);
    let args = ParamArgs {
        action: ParamAction::Get {
            id: 0x21,
            index: Some(2),
        },
    };
    let err = params::run_param(&b, &args, false).unwrap_err().to_string();
    assert!(err.contains("was not observed taking an index"), "{err}");
}

#[test]
fn param_get_passes_an_index_through_for_the_indexed_parameters() {
    let b = backend_with(vec![response(
        0x60,
        false,
        &[0x03, 0x02, 0x01, 0x00, 0x03, 0x00],
    )]);
    let args = ParamArgs {
        action: ParamAction::Get {
            id: 0x60,
            index: Some(3),
        },
    };
    params::run_param(&b, &args, false).unwrap();
    let writes = b.writes();
    assert_eq!(writes[0][12], 1, "one payload byte");
    assert_eq!(writes[0][13], 3, "the index");
}

#[test]
fn watch_names_known_parameters_and_leaves_unknown_ones_unnamed() {
    let b = backend_with(vec![
        event(Param::Battery.id(), &[49]),
        event(0x2C, &[0x0F]),
    ]);
    let args = WatchArgs { seconds: 1 };
    let out = params::run_watch(&b, &args, false).unwrap();
    assert!(out.contains("battery"), "{out}");
    assert!(out.contains("unidentified"), "{out}");
    assert!(!out.contains("0x2c unidentified   0f\n0x2c"), "{out}");
}

#[test]
fn watch_writes_nothing() {
    let b = backend_with(vec![event(Param::MicMute.id(), &[1])]);
    let args = WatchArgs { seconds: 1 };
    let out = params::run_watch(&b, &args, true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["wrote_to_device"], false);
    assert!(b.writes().is_empty(), "watch must not write");
}

#[test]
fn watch_explains_silence_rather_than_implying_a_fault() {
    let b = backend_with(vec![]);
    let args = WatchArgs { seconds: 1 };
    let out = params::run_watch(&b, &args, false).unwrap();
    assert!(out.contains("pushes only when something changes"), "{out}");
}
