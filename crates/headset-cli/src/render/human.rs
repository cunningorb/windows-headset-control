use std::fmt::Write as _;

use headset_device::{is_supported_device, Candidate, CollectionInfo, ReportKind};

use crate::redact::Redactor;

pub fn render_list(
    all: &[CollectionInfo],
    ranked: &[Candidate],
    shown: &[usize],
    r: &Redactor,
) -> String {
    let mut s = String::new();
    if let Some(w) = r.warning_banner() {
        let _ = writeln!(s, "{w}\n");
    }

    if shown.is_empty() {
        let _ = writeln!(s, "No HID collections found.");
        return s;
    }

    let by_index: std::collections::HashMap<usize, &Candidate> =
        ranked.iter().map(|c| (c.index, c)).collect();

    for &i in shown {
        let c = &all[i];
        let _ = writeln!(
            s,
            "[{i}] {}",
            c.product.as_deref().unwrap_or("<no product string>")
        );
        let _ = writeln!(
            s,
            "     vendor/product : {:#06x} / {:#06x}",
            c.vendor_id, c.product_id
        );
        let _ = writeln!(
            s,
            "     interface/coll : {} / {}",
            c.interface_number
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into()),
            c.collection_number
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into())
        );
        let _ = writeln!(
            s,
            "     usage page/usg : {:#06x} / {:#06x}",
            c.usage_page, c.usage
        );
        let _ = writeln!(
            s,
            "     reports in/out/feat : {} / {} / {}",
            c.input_report_len, c.output_report_len, c.feature_report_len
        );
        let _ = writeln!(s, "     serial         : {}", r.serial(c.has_serial));
        let _ = writeln!(s, "     path           : {}", r.path(&c.id));
        match by_index.get(&i) {
            Some(cand) if cand.disqualified.is_none() => {
                let _ = writeln!(
                    s,
                    "     candidate      : score {} ({})",
                    cand.score,
                    cand.reasons.join("; ")
                );
            }
            Some(cand) => {
                let _ = writeln!(
                    s,
                    "     candidate      : excluded - {}",
                    cand.disqualified.as_deref().unwrap_or("unknown")
                );
            }
            None => {}
        }
        let _ = writeln!(s);
    }

    let shown_ranked: Vec<Candidate> = ranked
        .iter()
        .filter(|c| shown.contains(&c.index))
        .cloned()
        .collect();

    // Automatic selection must be scoped to devices this project actually
    // supports: shape-based ranking alone will happily pick an unrelated
    // vendor's HID collection off the same machine (see
    // `headset_device::select::is_supported_device`). `list` still shows
    // every collection it saw, but only ever names a *supported* one as the
    // control candidate.
    let supported_ranked: Vec<Candidate> = shown_ranked
        .iter()
        .filter(|c| is_supported_device(&all[c.index]))
        .cloned()
        .collect();
    let best_supported = headset_device::has_unambiguous_winner(&supported_ranked)
        .then(|| supported_ranked.iter().find(|c| c.disqualified.is_none()))
        .flatten();

    match best_supported {
        Some(best) => {
            let _ = writeln!(s, "Best control candidate: index {}", best.index);
        }
        None => {
            let _ = writeln!(
                s,
                "No unambiguous control candidate. Pass --candidate <index> explicitly."
            );
        }
    }

    // The top-scoring collection overall may belong to a device this project
    // does not support. Report it as a separate, clearly-labelled fact rather
    // than hiding it: `list` is a diagnostic and should still show what it saw.
    if let Some(top) = shown_ranked.iter().find(|c| c.disqualified.is_none()) {
        if !is_supported_device(&all[top.index]) {
            let _ = writeln!(
                s,
                "Highest-scoring vendor collection: index {} (not a device this project supports)",
                top.index
            );
        }
    }
    s
}

pub fn render_inspect(c: &CollectionInfo, r: &Redactor) -> String {
    let mut s = String::new();
    if let Some(w) = r.warning_banner() {
        let _ = writeln!(s, "{w}\n");
    }
    let _ = writeln!(
        s,
        "{}",
        c.product.as_deref().unwrap_or("<no product string>")
    );
    let _ = writeln!(
        s,
        "  manufacturer   : {}",
        c.manufacturer.as_deref().unwrap_or("-")
    );
    let _ = writeln!(
        s,
        "  vendor/product : {:#06x} / {:#06x}",
        c.vendor_id, c.product_id
    );
    let _ = writeln!(s, "  version        : {:#06x}", c.version);
    let _ = writeln!(
        s,
        "  usage page/usg : {:#06x} / {:#06x}",
        c.usage_page, c.usage
    );
    let _ = writeln!(
        s,
        "  reports in/out/feat : {} / {} / {}",
        c.input_report_len, c.output_report_len, c.feature_report_len
    );
    let _ = writeln!(s, "  serial         : {}", r.serial(c.has_serial));
    let _ = writeln!(s, "  path           : {}", r.path(&c.id));
    let _ = writeln!(s, "  opened for I/O : no (descriptor access only)");

    for kind in [ReportKind::Input, ReportKind::Output, ReportKind::Feature] {
        let ids = c.report_ids(kind);
        let _ = writeln!(
            s,
            "  {:?} report ids : {}",
            kind,
            if ids.is_empty() {
                "-".to_string()
            } else {
                ids.iter()
                    .map(|i| format!("{i:#04x}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );
    }

    if !c.report_items.is_empty() {
        let _ = writeln!(s, "  declared items :");
        for i in &c.report_items {
            let _ = writeln!(
                s,
                "    {:?}{} id={:#04x} page={:#06x} usage={:#06x}..{:#06x} bits={} count={}",
                i.kind,
                if i.is_button { " button" } else { " value " },
                i.report_id,
                i.usage_page,
                i.usage_min,
                i.usage_max,
                i.bit_size,
                i.report_count
            );
        }
    }
    s
}
