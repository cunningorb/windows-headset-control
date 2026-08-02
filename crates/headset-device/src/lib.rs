//! Windows HID device access behind a mockable backend trait.

pub mod backend;
pub mod error;
pub mod fake;
pub mod model;
pub mod select;

#[cfg(windows)]
pub mod windows;

pub use backend::{HidBackend, HidTransport};
pub use error::DeviceError;
pub use fake::FakeHidBackend;
pub use model::{CollectionInfo, DeviceId, OpenMode, ReportItem, ReportKind};
pub use select::{
    has_unambiguous_winner, is_supported_device, rank_candidates, stable_sort_collections,
    Candidate, SUPPORTED_PRODUCT_IDS, SUPPORTED_VENDOR_ID,
};

#[cfg(windows)]
pub use windows::WindowsHidBackend;
