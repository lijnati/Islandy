//! Application services: plain Rust logic that commands delegate to.

pub mod activity;
pub mod app_info;
pub mod diagnostics;
pub mod media;
pub mod paths;
pub mod settings;
pub mod startup;
pub mod system;
#[cfg(windows)]
pub mod winrt;
