//! Pure protocol logic. No operating-system access.
#![forbid(unsafe_code)]

pub mod error;
pub mod frame;
pub mod noise;
pub mod param;

pub use error::ProtocolError;
pub use frame::{ControlFrame, CONTROL_PAYLOAD_LEN, CONTROL_REPORT_ID, CONTROL_REPORT_LEN};
pub use noise::{encode_noise_write, NoiseControl, NoiseMode, ANC_LEVEL_RANGE, NOISE_PARAM};
pub use param::{
    checksum, encode_read, encode_write, encode_write_payload, parse, write_payload_len, Param,
    ParamFrame, ResultCode, Role, INDEXED_READS, MAX_PAYLOAD, PAIR_WRITES, READ_ALLOWLIST,
    WRITE_ALLOWLIST,
};
