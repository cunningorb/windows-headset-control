use std::time::Duration;

use anyhow::{bail, Result};
use headset_device::{
    has_unambiguous_winner, rank_candidates, stable_sort_collections, CollectionInfo, DeviceError,
    HidBackend, OpenMode,
};
use headset_protocol::ControlFrame;
use serde_json::json;

use crate::cli::ProbeArgs;
use crate::redact::Redactor;
use crate::render::json::SCHEMA_VERSION;

/// The allowlist of operations `probe` may perform.
///
/// Exactly one variant exists, and it performs no write. Adding a variant that
/// writes requires a documented, reviewed decision. See `docs/device-research.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeOp {
    /// Open the control collection for reading and listen for an unsolicited
    /// input report. Sends nothing to the device.
    PassiveListen,
}

/// Minimum spacing between repeated device requests, enforced even though the
/// current allowlist contains no request-emitting operation.
pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(250);

pub fn run(
    backend: &dyn HidBackend,
    args: &ProbeArgs,
    r: &Redactor,
    as_json: bool,
) -> Result<String> {
    let op = ProbeOp::PassiveListen;

    let mut all: Vec<CollectionInfo> = backend.enumerate()?;
    if all.is_empty() {
        bail!("no HID collections found; is the dongle connected?");
    }
    stable_sort_collections(&mut all);
    let ranked = rank_candidates(&all);

    let index = match args.candidate {
        Some(i) => i,
        None => {
            if !has_unambiguous_winner(&ranked) {
                bail!(
                    "no unambiguous control candidate; re-run with --candidate <index> \
                     after reviewing `headsetctl list`"
                );
            }
            ranked
                .iter()
                .find(|c| c.disqualified.is_none())
                .expect("a winner exists")
                .index
        }
    };

    let Some(target) = all.get(index) else {
        bail!(
            "candidate index {index} is out of range; {} collections enumerated",
            all.len()
        );
    };

    if let Some(entry) = ranked.iter().find(|c| c.index == index) {
        if let Some(reason) = &entry.disqualified {
            bail!("candidate {index} is disqualified: {reason}");
        }
    }
    if target.is_audio_stack_collection() {
        bail!("refusing to open the Windows audio collection");
    }

    // Bound the listen window so a silent device cannot hang the process.
    let timeout = Duration::from_millis(args.listen_ms.clamp(100, 30_000));

    let transport = backend.open(&target.id, OpenMode::ReadWrite)?;
    let declared = transport.input_report_len() as usize;
    if declared == 0 || declared > 1024 {
        bail!("collection declares an implausible input report length of {declared}");
    }
    let mut buf = vec![0u8; declared];

    let outcome = match transport.read_report(&mut buf, timeout) {
        Ok(n) => match ControlFrame::parse(&buf[..n]) {
            Ok(frame) => Outcome::Frame {
                bytes: n,
                hex: frame.hex_payload(),
            },
            Err(e) => Outcome::Malformed {
                bytes: n,
                error: e.to_string(),
            },
        },
        Err(DeviceError::Timeout(_)) => Outcome::Silent,
        Err(e) => return Err(e.into()),
    };
    drop(transport); // release the handle before rendering

    Ok(if as_json {
        render_json(op, index, target, &outcome, r, timeout)
    } else {
        render_human(op, index, target, &outcome, r, timeout)
    })
}

enum Outcome {
    Frame { bytes: usize, hex: String },
    Malformed { bytes: usize, error: String },
    Silent,
}

fn render_json(
    op: ProbeOp,
    index: usize,
    c: &CollectionInfo,
    outcome: &Outcome,
    r: &Redactor,
    timeout: Duration,
) -> String {
    let result = match outcome {
        Outcome::Frame { bytes, hex } => json!({
            "status": "frame_received",
            "bytes": bytes,
            "payload_hex": hex,
            "interpreted_fields": {},
            "note": "no payload semantics are established for this hardware"
        }),
        Outcome::Malformed { bytes, error } => json!({
            "status": "unexpected_frame", "bytes": bytes, "error": error
        }),
        Outcome::Silent => json!({
            "status": "silent",
            "note": "no unsolicited input report within the listen window"
        }),
    };

    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "operation": format!("{op:?}"),
        "wrote_to_device": false,
        "candidate_index": index,
        "usage_page": format!("{:#06x}", c.usage_page),
        "input_report_len": c.input_report_len,
        "path": r.path(&c.id),
        "listen_ms": timeout.as_millis(),
        "result": result,
    }))
    .expect("serialization cannot fail")
}

fn render_human(
    op: ProbeOp,
    index: usize,
    c: &CollectionInfo,
    outcome: &Outcome,
    r: &Redactor,
    timeout: Duration,
) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    if let Some(w) = r.warning_banner() {
        let _ = writeln!(s, "{w}\n");
    }
    let _ = writeln!(s, "probe operation : {op:?}");
    let _ = writeln!(s, "wrote to device : no");
    let _ = writeln!(
        s,
        "candidate       : [{index}] usage page {:#06x}",
        c.usage_page
    );
    let _ = writeln!(s, "path            : {}", r.path(&c.id));
    let _ = writeln!(s, "listen window   : {} ms", timeout.as_millis());
    match outcome {
        Outcome::Frame { bytes, hex } => {
            let _ = writeln!(s, "result          : received {bytes} bytes");
            let _ = writeln!(s, "payload         : {hex}");
            let _ = writeln!(
                s,
                "interpretation  : none - no payload semantics established"
            );
        }
        Outcome::Malformed { bytes, error } => {
            let _ = writeln!(s, "result          : {bytes} bytes, failed validation");
            let _ = writeln!(s, "error           : {error}");
        }
        Outcome::Silent => {
            let _ = writeln!(s, "result          : silent (no unsolicited report)");
            let _ = writeln!(
                s,
                "note            : this is expected. The device may only emit reports in\n\
                 \x20                 response to a request, and no request is known to be safe\n\
                 \x20                 on this hardware yet. See docs/device-research.md."
            );
        }
    }
    s
}
