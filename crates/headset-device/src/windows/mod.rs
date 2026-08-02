mod ffi;
mod transport;

use crate::backend::{HidBackend, HidTransport};
use crate::error::DeviceError;
use crate::model::{CollectionInfo, DeviceId, OpenMode};
use transport::{DescriptorHandle, WindowsTransport};

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

    fn open(&self, id: &DeviceId, mode: OpenMode) -> Result<Box<dyn HidTransport>, DeviceError> {
        let info = self
            .enumerate()?
            .into_iter()
            .find(|c| c.id == *id)
            .ok_or(DeviceError::DongleNotFound)?;

        if mode == OpenMode::ReadWrite && info.is_audio_stack_collection() {
            return Err(DeviceError::RefusedAudioCollection);
        }

        match mode {
            OpenMode::Descriptors => {
                let h = ffi::open_for_descriptors(info.id.raw())?;
                Ok(Box::new(DescriptorHandle::new(h, info.input_report_len)))
            }
            OpenMode::ReadWrite => {
                // Phase 1 grants read access only, never write access, even for
                // OpenMode::ReadWrite. Write access arrives with the write phase.
                let h = ffi::open_for_read(info.id.raw())?;
                Ok(Box::new(WindowsTransport::new(h, info.input_report_len)))
            }
        }
    }
}
