//! The panel UI.
//!
//! `theme` holds appearance as data, `layout` turns state into positioned
//! primitives and hit regions with no OS access at all, and `render` walks that
//! primitive list through Direct2D. The split is what makes hit-testing, tick
//! spacing and value mapping testable without a window, a GPU, or a headset.

pub mod layout;
pub mod theme;

#[cfg(windows)]
pub mod render;

pub use layout::{build, HitTarget, Panel, SliderParam, View};
