//! Pure protocol logic. No operating-system access.
#![forbid(unsafe_code)]

pub mod error;
pub mod frame;
pub mod param;

pub use error::ProtocolError;
pub use frame::{ControlFrame, CONTROL_PAYLOAD_LEN, CONTROL_REPORT_ID, CONTROL_REPORT_LEN};
pub use param::{
    checksum, encode_read, encode_write, parse, Param, ParamFrame, ResultCode, Role, INDEXED_READS,
    MAX_PAYLOAD, READ_ALLOWLIST, WRITE_ALLOWLIST,
};
