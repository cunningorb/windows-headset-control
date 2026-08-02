use crate::error::ProtocolError;

/// Report ID declared by the control collection for both input and output.
/// Measured from the device's own report descriptor, not assumed.
pub const CONTROL_REPORT_ID: u8 = 0x02;

/// Total report length in bytes, including the leading report-ID byte.
pub const CONTROL_REPORT_LEN: usize = 64;

/// Payload length after the report-ID byte.
pub const CONTROL_PAYLOAD_LEN: usize = CONTROL_REPORT_LEN - 1;

/// A validated 64-byte control report.
///
/// This models the *container* only. No payload semantics are established for
/// this hardware, so no field is interpreted. Bytes are surfaced verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlFrame {
    pub report_id: u8,
    pub payload: [u8; CONTROL_PAYLOAD_LEN],
}

impl ControlFrame {
    /// Validates length and report id before touching the payload.
    pub fn parse(raw: &[u8]) -> Result<Self, ProtocolError> {
        if raw.len() != CONTROL_REPORT_LEN {
            return Err(ProtocolError::UnexpectedLength {
                expected: CONTROL_REPORT_LEN,
                actual: raw.len(),
            });
        }
        if raw[0] != CONTROL_REPORT_ID {
            return Err(ProtocolError::UnexpectedReportId {
                expected: CONTROL_REPORT_ID,
                actual: raw[0],
            });
        }
        let mut payload = [0u8; CONTROL_PAYLOAD_LEN];
        payload.copy_from_slice(&raw[1..]);
        Ok(Self {
            report_id: raw[0],
            payload,
        })
    }

    /// Payload fields whose meaning is established. Empty by design: nothing is
    /// known yet, and inventing meanings would corrupt the research record.
    pub fn known_fields(&self) -> Vec<(&'static str, u8)> {
        Vec::new()
    }

    /// Space-separated lowercase hex of every payload byte.
    pub fn hex_payload(&self) -> String {
        self.payload
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(id: u8, len: usize) -> Vec<u8> {
        let mut v = vec![0u8; len];
        v[0] = id;
        v
    }

    #[test]
    fn parses_a_well_formed_control_frame() {
        let mut raw = buf(CONTROL_REPORT_ID, CONTROL_REPORT_LEN);
        raw[1] = 0xAB;
        let f = ControlFrame::parse(&raw).unwrap();
        assert_eq!(f.report_id, CONTROL_REPORT_ID);
        assert_eq!(f.payload.len(), CONTROL_PAYLOAD_LEN);
        assert_eq!(f.payload[0], 0xAB);
    }

    #[test]
    fn rejects_a_short_buffer() {
        let raw = buf(CONTROL_REPORT_ID, 10);
        assert!(matches!(
            ControlFrame::parse(&raw),
            Err(ProtocolError::UnexpectedLength {
                expected: CONTROL_REPORT_LEN,
                actual: 10
            })
        ));
    }

    #[test]
    fn rejects_an_oversized_buffer() {
        let raw = buf(CONTROL_REPORT_ID, CONTROL_REPORT_LEN + 1);
        assert!(matches!(
            ControlFrame::parse(&raw),
            Err(ProtocolError::UnexpectedLength { .. })
        ));
    }

    #[test]
    fn rejects_an_empty_buffer() {
        assert!(matches!(
            ControlFrame::parse(&[]),
            Err(ProtocolError::UnexpectedLength { .. })
        ));
    }

    #[test]
    fn rejects_an_unexpected_report_id() {
        let raw = buf(0x07, CONTROL_REPORT_LEN);
        assert!(matches!(
            ControlFrame::parse(&raw),
            Err(ProtocolError::UnexpectedReportId {
                expected: CONTROL_REPORT_ID,
                actual: 0x07
            })
        ));
    }

    #[test]
    fn every_payload_byte_starts_unknown() {
        let raw = buf(CONTROL_REPORT_ID, CONTROL_REPORT_LEN);
        let f = ControlFrame::parse(&raw).unwrap();
        assert_eq!(
            f.known_fields().len(),
            0,
            "no payload semantics are established yet"
        );
    }

    #[test]
    fn hex_dump_covers_the_whole_payload() {
        let mut raw = buf(CONTROL_REPORT_ID, CONTROL_REPORT_LEN);
        raw[CONTROL_REPORT_LEN - 1] = 0xFF;
        let f = ControlFrame::parse(&raw).unwrap();
        let dump = f.hex_payload();
        assert!(dump.ends_with("ff"));
        assert_eq!(dump.split(' ').count(), CONTROL_PAYLOAD_LEN);
    }
}
