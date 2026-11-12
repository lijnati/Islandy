//! Win32 integration for the island window.
//!
//! Tauri already provides, through supported configuration:
//! - `focusable: false`  → `WS_EX_NOACTIVATE` (clicks don't steal focus)
//! - `skipTaskbar: true` → removed from the taskbar via `ITaskbarList`
//! - `alwaysOnTop: true` → `HWND_TOPMOST`
//!
//! Added here, because Tauri has no equivalent:
//! - `WS_EX_TOOLWINDOW` keeps the island out of Alt+Tab / Task View.
//! - Atomic move+resize (`SetWindowPos`) and a window region (`SetWindowRgn`)
//!   for the frame coordinator.
//! - Recording/restoring the foreground window around keyboard interaction.
//! - Opening allow-listed `ms-settings:` pages (`ShellExecuteW`).
//!
//! Caveat: tao recomputes and rewrites `GWL_EXSTYLE` wholesale whenever one of
//! its own window flags changes, which drops `WS_EX_TOOLWINDOW`. Callers must
//! re-apply it after such changes (see `IslandWindow::set_interactive`).

use tauri::{Runtime, WebviewWindow};
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::HWND,
        Graphics::Gdi::{CreateRectRgn, SetWindowRgn},
        UI::{
            Shell::ShellExecuteW,
            WindowsAndMessaging::{
                GetForegroundWindow, GetWindowLongPtrW, IsWindow, SetForegroundWindow,
                SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
                SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_SHOWNORMAL, WS_EX_APPWINDOW,
                WS_EX_TOOLWINDOW,
            },
        },
    },
};

use crate::{
    error::{IslandyError, Result},
    window::geometry::PhysicalRect,
};

pub fn apply_tool_window_style<R: Runtime>(window: &WebviewWindow<R>) -> Result<()> {
    let hwnd = window.hwnd()?;

    // SAFETY: `hwnd` is a live top-level window owned by this process. We only
    // toggle extended-style bits and then ask Windows to refresh the frame
    // without moving, resizing, re-ordering or activating the window.
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

pub fn set_window_bounds<R: Runtime>(window: &WebviewWindow<R>, b: PhysicalRect) -> Result<()> {
    let hwnd = window.hwnd()?;
    // SAFETY: plain move/resize of our own window; no z-order or activation change.
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            b.x,
            b.y,
            b.width as i32,
            b.height as i32,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
    .map_err(|e| IslandyError::Platform(format!("SetWindowPos failed: {e}")))
}

pub fn set_window_region<R: Runtime>(window: &WebviewWindow<R>, r: PhysicalRect) -> Result<()> {
    let hwnd = window.hwnd()?;
    // SAFETY: on success the system takes ownership of the region handle, so
    // it must not be deleted here. On failure it is leaked (tiny, and only on
    // a failure path we report).
    unsafe {
        let region = CreateRectRgn(r.x, r.y, r.x + r.width as i32, r.y + r.height as i32);
        if region.is_invalid() {
            return Err(IslandyError::Platform("CreateRectRgn failed".into()));
        }
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            return Err(IslandyError::Platform("SetWindowRgn failed".into()));
        }
    }
    Ok(())
}

pub fn window_handle<R: Runtime>(window: &WebviewWindow<R>) -> Option<isize> {
    window.hwnd().ok().map(|h| h.0 as isize)
}

pub fn foreground_window() -> Option<isize> {
    // SAFETY: no preconditions.
    let hwnd = unsafe { GetForegroundWindow() };
    (!hwnd.is_invalid()).then_some(hwnd.0 as isize)
}

pub fn is_foreground<R: Runtime>(window: &WebviewWindow<R>) -> bool {
    window_handle(window).is_some_and(|own| foreground_window() == Some(own))
}

pub fn restore_foreground(handle: isize) {
    let hwnd = HWND(handle as *mut core::ffi::c_void);
    // SAFETY: `IsWindow` validates a possibly stale handle before use. We are
    // the foreground process at this point, so the call is permitted; if it
    // is refused, focus simply stays where Windows puts it.
    unsafe {
        if IsWindow(Some(hwnd)).as_bool() {
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

pub fn open_settings_uri(uri: &'static str) -> Result<()> {
    debug_assert!(uri.starts_with("ms-settings:"));
    let target = HSTRING::from(uri);
    // SAFETY: all strings are valid, NUL-terminated and outlive the call.
    let result = unsafe {
        ShellExecuteW(
            None,
            &HSTRING::from("open"),
            &target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns a value greater than 32 on success.
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(IslandyError::Platform(format!(
            "could not open {uri} (code {})",
            result.0 as isize
        )))
    }
}
