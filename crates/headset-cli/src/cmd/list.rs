use anyhow::Result;
use headset_device::{rank_candidates, stable_sort_collections, CollectionInfo, HidBackend};

use crate::cli::ListArgs;
use crate::redact::Redactor;
use crate::render;

/// Enumerates and filters. Sends nothing; opens nothing for I/O.
///
/// Indices are assigned over the full sorted enumeration *before* filtering,
/// so an index reported here always resolves to the same collection when
/// passed to `headsetctl inspect --path-index`. Filtering only selects which
/// rows are shown; it never renumbers them.
pub fn run(
    backend: &dyn HidBackend,
    args: &ListArgs,
    r: &Redactor,
    as_json: bool,
) -> Result<String> {
    let mut all: Vec<CollectionInfo> = backend.enumerate()?;
    stable_sort_collections(&mut all);
    let ranked = rank_candidates(&all);

    let shown: Vec<usize> = all
        .iter()
        .enumerate()
        .filter(|(_, c)| args.vendor_id.is_none_or(|v| c.vendor_id == v))
        .filter(|(_, c)| args.product_id.is_none_or(|p| c.product_id == p))
        .map(|(i, _)| i)
        .collect();

    Ok(render::render_list(&all, &ranked, &shown, r, as_json))
}
