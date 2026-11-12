//! OS-specific integrations, isolated behind small functions so the rest of
//! the app stays platform-neutral. Everything here should be a thin layer
//! over a supported API; prefer Tauri APIs and fall back to Win32 only when
//! Tauri has no equivalent.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use tauri::{Runtime, WebviewWindow};

use crate::{error::Result, window::geometry::PhysicalRect};

/// Applies native window styles Tauri does not expose.
///
/// Must be re-applied after any Tauri call that changes window flags
/// (always-on-top, focusable, decorations, …) — see [`crate::window::IslandWindow`].
pub fn configure_island_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<()> {
    #[cfg(windows)]
    {
        windows::apply_tool_window_style(window)
    }
    #[cfg(target_os = "linux")]
    {
        linux::allow_small_window(window)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = window;
        Ok(())
    }
}

/// Moves and resizes the window in physical virtual-desktop pixels.
/// On Windows this is one atomic `SetWindowPos`, so there is never a frame
/// with the new size at the old position.
pub fn set_window_bounds<R: Runtime>(
    window: &WebviewWindow<R>,
    bounds: PhysicalRect,
) -> Result<()> {
    #[cfg(windows)]
    {
        windows::set_window_bounds(window, bounds)
    }
    #[cfg(not(windows))]
    {
        use tauri::{PhysicalPosition, PhysicalSize};
        let position = PhysicalPosition::new(bounds.x, bounds.y);
        window.set_position(position)?;
        window.set_size(PhysicalSize::new(bounds.width, bounds.height))?;
        window.set_position(position)?;
        Ok(())
    }
}

/// Limits drawing and hit-testing to `region` (physical px, window-relative).
/// Pixels outside it are not part of the window: clicks go to the window below.
pub fn set_window_region<R: Runtime>(
    window: &WebviewWindow<R>,
    region: PhysicalRect,
) -> Result<()> {
    #[cfg(windows)]
    {
        windows::set_window_region(window, region)
    }
    #[cfg(target_os = "linux")]
    {
        linux::set_window_shape(window, region)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (window, region);
        Ok(())
    }
}

/// Opaque handle of the island window, comparable with [`foreground_window`].
pub fn window_handle<R: Runtime>(window: &WebviewWindow<R>) -> Option<isize> {
    #[cfg(windows)]
    {
        windows::window_handle(window)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        None
    }
}

/// The current foreground window (Windows only).
pub fn foreground_window() -> Option<isize> {
    #[cfg(windows)]
    {
        windows::foreground_window()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Whether the island window currently has keyboard focus.
pub fn is_foreground<R: Runtime>(window: &WebviewWindow<R>) -> bool {
    #[cfg(windows)]
    {
        windows::is_foreground(window)
    }
    #[cfg(not(windows))]
    {
        window.is_focused().unwrap_or(false)
    }
}

/// Gives focus back to a window recorded with [`foreground_window`].
pub fn restore_foreground(handle: isize) {
    #[cfg(windows)]
    windows::restore_foreground(handle);
    #[cfg(not(windows))]
    let _ = handle;
}

/// Opens a fixed, allow-listed `ms-settings:` URI.
pub fn open_settings_uri(uri: &'static str) -> Result<()> {
    #[cfg(windows)]
    {
        windows::open_settings_uri(uri)
    }
    #[cfg(not(windows))]
    {
        let _ = uri;
        Err(crate::error::IslandyError::Unsupported(
            "Windows settings pages are only available on Windows",
        ))
    }
}
