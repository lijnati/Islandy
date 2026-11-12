//! Island window management: geometry math is pure and unit-tested
//! ([`geometry`]); [`manager`] applies it to the native window.

pub mod geometry;
mod manager;

pub use manager::{FrameStrategy, IslandWindow, WindowState};
