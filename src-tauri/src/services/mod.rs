//! Application services: plain Rust logic that commands delegate to.

pub mod app_info;
pub mod media;
pub mod system;
#[cfg(windows)]
pub mod winrt;
