use anyhow::{bail, Result};
use headset_device::{stable_sort_collections, CollectionInfo, HidBackend, OpenMode};

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
            "index {} is out of range; `headsetctl list` reported {} collections",
            args.path_index,
            all.len()
        );
    };

    // Prove the handle can be acquired and released without I/O rights.
    // Failure here is informative, not fatal to reporting descriptors.
    match backend.open(&c.id, OpenMode::Descriptors) {
        Ok(_handle) => tracing::debug!("descriptor handle acquired and released"),
        Err(e) => tracing::warn!("descriptor open failed: {e}"),
    }

    Ok(render::render_inspect(c, r, as_json))
}
