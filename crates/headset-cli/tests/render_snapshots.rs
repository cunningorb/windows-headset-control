use headset_cli::redact::Redactor;
use headset_cli::render::{render_inspect, render_list};
use headset_device::{
    rank_candidates, stable_sort_collections, CollectionInfo, FakeHidBackend, HidBackend,
};

const FIXTURE: &str = include_str!("../../headset-device/tests/fixtures/blackshark-v3-pro-ps.json");
const FIXTURE_WITH_INTERLOPER: &str =
    include_str!("../../headset-device/tests/fixtures/blackshark-plus-interloper.json");

fn sorted() -> Vec<CollectionInfo> {
    let mut all = FakeHidBackend::from_fixture_str(FIXTURE)
        .unwrap()
        .enumerate()
        .unwrap();
    stable_sort_collections(&mut all);
    all
}

/// All absolute indices into `all`, in order — the "no filter applied" case.
fn all_indices(all: &[CollectionInfo]) -> Vec<usize> {
    (0..all.len()).collect()
}

#[test]
fn list_human_output_is_stable() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let shown = all_indices(&all);
    let out = render_list(&all, &ranked, &shown, &Redactor::new(false), false);
    insta::assert_snapshot!("list_human", out);
}

#[test]
fn list_json_output_is_stable() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let shown = all_indices(&all);
    let out = render_list(&all, &ranked, &shown, &Redactor::new(false), true);
    insta::assert_snapshot!("list_json", out);
}

#[test]
fn list_json_has_the_documented_envelope() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let shown = all_indices(&all);
    let out = render_list(&all, &ranked, &shown, &Redactor::new(false), true);
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
fn list_index_survives_filtering() {
    // The index labelling each row must be its absolute index into the full
    // sorted enumeration, not its position within the filtered/shown subset —
    // otherwise the index printed by `list` cannot be fed back into
    // `inspect --path-index` unchanged.
    let all = sorted();
    let ranked = rank_candidates(&all);
    let control_index = all.iter().position(|c| c.usage_page == 0xFF14).unwrap();

    // Filter down to just the control collection, as `--vendor-id`/`--product-id` would.
    let shown = vec![control_index];
    let out = render_list(&all, &ranked, &shown, &Redactor::new(false), true);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let collections = v["collections"].as_array().unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0]["index"], control_index);
    assert_eq!(v["best_candidate_index"], control_index);
}

#[test]
fn no_output_contains_a_raw_path_by_default() {
    let all = sorted();
    let ranked = rank_candidates(&all);
    let shown = all_indices(&all);
    for json in [false, true] {
        let out = render_list(&all, &ranked, &shown, &Redactor::new(false), json);
        assert!(!out.contains("fixture"), "raw path fragment leaked");
        assert!(
            !out.to_lowercase().contains("\\\\?\\hid#"),
            "raw path leaked"
        );
    }
}

#[test]
fn list_never_names_an_unsupported_device_as_the_control_candidate() {
    // blackshark-plus-interloper.json adds a fifth, unrelated vendor (0x1770)
    // collection that outscores every headset collection under
    // `rank_candidates`'s shape-only formula. `list` must scope its "Best
    // control candidate" determination to `is_supported_device`, exactly as
    // `probe` already does, so it never steers an operator toward
    // `probe --candidate <the interloper's index>`.
    let mut all: Vec<CollectionInfo> = FakeHidBackend::from_fixture_str(FIXTURE_WITH_INTERLOPER)
        .unwrap()
        .enumerate()
        .unwrap();
    stable_sort_collections(&mut all);
    let ranked = rank_candidates(&all);
    let interloper_index = all.iter().position(|c| c.vendor_id == 0x1770).unwrap();
    let headset_index = all.iter().position(|c| c.usage_page == 0xFF14).unwrap();
    let shown = all_indices(&all);

    for json in [false, true] {
        let out = render_list(&all, &ranked, &shown, &Redactor::new(false), json);
        if json {
            let v: serde_json::Value = serde_json::from_str(&out).unwrap();
            assert_eq!(
                v["best_candidate_index"], headset_index,
                "json={json}: best_candidate_index must be the headset, not the interloper"
            );
        } else {
            let interloper_line = format!("Best control candidate: index {interloper_index}");
            assert!(
                !out.contains(&interloper_line),
                "json={json}: interloper must never be named the control candidate:\n{out}"
            );
            let headset_line = format!("Best control candidate: index {headset_index}");
            assert!(
                out.contains(&headset_line),
                "json={json}: expected the headset's 0xFF14 collection to be named:\n{out}"
            );
        }
    }
}

#[test]
fn inspect_human_output_is_stable() {
    let all = sorted();
    let index = all.iter().position(|c| c.usage_page == 0xFF14).unwrap();
    let control = &all[index];
    let out = render_inspect(index, control, None, &Redactor::new(false), false);
    assert!(!out.contains("fixture"), "raw path fragment leaked");
    assert!(
        !out.to_lowercase().contains("\\\\?\\hid#"),
        "raw path leaked"
    );
    insta::assert_snapshot!("inspect_human", out);
}

#[test]
fn inspect_json_reports_declared_report_ids() {
    let all = sorted();
    let index = all.iter().position(|c| c.usage_page == 0xFF14).unwrap();
    let control = &all[index];
    let out = render_inspect(index, control, None, &Redactor::new(false), true);
    assert!(!out.contains("fixture"), "raw path fragment leaked");
    assert!(
        !out.to_lowercase().contains("\\\\?\\hid#"),
        "raw path leaked"
    );
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["index"], index);
    let items = v["report_items"].as_array().unwrap();
    assert!(items
        .iter()
        .any(|i| i["report_id"] == 2 && i["kind"] == "output"));
}

#[test]
fn inspect_json_score_matches_what_list_reports_for_the_same_index() {
    // For the same collection at the same schema_version, `list --json` and
    // `inspect --json` must not contradict each other: previously `inspect`
    // always reported `"score": null, "reasons": []` because `render_inspect`
    // passed `None` for the `Candidate` regardless of what `list` computed.
    let all = sorted();
    let ranked = rank_candidates(&all);
    let index = all.iter().position(|c| c.usage_page == 0xFF14).unwrap();
    let control = &all[index];

    let list_out = render_list(
        &all,
        &ranked,
        &all_indices(&all),
        &Redactor::new(false),
        true,
    );
    let list_v: serde_json::Value = serde_json::from_str(&list_out).unwrap();
    let list_score = list_v["collections"][index]["score"].clone();
    assert_ne!(
        list_score,
        serde_json::Value::Null,
        "sanity: list reports a real score"
    );

    let cand = ranked.iter().find(|c| c.index == index);
    let inspect_out = render_inspect(index, control, cand, &Redactor::new(false), true);
    let inspect_v: serde_json::Value = serde_json::from_str(&inspect_out).unwrap();

    assert_eq!(inspect_v["score"], list_score);
    assert_ne!(inspect_v["reasons"], serde_json::json!([]));
}
