//! Islandy — a lightweight, context-aware floating island for Windows.
//!
//! Module map:
//! - [`app`]       application lifecycle (builder, setup, tray, run loop)
//! - [`cli`]       command-line modes (`islandy hook`, `notify`, `status`)
//! - [`window`]    island window geometry and management
//! - [`platform`]  isolated OS-specific integrations (Win32)
//! - [`commands`]  the Tauri IPC surface exposed to the frontend
//! - [`services`]  application services used by commands
//! - [`signals`]   Rust → frontend push channel
//! - [`shortcut`]  global toggle shortcut (configurable)
//! - [`tray`]      notification-area icon and menu
//! - [`settings_window`] the separate settings window
//! - [`error`]     the shared, serializable error type

mod app;
mod cli;
mod commands;
mod error;
mod platform;
mod services;
mod settings_window;
mod shortcut;
mod signals;
mod tray;
mod window;

pub use app::run;
pub use cli::main as cli_main;
