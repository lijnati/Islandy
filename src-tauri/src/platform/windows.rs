//! Win32 integration for the island window.
//!
//! Tauri already provides, through supported configuration:
//! - `focusable: false`  → `WS_EX_NOACTIVATE` (clicks don't steal focus)
//! - `skipTaskbar: true` → removed from the taskbar via `ITaskbarList`
//! - `alwaysOnTop: true` → `HWND_TOPMOST`
//!
//! It does not expose `WS_EX_TOOLWINDOW`, which additionally keeps the island
//! out of Alt+Tab / Task View. We add that here.
//!
//! Caveat: tao recomputes and rewrites `GWL_EXSTYLE` wholesale whenever one of
//! its own window flags changes, which drops `WS_EX_TOOLWINDOW`. Callers must
//! re-apply this after such changes.

use tauri::{Runtime, WebviewWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

use crate::error::{IslandyError, Result};

pub fn apply_tool_window_style<R: Runtime>(window: &WebviewWindow<R>) -> Result<()> {
    let hwnd = window.hwnd()?;

    // SAFETY: `hwnd` is a live top-level window owned by this process and we
    // run on the main thread (Tauri `setup`). We only toggle extended-style
    // bits and then ask Windows to refresh the frame without moving,
    // resizing, re-ordering or activating the window.
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let updated = (current | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
        if updated != current {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, updated);
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )
            .map_err(|e| IslandyError::Platform(format!("SetWindowPos failed: {e}")))?;
        }
    }
    Ok(())
}
