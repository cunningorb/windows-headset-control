//! `get`, `set`, `param`, and `watch`.
//!
//! Everything here goes through `ControlSession`, so device resolution and
//! request/response correlation are solved once, below this layer.

use std::time::Duration;

use anyhow::{bail, Result};
use headset_device::{ControlSession, HidBackend};
use headset_protocol::{
    encode_read, encode_write, Param, ParamFrame, ResultCode, INDEXED_READS, READ_ALLOWLIST,
    WRITE_ALLOWLIST,
};
use serde_json::json;

use crate::cli::{GetArgs, ParamAction, ParamArgs, SetArgs, WatchArgs};
use crate::render::json::SCHEMA_VERSION;

/// Sidetone writes are preceded by `0x18 = 01`, which is what the vendor
/// software was observed doing before every level write. Reproducing the
/// observed sequence is the only behaviour we can defend; sending the level
/// alone was never seen on the wire.
const SIDETONE_ENABLE: u8 = 0x18;

fn hex_list(ids: &[u8]) -> String {
    ids.iter()
        .map(|i| format!("{i:#04x}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn lookup(name: &str) -> Result<Param> {
    Param::from_name(name).ok_or_else(|| {
        let names = Param::ALL
            .iter()
            .map(|p| p.name())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::anyhow!("unknown parameter `{name}`; known parameters are: {names}")
    })
}

/// Whether a read came back refused.
///
/// The dongle answers a read of a headset-proxied parameter with the same
/// `0xff` it uses to refuse a write when the headset is not reachable. Observed
/// directly: with the headset powered off, `link` reported `00 00` while every
/// other parameter returned a lone `0xff` in the same session.
///
/// Only applied to named parameters. The raw `param get` path prints whatever
/// byte arrived, because an unidentified parameter might legitimately hold
/// `0xff` and this project does not guess.
fn looks_refused(param: Param, frame: &ParamFrame) -> bool {
    param != Param::LinkState && frame.value() == Some(0xFF)
}

/// `true` when the headset is reachable over the wireless link.
fn link_is_up(frame: &ParamFrame) -> bool {
    frame.payload.first() == Some(&0x01)
}

pub fn run_get(backend: &dyn HidBackend, args: &GetArgs, as_json: bool) -> Result<String> {
    let param = lookup(&args.name)?;
    let mut session = ControlSession::open(backend)?;
    let request = encode_read(param.id(), None)?;
    let frame = session.exchange(&request, param.id(), false)?;

    // Only pay for the extra exchange when something is actually wrong: a
    // healthy read costs one round trip, and the link is consulted purely to
    // explain a refusal.
    let link_up = if looks_refused(param, &frame) {
        let probe = encode_read(Param::LinkState.id(), None)?;
        session
            .exchange(&probe, Param::LinkState.id(), false)
            .ok()
            .map(|f| link_is_up(&f))
    } else {
        None
    };

    let refused = looks_refused(param, &frame);

    Ok(if as_json {
        serde_json::to_string_pretty(&json!({
            "schema_version": SCHEMA_VERSION,
            "parameter": param.name(),
            "id": format!("{:#04x}", param.id()),
            // Null rather than 255 when refused: 255 is not a battery level,
            // and emitting it would put a wrong number into a machine-readable
            // contract.
            "value": if refused { None } else { frame.value() },
            "available": !refused,
            "headset_connected": link_up,
            "payload_hex": frame.hex_payload(),
            "wrote_to_device": true,
        }))
        .expect("serialization cannot fail")
    } else if refused {
        match link_up {
            Some(false) => format!(
                "{}: unavailable - the headset is powered off or out of range\n",
                param.name()
            ),
            _ => format!(
                "{}: unavailable - the device returned the refusal byte 0xff\n",
                param.name()
            ),
        }
    } else {
        match frame.value() {
            Some(v) => format!("{}: {v}\n", param.name()),
            // Multi-byte parameters such as link state have no single value.
            None if param == Param::LinkState => format!(
                "{}: {}\n",
                param.name(),
                if link_is_up(&frame) {
                    "connected"
                } else {
                    "not connected"
                }
            ),
            None => format!("{}: {}\n", param.name(), frame.hex_payload()),
        }
    })
}

pub fn run_set(backend: &dyn HidBackend, args: &SetArgs, as_json: bool) -> Result<String> {
    let param = lookup(&args.name)?;
    if !param.is_writable() {
        bail!(
            "`{}` is read-only: no write for it was ever observed on the wire, so this \
             project has no command to send",
            param.name()
        );
    }
    if let Some((lo, hi)) = param.range() {
        if args.value < lo || args.value > hi {
            bail!(
                "{} accepts {lo}..={hi}; {} is outside the range observed on this hardware",
                param.name(),
                args.value
            );
        }
    }

    let mut session = ControlSession::open(backend)?;

    if param == Param::Sidetone {
        let enable = encode_write(SIDETONE_ENABLE, 1)?;
        let ack = session.exchange(&enable, SIDETONE_ENABLE, true)?;
        check_result(&ack, "sidetone enable")?;
    }

    let request = encode_write(param.id(), args.value)?;
    let ack = session.exchange(&request, param.id(), true)?;
    check_result(&ack, param.name())?;

    // Device is the source of truth: a successful write is not evidence that
    // the value changed. Read it back and report what the device actually holds.
    let verify = encode_read(param.id(), None)?;
    let now = session.exchange(&verify, param.id(), false)?;
    let readback = now.value();

    Ok(if as_json {
        serde_json::to_string_pretty(&json!({
            "schema_version": SCHEMA_VERSION,
            "parameter": param.name(),
            "id": format!("{:#04x}", param.id()),
            "requested": args.value,
            "readback": readback,
            "applied": readback == Some(args.value),
            "wrote_to_device": true,
        }))
        .expect("serialization cannot fail")
    } else {
        match readback {
            Some(v) if v == args.value => format!("{}: {v}\n", param.name()),
            Some(v) => format!(
                "{}: wrote {} but the device reports {v}\n",
                param.name(),
                args.value
            ),
            None => format!(
                "{}: wrote {}; read-back returned {}\n",
                param.name(),
                args.value,
                now.hex_payload()
            ),
        }
    })
}

fn check_result(ack: &ParamFrame, what: &str) -> Result<()> {
    match ack.result() {
        Some(ResultCode::Ok) | None => Ok(()),
        Some(ResultCode::Refused) => bail!(
            "the device refused the {what} write (result 0xff). This was observed when the \
             microphone is muted by the headset's own switch; unmuting it and retrying is \
             the first thing to try."
        ),
        Some(ResultCode::Unknown(b)) => bail!(
            "the device answered the {what} write with an unrecognised result {b:#04x}; \
             only 0x00 and 0xff have been observed, so this is not being treated as success"
        ),
    }
}

pub fn run_param(backend: &dyn HidBackend, args: &ParamArgs, as_json: bool) -> Result<String> {
    match &args.action {
        ParamAction::Get { id, index } => {
            if !READ_ALLOWLIST.contains(id) {
                bail!(
                    "{id:#04x} is not in the observed read allowlist. Only identifiers seen \
                     on the wire may be sent. Allowed: {}",
                    hex_list(&READ_ALLOWLIST)
                );
            }
            if index.is_some() && !INDEXED_READS.contains(id) {
                bail!(
                    "{id:#04x} was not observed taking an index; indexed reads are: {}",
                    hex_list(&INDEXED_READS)
                );
            }
            let mut session = ControlSession::open(backend)?;
            let request = encode_read(*id, *index)?;
            let frame = session.exchange(&request, *id, false)?;
            Ok(render_raw(*id, false, &frame, as_json))
        }
        ParamAction::Set { id, value } => {
            if !WRITE_ALLOWLIST.contains(id) {
                bail!(
                    "{id:#04x} is not in the observed write allowlist. Only identifiers seen \
                     being written may be sent. Allowed: {}",
                    hex_list(&WRITE_ALLOWLIST)
                );
            }
            let mut session = ControlSession::open(backend)?;
            let request = encode_write(*id, *value)?;
            let frame = session.exchange(&request, *id, true)?;
            Ok(render_raw(*id, true, &frame, as_json))
        }
    }
}

fn render_raw(id: u8, is_write: bool, frame: &ParamFrame, as_json: bool) -> String {
    if as_json {
        serde_json::to_string_pretty(&json!({
            "schema_version": SCHEMA_VERSION,
            "id": format!("{id:#04x}"),
            "write": is_write,
            "payload_hex": frame.hex_payload(),
            "value": frame.value(),
            // Deliberately absent: any interpretation of these bytes. Eleven
            // observed parameters have no established meaning, and inventing
            // one here would put it into a machine-readable contract.
            "interpretation": serde_json::Value::Null,
            "wrote_to_device": true,
        }))
        .expect("serialization cannot fail")
    } else {
        format!("{id:#04x}: {}\n", frame.hex_payload())
    }
}

pub fn run_watch(backend: &dyn HidBackend, args: &WatchArgs, as_json: bool) -> Result<String> {
    let mut session = ControlSession::open_read_only(backend)?;
    let events = session.listen(Duration::from_secs(args.seconds.clamp(1, 3600)))?;

    Ok(if as_json {
        let items: Vec<serde_json::Value> = events
            .iter()
            .map(|e| {
                json!({
                    "id": format!("{:#04x}", e.param),
                    "name": name_of(e.param),
                    "value": e.value(),
                    "payload_hex": e.hex_payload(),
                })
            })
            .collect();
        serde_json::to_string_pretty(&json!({
            "schema_version": SCHEMA_VERSION,
            "seconds": args.seconds,
            "events": items,
            "wrote_to_device": false,
        }))
        .expect("serialization cannot fail")
    } else if events.is_empty() {
        "no events. The device pushes only when something changes - move the onboard \
         wheel or toggle the mic mute to produce one.\n"
            .to_string()
    } else {
        let mut s = String::new();
        for e in &events {
            use std::fmt::Write as _;
            let label = name_of(e.param).unwrap_or("unidentified");
            let _ = writeln!(
                s,
                "{:#04x} {:<16} {}",
                e.param,
                label,
                e.value().map_or_else(|| e.hex_payload(), |v| v.to_string())
            );
        }
        s
    })
}

/// The name of a parameter, or `None` when its meaning is not established.
fn name_of(id: u8) -> Option<&'static str> {
    Param::ALL.iter().find(|p| p.id() == id).map(|p| p.name())
}
