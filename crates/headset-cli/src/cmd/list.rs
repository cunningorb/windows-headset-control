use anyhow::Result;
use headset_device::{rank_candidates, stable_sort_collections, CollectionInfo, HidBackend};

use crate::cli::ListArgs;
use crate::redact::Redactor;
use crate::render;

/// Enumerates and filters. Sends nothing; opens nothing for I/O.
pub fn run(
    backend: &dyn HidBackend,
    args: &ListArgs,
    r: &Redactor,
    as_json: bool,
) -> Result<String> {
    let mut all: Vec<CollectionInfo> = backend.enumerate()?;
    if let Some(vid) = args.vendor_id {
        all.retain(|c| c.vendor_id == vid);
    }
    if let Some(pid) = args.product_id {
        all.retain(|c| c.product_id == pid);
    }
    stable_sort_collections(&mut all);
    let ranked = rank_candidates(&all);
    Ok(render::render_list(&all, &ranked, r, as_json))
}
