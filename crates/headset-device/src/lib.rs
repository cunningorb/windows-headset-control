//! Windows HID device access behind a mockable backend trait.

pub mod backend;
pub mod error;
pub mod fake;
pub mod model;

#[cfg(windows)]
pub mod windows;

pub use backend::{HidBackend, HidTransport};
pub use error::DeviceError;
pub use fake::FakeHidBackend;
pub use model::{CollectionInfo, DeviceId, OpenMode, ReportItem, ReportKind};

#[cfg(windows)]
pub use windows::WindowsHidBackend;
