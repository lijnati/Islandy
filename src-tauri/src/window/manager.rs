use tauri::{AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

use super::geometry::{self, LogicalSize, PhysicalRect};
use crate::{
    error::{IslandyError, Result},
    platform,
};

/// Label of the island window; must match `app.windows[].label` in `tauri.conf.json`.
pub const ISLAND_LABEL: &str = "island";

/// Owns all native manipulation of the island window.
///
/// Every operation that changes window flags must go through this type so it
/// can re-apply platform styles afterwards (tao rewrites the whole extended
/// style on Windows when its own flags change; see `platform::windows`).
pub struct IslandWindow<R: Runtime> {
    window: WebviewWindow<R>,
}

impl<R: Runtime> IslandWindow<R> {
    pub fn get(app: &AppHandle<R>) -> Result<Self> {
        app.get_webview_window(ISLAND_LABEL)
            .map(|window| Self { window })
            .ok_or(IslandyError::WindowNotFound(ISLAND_LABEL))
    }

    /// One-time native setup, called before the window is first shown.
    pub fn initialize(&self) -> Result<()> {
        platform::configure_island_window(&self.window)?;
        self.apply_size(geometry::size::COLLAPSED)
    }

    /// The monitor the island lives on: the primary monitor, falling back to
    /// the window's current monitor, then to any available monitor.
    pub fn target_monitor(&self) -> Result<Monitor> {
        if let Some(monitor) = self.window.primary_monitor()? {
            return Ok(monitor);
        }
        if let Some(monitor) = self.window.current_monitor()? {
            return Ok(monitor);
        }
        self.window
            .available_monitors()?
            .into_iter()
            .next()
            .ok_or(IslandyError::NoMonitor)
    }

    /// Resizes the island to `size` (logical px) and re-centers it at the top
    /// of the target monitor's work area.
    pub fn apply_size(&self, size: LogicalSize) -> Result<()> {
        let monitor = self.target_monitor()?;
        let work_area = monitor.work_area();
        let area = PhysicalRect {
            x: work_area.position.x,
            y: work_area.position.y,
            width: work_area.size.width,
            height: work_area.size.height,
        };
        let bounds = geometry::top_center(area, monitor.scale_factor(), size, geometry::TOP_MARGIN);

        // Move first so a cross-monitor DPI change (WM_DPICHANGED) is handled
        // before we set the final physical size, then move again to land on
        // the exact centered position for that size.
        let position = PhysicalPosition::new(bounds.x, bounds.y);
        self.window.set_position(position)?;
        self.window
            .set_size(PhysicalSize::new(bounds.width, bounds.height))?;
        self.window.set_position(position)?;
        Ok(())
    }

    /// Re-centers at the current size preset (e.g. after a DPI change).
    pub fn reposition(&self) -> Result<()> {
        self.apply_size(geometry::size::COLLAPSED)
    }

    pub fn show(&self) -> Result<()> {
        self.window.show()?;
        Ok(())
    }
}
