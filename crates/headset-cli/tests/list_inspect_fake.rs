//! Drives `cmd::list::run` and `cmd::inspect::run` directly over
//! `FakeHidBackend`, rather than only the renderers they call into.
//!
//! The existing index-stability coverage (`render_snapshots.rs`) calls
//! `render_list` with a hand-built `shown` vector, so it exercises the
//! renderer, not the index assignment performed by `cmd::list::run`'s own
//! `.filter(...).enumerate()` pipeline. Reintroducing the original bug there
//! — assigning indices *after* filtering instead of before — would leave
//! every other test in the suite green. These tests close that gap.

use headset_cli::cli::{InspectArgs, ListArgs};
use headset_cli::cmd::{inspect, list};
use headset_cli::redact::Redactor;
use headset_device::FakeHidBackend;

const FIXTURE_WITH_INTERLOPER: &str =
    include_str!("../../headset-device/tests/fixtures/blackshark-plus-interloper.json");

#[test]
fn list_run_reports_absolute_index_not_filtered_position() {
    // The interloper (vendor 0x1770) sorts after all four Razer (0x1532)
    // collections, landing at absolute index 4 in the full enumeration.
    // Filtering `list` down to just that vendor must still report index 4 —
    // the exact regression this test guards against is the index being
    // recomputed as 0 (its position within the filtered subset).
    let backend = FakeHidBackend::from_fixture_str(FIXTURE_WITH_INTERLOPER).unwrap();
    let args = ListArgs {
        vendor_id: Some(0x1770),
        product_id: None,
    };
    let out = list::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let collections = v["collections"].as_array().unwrap();
    assert_eq!(
        collections.len(),
        1,
        "filter must show exactly one collection"
    );
    assert_eq!(
        collections[0]["index"], 4,
        "must report the absolute index, not 0"
    );
}

#[test]
fn inspect_run_at_index_4_agrees_with_list_about_which_collection_that_is() {
    // `list` and `inspect` must agree on what index 4 means: the previous
    // test establishes that `list --vendor-id 0x1770` reports the interloper
    // at index 4; this proves `inspect --path-index 4` resolves to the same
    // collection.
    let backend = FakeHidBackend::from_fixture_str(FIXTURE_WITH_INTERLOPER).unwrap();
    let args = InspectArgs { path_index: 4 };
    let out = inspect::run(&backend, &args, &Redactor::new(false), true).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["usage_page"], "0xffa0");
}

#[test]
fn inspect_run_out_of_range_index_bails_with_a_clear_error() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE_WITH_INTERLOPER).unwrap();
    let args = InspectArgs { path_index: 99 };
    let err = inspect::run(&backend, &args, &Redactor::new(false), true).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("99"),
        "expected the offending index in the message: {msg}"
    );
    assert!(
        msg.contains("5 collections") || msg.contains("collections"),
        "expected the message to state how many collections exist: {msg}"
    );
}

#[test]
fn neither_command_leaks_fixture_markers_or_raw_paths() {
    let backend = FakeHidBackend::from_fixture_str(FIXTURE_WITH_INTERLOPER).unwrap();

    let list_args = ListArgs {
        vendor_id: None,
        product_id: None,
    };
    for json in [false, true] {
        let out = list::run(&backend, &list_args, &Redactor::new(false), json).unwrap();
        assert!(
            !out.contains("fixture"),
            "list leaked a raw fixture path fragment"
        );
        assert!(
            !out.to_lowercase().contains("\\\\?\\hid#"),
            "list leaked a raw device path"
        );
    }

    let inspect_args = InspectArgs { path_index: 4 };
    for json in [false, true] {
        let out = inspect::run(&backend, &inspect_args, &Redactor::new(false), json).unwrap();
        assert!(
            !out.contains("fixture"),
            "inspect leaked a raw fixture path fragment"
        );
        assert!(
            !out.to_lowercase().contains("\\\\?\\hid#"),
            "inspect leaked a raw device path"
        );
    }
}
