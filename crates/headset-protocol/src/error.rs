use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("expected a {expected}-byte report, got {actual}")]
    UnexpectedLength { expected: usize, actual: usize },

    #[error("expected report id {expected:#04x}, got {actual:#04x}")]
    UnexpectedReportId { expected: u8, actual: u8 },
}
