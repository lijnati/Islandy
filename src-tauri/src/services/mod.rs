//! Application services: plain Rust logic that commands delegate to.

pub mod activity;
pub mod app_info;
pub mod diagnostics;
pub mod media;
pub mod paths;
pub mod settings;
pub mod sound;
pub mod startup;
pub mod system;
pub mod updates;
pub mod weather;
#[cfg(windows)]
pub mod winrt;
