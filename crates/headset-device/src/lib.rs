//! Windows HID device access behind a mockable backend trait.

pub mod error;
pub mod model;

pub use error::DeviceError;
pub use model::{CollectionInfo, DeviceId, OpenMode, ReportItem, ReportKind};
