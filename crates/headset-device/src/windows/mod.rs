mod ffi;

use crate::backend::{HidBackend, HidTransport};
use crate::error::DeviceError;
use crate::model::{CollectionInfo, DeviceId, OpenMode};

pub struct WindowsHidBackend;

impl WindowsHidBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsHidBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl HidBackend for WindowsHidBackend {
    fn enumerate(&self) -> Result<Vec<CollectionInfo>, DeviceError> {
        let mut out = Vec::new();
        for path in ffi::enumerate_interface_paths()? {
            let Some(raw) = ffi::read_caps(&path) else {
                // A device that vanished or denied a descriptor open is skipped,
                // not fatal: enumeration must survive concurrent disconnects.
                tracing::debug!("skipping unreadable HID interface");
                continue;
            };
            let id = DeviceId::new(path);
            out.push(CollectionInfo {
                interface_number: id.interface_number(),
                collection_number: id.collection_number(),
                id,
                vendor_id: raw.vendor_id,
                product_id: raw.product_id,
                version: raw.version,
                usage_page: raw.usage_page,
                usage: raw.usage,
                input_report_len: raw.input_report_len,
                output_report_len: raw.output_report_len,
                feature_report_len: raw.feature_report_len,
                product: raw.product,
                manufacturer: raw.manufacturer,
                has_serial: raw.has_serial,
                report_items: raw.report_items,
            });
        }
        Ok(out)
    }

    fn open(&self, _id: &DeviceId, _mode: OpenMode) -> Result<Box<dyn HidTransport>, DeviceError> {
        // Implemented in Task 9. Deliberately absent until then so that no
        // caller can open a real device before the read path is reviewed.
        Err(DeviceError::Os(
            "open is not implemented until Task 9".into(),
        ))
    }
}
