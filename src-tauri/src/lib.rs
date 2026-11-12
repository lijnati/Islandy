//! Islandy — a lightweight, context-aware floating island for Windows.
//!
//! Module map:
//! - [`app`]       application lifecycle (builder, setup, tray, run loop)
//! - [`window`]    island window geometry and management
//! - [`platform`]  isolated OS-specific integrations (Win32)
//! - [`commands`]  the Tauri IPC surface exposed to the frontend
//! - [`services`]  application services used by commands
//! - [`signals`]   Rust → frontend push channel
//! - [`shortcut`]  global toggle shortcut
//! - [`error`]     the shared, serializable error type

mod app;
mod commands;
mod error;
mod platform;
mod services;
mod shortcut;
mod signals;
mod window;

pub use app::run;
