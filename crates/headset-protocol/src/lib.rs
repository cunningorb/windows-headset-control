//! Pure protocol logic. No operating-system access.
#![forbid(unsafe_code)]

pub mod error;
pub mod frame;

pub use error::ProtocolError;
pub use frame::{ControlFrame, CONTROL_PAYLOAD_LEN, CONTROL_REPORT_ID, CONTROL_REPORT_LEN};
