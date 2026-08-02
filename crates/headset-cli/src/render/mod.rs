pub mod human;
pub mod json;

use headset_device::{Candidate, CollectionInfo};

use crate::redact::Redactor;

/// `shown` holds absolute indices into `all`/`ranked` to render, in display
/// order. Indices are never recomputed from `shown`'s length or position:
/// the value shown for each row is its absolute index, so it always means
/// the same thing to `headsetctl inspect --path-index`.
pub fn render_list(
    all: &[CollectionInfo],
    ranked: &[Candidate],
    shown: &[usize],
    r: &Redactor,
    as_json: bool,
) -> String {
    if as_json {
        json::render_list(all, ranked, shown, r)
    } else {
        human::render_list(all, ranked, shown, r)
    }
}

pub fn render_inspect(
    index: usize,
    c: &CollectionInfo,
    cand: Option<&Candidate>,
    r: &Redactor,
    as_json: bool,
) -> String {
    if as_json {
        json::render_inspect(index, c, cand, r)
    } else {
        human::render_inspect(c, r)
    }
}
