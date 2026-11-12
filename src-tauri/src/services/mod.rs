//! Application services: plain Rust logic that commands delegate to.

pub mod activity;
pub mod app_info;
pub mod media;
pub mod paths;
pub mod system;
#[cfg(windows)]
pub mod winrt;
