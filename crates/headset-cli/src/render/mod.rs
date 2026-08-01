pub mod human;
pub mod json;

use headset_device::{Candidate, CollectionInfo};

use crate::redact::Redactor;

pub fn render_list(
    all: &[CollectionInfo],
    ranked: &[Candidate],
    r: &Redactor,
    as_json: bool,
) -> String {
    if as_json {
        json::render_list(all, ranked, r)
    } else {
        human::render_list(all, ranked, r)
    }
}

pub fn render_inspect(c: &CollectionInfo, r: &Redactor, as_json: bool) -> String {
    if as_json {
        json::render_inspect(c, r)
    } else {
        human::render_inspect(c, r)
    }
}
