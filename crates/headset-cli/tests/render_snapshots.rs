use headset_cli::redact::Redactor;
use headset_cli::render::{render_inspect, render_list};
use headset_device::{
    rank_candidates, stable_sort_collections, CollectionInfo, FakeHidBackend, HidBackend,
};

const FIXTURE: &str = include_str!("../../headset-device/tests/fixtures/blackshark-v3-pro-ps.json");

fn sorted() -> Vec<CollectionInfo> {
    let mut all = FakeHidBackend::from_fixture_str(FIXTURE)
        .unwrap()
        .enumerate()
        .unwrap();
    stable_sort_collections(&mut all);
    all
}

#[test]
fn list_human_output_is_stable() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let out = render_list(&all, &ranked, &Redactor::new(false), false);
    insta::assert_snapshot!("list_human", out);
}

#[test]
fn list_json_output_is_stable() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let out = render_list(&all, &ranked, &Redactor::new(false), true);
    insta::assert_snapshot!("list_json", out);
}

#[test]
fn list_json_has_the_documented_envelope() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let out = render_list(&all, &ranked, &Redactor::new(false), true);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert!(v["collections"].is_array());
    assert_eq!(v["collections"].as_array().unwrap().len(), 4);
    let first = &v["collections"][0];
    for key in [
        "index",
        "vendor_id",
        "product_id",
        "version",
        "interface_number",
        "collection_number",
        "usage_page",
        "usage",
        "input_report_len",
        "output_report_len",
        "feature_report_len",
        "product",
        "manufacturer",
        "serial",
        "path",
        "score",
        "disqualified",
    ] {
        assert!(first.get(key).is_some(), "missing key `{key}`");
    }
}

#[test]
fn no_output_contains_a_raw_path_by_default() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    for json in [false, true] {
        let out = render_list(&all, &ranked, &Redactor::new(false), json);
        assert!(!out.contains("fixture"), "raw path fragment leaked");
        assert!(
            !out.to_lowercase().contains("\\\\?\\hid#"),
            "raw path leaked"
        );
    }
}

#[test]
fn inspect_human_output_is_stable() {
    let all = sorted();
    let control = all.iter().find(|c| c.usage_page == 0xFF14).unwrap();
    let out = render_inspect(control, &Redactor::new(false), false);
    insta::assert_snapshot!("inspect_human", out);
}

#[test]
fn inspect_json_reports_declared_report_ids() {
    let all = sorted();
    let control = all.iter().find(|c| c.usage_page == 0xFF14).unwrap();
    let out = render_inspect(control, &Redactor::new(false), true);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    let items = v["report_items"].as_array().unwrap();
    assert!(items
        .iter()
        .any(|i| i["report_id"] == 2 && i["kind"] == "output"));
}
