use anyhow::{bail, Result};
use headset_device::{
    rank_candidates, stable_sort_collections, CollectionInfo, HidBackend, OpenMode,
};

use crate::cli::InspectArgs;
use crate::redact::Redactor;
use crate::render;

/// Opens the selected collection in `Descriptors` mode and closes it. Never writes.
pub fn run(
    backend: &dyn HidBackend,
    args: &InspectArgs,
    r: &Redactor,
    as_json: bool,
) -> Result<String> {
    let mut all: Vec<CollectionInfo> = backend.enumerate()?;
    stable_sort_collections(&mut all);

    let Some(c) = all.get(args.path_index) else {
        bail!(
            "index {} is out of range; the full enumeration contains {} collections \
             (indices 0..{})",
            args.path_index,
            all.len(),
            all.len().saturating_sub(1)
        );
    };

    // Prove the handle can be acquired and released without I/O rights.
    // Failure here is informative, not fatal to reporting descriptors.
    match backend.open(&c.id, OpenMode::Descriptors) {
        Ok(_handle) => tracing::debug!("descriptor handle acquired and released"),
        Err(e) => tracing::warn!("descriptor open failed: {e}"),
    }

    // Compute the same ranking `list` computes, so `inspect --json` reports
    // the same score/reasons for this index rather than silently contradicting
    // `list --json` for the same collection (see Task 11 fix-round finding).
    let ranked = rank_candidates(&all);
    let cand = ranked.iter().find(|cand| cand.index == args.path_index);

    Ok(render::render_inspect(args.path_index, c, cand, r, as_json))
}
