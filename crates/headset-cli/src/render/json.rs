use headset_device::{is_supported_device, Candidate, CollectionInfo};
use serde_json::{json, Value};

use crate::redact::Redactor;

/// Bump when a field is removed or its meaning changes. Adding a field does not
/// require a bump; consumers must tolerate unknown fields.
pub const SCHEMA_VERSION: u32 = 1;

pub fn collection_value(
    index: usize,
    c: &CollectionInfo,
    cand: Option<&Candidate>,
    r: &Redactor,
) -> Value {
    json!({
        "index": index,
        "vendor_id": format!("{:#06x}", c.vendor_id),
        "product_id": format!("{:#06x}", c.product_id),
        "version": format!("{:#06x}", c.version),
        "interface_number": c.interface_number,
        "collection_number": c.collection_number,
        "usage_page": format!("{:#06x}", c.usage_page),
        "usage": format!("{:#06x}", c.usage),
        "input_report_len": c.input_report_len,
        "output_report_len": c.output_report_len,
        "feature_report_len": c.feature_report_len,
        "product": c.product,
        "manufacturer": c.manufacturer,
        "serial": r.serial(c.has_serial),
        "path": r.path(&c.id),
        "supported_device": is_supported_device(c),
        "score": cand.map(|x| x.score),
        "disqualified": cand.and_then(|x| x.disqualified.clone()),
        "reasons": cand.map(|x| x.reasons.clone()).unwrap_or_default(),
    })
}

pub fn render_list(
    all: &[CollectionInfo],
    ranked: &[Candidate],
    shown: &[usize],
    r: &Redactor,
) -> String {
    let by_index: std::collections::HashMap<usize, &Candidate> =
        ranked.iter().map(|c| (c.index, c)).collect();
    let collections: Vec<Value> = shown
        .iter()
        .map(|&i| collection_value(i, &all[i], by_index.get(&i).copied(), r))
        .collect();

    let shown_ranked: Vec<Candidate> = ranked
        .iter()
        .filter(|c| shown.contains(&c.index))
        .cloned()
        .collect();

    // Automatic selection is scoped to devices this project actually
    // supports (see `human::render_list` for the rationale); `collections`
    // above still lists every collection `list` saw, each carrying its own
    // `supported_device` flag.
    let supported_ranked: Vec<Candidate> = shown_ranked
        .iter()
        .filter(|c| is_supported_device(&all[c.index]))
        .cloned()
        .collect();
    let best = supported_ranked
        .iter()
        .find(|c| c.disqualified.is_none())
        .filter(|_| headset_device::has_unambiguous_winner(&supported_ranked))
        .map(|c| c.index);

    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "include_sensitive": r.include_sensitive(),
        "collections": collections,
        "best_candidate_index": best,
    }))
    .expect("serialization cannot fail")
}

pub fn render_inspect(
    index: usize,
    c: &CollectionInfo,
    cand: Option<&Candidate>,
    r: &Redactor,
) -> String {
    let items: Vec<Value> = c
        .report_items
        .iter()
        .map(|i| {
            json!({
                "kind": i.kind,
                "report_id": i.report_id,
                "usage_page": format!("{:#06x}", i.usage_page),
                "usage_min": format!("{:#06x}", i.usage_min),
                "usage_max": format!("{:#06x}", i.usage_max),
                "bit_size": i.bit_size,
                "report_count": i.report_count,
                "is_button": i.is_button,
            })
        })
        .collect();

    let mut root = collection_value(index, c, cand, r);
    root["schema_version"] = json!(SCHEMA_VERSION);
    root["report_items"] = json!(items);
    root["opened_for_io"] = json!(false);
    serde_json::to_string_pretty(&root).expect("serialization cannot fail")
}
