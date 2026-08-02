//! Device resolution and request/response correlation for the control channel.
//!
//! Two problems live here that no caller should have to solve twice.
//!
//! **Resolution.** A candidate index and a device path are both machine-local:
//! the control collection sits at a different absolute index on a different PC,
//! and the path embeds an instance id that changes when the dongle is replugged.
//! Nothing here persists either. Every session re-resolves by identity.
//!
//! **Correlation.** The device pushes unsolicited events — battery, mute, wheel
//! movement — on the same collection that carries responses. A naive
//! read-one-report would let a battery event satisfy a sidetone read. The
//! session matches on the command bytes and the response role, and routes
//! everything else to an event sink.

use std::time::{Duration, Instant};

use headset_protocol::{parse, ParamFrame, ProtocolError, Role, CONTROL_REPORT_LEN};

use crate::backend::{HidBackend, HidTransport};
use crate::error::DeviceError;
use crate::model::{CollectionInfo, OpenMode};
use crate::select::is_supported_device;

/// Minimum spacing between requests. Reserved in Phase 1; enforced here.
pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(250);

/// How long a single exchange may take, including any events that arrive while
/// waiting. Applies to the exchange as a whole, so a talkative device cannot
/// extend it indefinitely one event at a time.
pub const DEFAULT_EXCHANGE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Per-read timeout inside an exchange. Shorter than the exchange budget so a
/// silent device is noticed without waiting out the whole deadline.
const READ_SLICE: Duration = Duration::from_millis(250);

/// The descriptor shape the protocol in `headset-protocol` was derived from.
///
/// A supported device whose control collection does not match this shape is
/// refused rather than written to on the assumption that our framing applies.
/// The protocol was measured on one PID; a second supported PID could differ.
const CONTROL_USAGE_PAGE: u16 = 0xFF14;

/// Finds the control collection by identity, never by index or stored path.
pub fn resolve_control_device(backend: &dyn HidBackend) -> Result<CollectionInfo, DeviceError> {
    let all = backend.enumerate()?;
    let matches: Vec<&CollectionInfo> = all
        .iter()
        .filter(|c| is_supported_device(c) && c.usage_page == CONTROL_USAGE_PAGE)
        .collect();

    match matches.len() {
        0 => Err(DeviceError::DongleNotFound),
        1 => {
            let c = matches[0];
            check_shape(c)?;
            Ok(c.clone())
        }
        // Two dongles is a real configuration, not a bug. Refuse rather than
        // pick one, because picking silently would write to whichever happened
        // to enumerate first.
        n => Err(DeviceError::AmbiguousDevice(n)),
    }
}

fn check_shape(c: &CollectionInfo) -> Result<(), DeviceError> {
    let mut wrong = Vec::new();
    if c.input_report_len as usize != CONTROL_REPORT_LEN {
        wrong.push(format!(
            "input report {} bytes, expected {CONTROL_REPORT_LEN}",
            c.input_report_len
        ));
    }
    if c.output_report_len as usize != CONTROL_REPORT_LEN {
        wrong.push(format!(
            "output report {} bytes, expected {CONTROL_REPORT_LEN}",
            c.output_report_len
        ));
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(DeviceError::UnexpectedControlShape(wrong.join("; ")))
    }
}

/// An open control channel that can exchange parameter requests.
pub struct ControlSession {
    transport: Box<dyn HidTransport>,
    info: CollectionInfo,
    last_request: Option<Instant>,
    exchange_timeout: Duration,
    /// Events observed while waiting for responses, oldest first.
    pending_events: Vec<ParamFrame>,
}

impl ControlSession {
    /// Resolves the device and opens it for read and write.
    pub fn open(backend: &dyn HidBackend) -> Result<Self, DeviceError> {
        let info = resolve_control_device(backend)?;
        let transport = backend.open(&info.id, OpenMode::ReadWrite)?;
        Ok(Self {
            transport,
            info,
            last_request: None,
            exchange_timeout: DEFAULT_EXCHANGE_TIMEOUT,
            pending_events: Vec::new(),
        })
    }

    /// Opens read-only, for listening without the ability to write.
    pub fn open_read_only(backend: &dyn HidBackend) -> Result<Self, DeviceError> {
        let info = resolve_control_device(backend)?;
        let transport = backend.open(&info.id, OpenMode::Read)?;
        Ok(Self {
            transport,
            info,
            last_request: None,
            exchange_timeout: DEFAULT_EXCHANGE_TIMEOUT,
            pending_events: Vec::new(),
        })
    }

    pub fn with_exchange_timeout(mut self, t: Duration) -> Self {
        self.exchange_timeout = t;
        self
    }

    pub fn info(&self) -> &CollectionInfo {
        &self.info
    }

    /// Events collected while waiting for responses. Draining hands ownership to
    /// the caller so a long-lived session does not accumulate them forever.
    pub fn drain_events(&mut self) -> Vec<ParamFrame> {
        std::mem::take(&mut self.pending_events)
    }

    /// Sends an encoded request and waits for the response that answers it.
    ///
    /// `param` and `is_write` describe the request, and only a frame matching
    /// both — with role `Response` — resolves the wait. Events are collected and
    /// do not satisfy it.
    pub fn exchange(
        &mut self,
        request: &[u8; CONTROL_REPORT_LEN],
        param: u8,
        is_write: bool,
    ) -> Result<ParamFrame, DeviceError> {
        self.pace();
        self.transport.write_report(request)?;
        self.last_request = Some(Instant::now());

        let deadline = Instant::now() + self.exchange_timeout;
        let mut events_seen = 0usize;
        let mut buf = vec![0u8; self.transport.input_report_len() as usize];

        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self
                .transport
                .read_report(&mut buf, READ_SLICE.min(remaining))
            {
                Ok(n) => match parse(&buf[..n]) {
                    Ok(Some(frame)) => {
                        if frame.role == Role::Response && frame.param == param
                            // A write and a read of the same parameter are
                            // distinct commands on the wire, so matching the
                            // parameter alone would let a read response satisfy
                            // a write that was issued alongside it.
                            && frame.is_write == is_write
                        {
                            return Ok(frame);
                        }
                        if frame.role == Role::Event {
                            events_seen += 1;
                            self.pending_events.push(frame);
                        }
                        // A response for something else is dropped: it belongs
                        // to another writer on this collection, not to us.
                    }
                    // A frame outside the parameter family, or one that fails
                    // validation, is not fatal to the exchange -- another
                    // process shares this collection. Keep waiting for ours.
                    Ok(None) => {}
                    Err(ProtocolError::ChecksumMismatch { .. }) => {
                        tracing::debug!("discarding a frame with a bad checksum");
                    }
                    Err(e) => {
                        tracing::debug!("discarding an undecodable frame: {e}");
                    }
                },
                Err(DeviceError::Timeout(_)) => continue,
                Err(e) => return Err(e),
            }
        }

        Err(DeviceError::ProtocolMismatch(format!(
            "no response for parameter {param:#04x} within {:?}; {events_seen} unrelated \
             event(s) arrived while waiting",
            self.exchange_timeout
        )))
    }

    /// Reads input reports until the deadline, collecting decoded events.
    ///
    /// Sends nothing. This is the listen path the tray's reader thread and
    /// `headsetctl watch` share.
    pub fn listen(&mut self, duration: Duration) -> Result<Vec<ParamFrame>, DeviceError> {
        let deadline = Instant::now() + duration;
        let mut out = std::mem::take(&mut self.pending_events);
        let mut buf = vec![0u8; self.transport.input_report_len() as usize];

        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self
                .transport
                .read_report(&mut buf, READ_SLICE.min(remaining))
            {
                Ok(n) => {
                    if let Ok(Some(frame)) = parse(&buf[..n]) {
                        if frame.role == Role::Event {
                            out.push(frame);
                        }
                    }
                }
                Err(DeviceError::Timeout(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Honours `MIN_REQUEST_INTERVAL` between consecutive requests.
    fn pace(&self) {
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed();
            if elapsed < MIN_REQUEST_INTERVAL {
                std::thread::sleep(MIN_REQUEST_INTERVAL - elapsed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeHidBackend;
    use headset_protocol::{checksum, encode_read, Param};

    const FIXTURE: &str = include_str!("../tests/fixtures/blackshark-v3-pro-ps.json");
    const INTERLOPER: &str = include_str!("../tests/fixtures/blackshark-plus-interloper.json");

    fn response(param: u8, is_write: bool, payload: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; CONTROL_REPORT_LEN];
        b[0] = 0x02;
        b[1] = 0x02;
        b[2] = 0x60;
        b[6] = 4 + payload.len() as u8;
        b[9] = 0x80;
        b[10] = if is_write { param | 0x80 } else { param };
        b[11] = 0x01;
        b[12] = payload.len() as u8;
        b[13..13 + payload.len()].copy_from_slice(payload);
        b[62] = checksum(&b);
        b
    }

    fn event(param: u8, payload: &[u8]) -> Vec<u8> {
        let mut b = response(param, false, payload);
        b[9] = 0x00;
        b[11] = 0x02;
        b[62] = 0;
        b[62] = checksum(&b);
        b
    }

    #[test]
    fn resolves_the_control_collection_by_identity() {
        let b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        assert_eq!(info.usage_page, CONTROL_USAGE_PAGE);
        assert_eq!(info.input_report_len, 64);
    }

    #[test]
    fn resolution_ignores_an_unrelated_vendors_wider_collection() {
        // The interloper fixture carries a non-Razer collection that outscores
        // the headset on shape alone. Identity-based resolution must not pick
        // it, which is the failure mode `list` and `probe` both had.
        let b = FakeHidBackend::from_fixture_str(INTERLOPER).unwrap();
        let info = resolve_control_device(&b).unwrap();
        assert_eq!(info.vendor_id, 0x1532);
    }

    #[test]
    fn a_read_exchange_returns_the_matching_response() {
        let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        b.push_read(&info.id, response(Param::Battery.id(), false, &[52]));

        let mut s = ControlSession::open(&b).unwrap();
        let req = encode_read(Param::Battery.id(), None).unwrap();
        let f = s.exchange(&req, Param::Battery.id(), false).unwrap();
        assert_eq!(f.value(), Some(52));
    }

    #[test]
    fn an_event_does_not_satisfy_a_pending_read() {
        let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        // A battery event arrives first; the sidetone response follows.
        b.push_read(&info.id, event(Param::Battery.id(), &[49]));
        b.push_read(&info.id, response(Param::Sidetone.id(), false, &[7]));

        let mut s = ControlSession::open(&b).unwrap();
        let req = encode_read(Param::Sidetone.id(), None).unwrap();
        let f = s.exchange(&req, Param::Sidetone.id(), false).unwrap();
        assert_eq!(f.value(), Some(7), "the event must not have answered this");

        let events = s.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].param, Param::Battery.id());
        assert_eq!(events[0].value(), Some(49));
    }

    #[test]
    fn a_read_response_does_not_satisfy_a_write() {
        let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        // Same parameter, but the read form. It must not resolve a write.
        b.push_read(&info.id, response(Param::Sidetone.id(), false, &[7]));

        let mut s = ControlSession::open(&b)
            .unwrap()
            .with_exchange_timeout(Duration::from_millis(300));
        let req = encode_read(Param::Sidetone.id(), None).unwrap();
        assert!(s.exchange(&req, Param::Sidetone.id(), true).is_err());
    }

    #[test]
    fn a_silent_device_times_out_with_a_useful_message() {
        let b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let mut s = ControlSession::open(&b)
            .unwrap()
            .with_exchange_timeout(Duration::from_millis(300));
        let req = encode_read(Param::Battery.id(), None).unwrap();
        let err = s.exchange(&req, Param::Battery.id(), false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no response"), "{msg}");
        assert!(msg.contains("0 unrelated event"), "{msg}");
    }

    #[test]
    fn the_request_reaches_the_wire_verbatim() {
        let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        b.push_read(&info.id, response(Param::Battery.id(), false, &[52]));

        let mut s = ControlSession::open(&b).unwrap();
        let req = encode_read(Param::Battery.id(), None).unwrap();
        s.exchange(&req, Param::Battery.id(), false).unwrap();

        let writes = b.writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0], req.to_vec());
    }

    #[test]
    fn a_read_only_session_cannot_write() {
        let b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let mut s = ControlSession::open_read_only(&b).unwrap();
        let req = encode_read(Param::Battery.id(), None).unwrap();
        assert!(matches!(
            s.exchange(&req, Param::Battery.id(), false),
            Err(DeviceError::WriteNotSupported)
        ));
    }

    #[test]
    fn listen_collects_only_events() {
        let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        b.push_read(&info.id, event(Param::MicMute.id(), &[1]));
        b.push_read(&info.id, response(Param::Battery.id(), false, &[52]));
        b.push_read(&info.id, event(Param::Sidetone.id(), &[3]));

        let mut s = ControlSession::open_read_only(&b).unwrap();
        let events = s.listen(Duration::from_millis(400)).unwrap();
        assert_eq!(
            events.len(),
            2,
            "the response must not be reported as an event"
        );
        assert_eq!(events[0].param, Param::MicMute.id());
        assert_eq!(events[1].param, Param::Sidetone.id());
    }
}
