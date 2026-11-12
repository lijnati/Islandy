//! OS-specific integrations, isolated behind small functions so the rest of
//! the app stays platform-neutral. Everything here should be a thin layer
//! over a supported API; prefer Tauri APIs and fall back to Win32 only when
//! Tauri has no equivalent.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use tauri::{Runtime, WebviewWindow};

use crate::error::Result;

/// Applies native window styles Tauri does not expose.
///
/// Must be re-applied after any Tauri call that changes window flags
/// (always-on-top, focusable, decorations, …) — see [`crate::window::IslandWindow`].
pub fn configure_island_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<()> {
    #[cfg(windows)]
    {
        windows::apply_tool_window_style(window)
    }
    #[cfg(not(windows))]
    {
        #[cfg(target_os = "linux")]
        linux::allow_small_window(window)?;
        #[cfg(not(target_os = "linux"))]
        let _ = window;
        Ok(())
    }
}
