use std::time::Duration;

use crate::error::DeviceError;
use crate::model::{CollectionInfo, DeviceId, OpenMode};

/// An open handle to one HID collection.
pub trait HidTransport {
    /// Reads one input report. `buf` must be at least the collection's
    /// `input_report_len`. Returns the number of bytes read, including the
    /// leading report-ID byte.
    fn read_report(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, DeviceError>;

    /// The collection's declared input report length, including the report-ID byte.
    fn input_report_len(&self) -> u16;

    /// Sends one output report. `buf[0]` must be the report ID.
    ///
    /// Defaulted to an error on purpose: a transport acquires write ability only
    /// by overriding this deliberately. A read-only handle that inherits the
    /// default cannot write even if a caller asks it to, so the guarantee is a
    /// property of the type rather than of caller discipline.
    fn write_report(&self, _buf: &[u8]) -> Result<(), DeviceError> {
        Err(DeviceError::WriteNotSupported)
    }

    /// The collection's declared output report length, including the report-ID
    /// byte. Zero when the collection declares no output report.
    fn output_report_len(&self) -> u16 {
        0
    }
}

// `Result::unwrap_err` requires the `Ok` type to implement `Debug`. `HidTransport`
// implementors otherwise have no reason to derive it, so provide a minimal impl for
// the trait object itself rather than widening the trait with a `Debug` supertrait.
impl std::fmt::Debug for dyn HidTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("dyn HidTransport")
    }
}

pub trait HidBackend {
    fn enumerate(&self) -> Result<Vec<CollectionInfo>, DeviceError>;

    fn open(&self, id: &DeviceId, mode: OpenMode) -> Result<Box<dyn HidTransport>, DeviceError>;
}
