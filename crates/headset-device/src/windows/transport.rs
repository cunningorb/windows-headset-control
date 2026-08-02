use std::time::Duration;

use windows::Win32::Foundation::HANDLE;

use super::ffi;
use crate::backend::HidTransport;
use crate::error::DeviceError;

/// A read-only handle to one HID collection.
///
/// There is deliberately no write method. Adding one is a reviewed change that
/// belongs to the write phase, not to Phase 1.
pub struct WindowsTransport {
    handle: HANDLE,
    input_report_len: u16,
}

impl WindowsTransport {
    pub(super) fn new(handle: HANDLE, input_report_len: u16) -> Self {
        Self {
            handle,
            input_report_len,
        }
    }
}

impl HidTransport for WindowsTransport {
    fn read_report(&self, buf: &mut [u8], timeout: Duration) -> Result<usize, DeviceError> {
        // Windows requires the read buffer to be at least the declared input
        // report length. A short buffer yields ERROR_INVALID_USER_BUFFER, so
        // reject it here with a message that explains the real constraint.
        if buf.len() < self.input_report_len as usize {
            return Err(DeviceError::UnexpectedDescriptor(format!(
                "read buffer is {} bytes; the collection declares {}",
                buf.len(),
                self.input_report_len
            )));
        }
        ffi::read_with_timeout(self.handle, buf, timeout)
    }

    fn input_report_len(&self) -> u16 {
        self.input_report_len
    }
}

impl Drop for WindowsTransport {
    fn drop(&mut self) {
        ffi::close(self.handle);
    }
}

/// A handle opened with no I/O rights. Exists to prove the collection can be
/// opened and released; it cannot read or write.
pub struct DescriptorHandle {
    handle: HANDLE,
    input_report_len: u16,
}

impl DescriptorHandle {
    pub(super) fn new(handle: HANDLE, input_report_len: u16) -> Self {
        Self {
            handle,
            input_report_len,
        }
    }
}

impl HidTransport for DescriptorHandle {
    fn read_report(&self, _buf: &mut [u8], _timeout: Duration) -> Result<usize, DeviceError> {
        Err(DeviceError::AccessDenied)
    }

    fn input_report_len(&self) -> u16 {
        self.input_report_len
    }
}

impl Drop for DescriptorHandle {
    fn drop(&mut self) {
        ffi::close(self.handle);
    }
}
