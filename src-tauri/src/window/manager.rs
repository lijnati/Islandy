use std::sync::{Mutex, MutexGuard};

use serde::Serialize;
use tauri::{AppHandle, Manager, Monitor, Runtime, WebviewWindow};

use super::geometry::{self, Layouts, LogicalSize, PhysicalRect};
use crate::{
    error::{IslandyError, Result},
    platform,
};

/// Label of the island window; must match `app.windows[].label` in `tauri.conf.json`.
pub const ISLAND_LABEL: &str = "island";

/// How the visible island *frame* (the animated surface) maps onto the native window.
///
/// See `docs/architecture.md` → "Window frame coordination" for the tradeoffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameStrategy {
    /// The native window is a fixed canvas big enough for every layout, and a
    /// window region (Win32 `SetWindowRgn`; X11 shape on the Linux dev host)
    /// limits drawing and hit-testing to the current frame. The window never
    /// moves or resizes during an animation, so the WebView never re-lays out
    /// mid-transition and the island cannot jitter. Default.
    Region,
    /// The native window itself is resized and re-centered to each frame
    /// (one atomic `SetWindowPos` on Windows). Fallback for systems where
    /// regions misbehave; select with `ISLANDY_FRAME_STRATEGY=resize`.
    Resize,
}

impl FrameStrategy {
    fn from_env() -> Self {
        match std::env::var("ISLANDY_FRAME_STRATEGY").as_deref() {
            Ok("resize") => Self::Resize,
            _ => Self::Region,
        }
    }
}

/// Where the island currently lives: one monitor's work area and scale.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placement {
    area: PhysicalRect,
    scale: f64,
    layouts: Layouts,
    /// Native window bounds under [`FrameStrategy::Region`] (the canvas).
    canvas: PhysicalRect,
}

/// Mutable native-window state, managed by Tauri (`app.manage`).
pub struct WindowState {
    strategy: FrameStrategy,
    placement: Option<Placement>,
    /// The logical frame size most recently requested by the frontend.
    frame: LogicalSize,
    /// Window that was in the foreground before the island took focus, so it
    /// can be given focus back when the island is dismissed with the keyboard.
    previous_foreground: Option<isize>,
}

impl WindowState {
    pub fn new() -> Mutex<Self> {
        Mutex::new(Self {
            strategy: FrameStrategy::from_env(),
            placement: None,
            frame: geometry::size::COLLAPSED,
            previous_foreground: None,
        })
    }
}

/// What to do to the native window, computed under the state lock and applied
/// after releasing it (platform calls can re-enter the event loop).
struct FramePlan {
    move_canvas: Option<PhysicalRect>,
    region: Option<PhysicalRect>,
    bounds: Option<PhysicalRect>,
    placement_changed: bool,
}

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

    fn state(&self) -> MutexGuard<'_, WindowState> {
        let state = self.window.state::<Mutex<WindowState>>().inner();
        state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn strategy(&self) -> FrameStrategy {
        self.state().strategy
    }

    /// One-time native setup, called before the window is first shown.
    pub fn initialize(&self) -> Result<()> {
        platform::configure_island_window(&self.window)?;
        self.apply(true).map(|_| ())
    }

    /// The monitor the island lives on: the primary monitor, falling back to
    /// the window's current monitor, then to any available monitor.
    fn target_monitor(&self) -> Result<Monitor> {
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

    fn read_monitor(&self) -> Result<(PhysicalRect, f64)> {
        let monitor = self.target_monitor()?;
        let work_area = monitor.work_area();
        let area = PhysicalRect {
            x: work_area.position.x,
            y: work_area.position.y,
            width: work_area.size.width,
            height: work_area.size.height,
        };
        Ok((area, monitor.scale_factor()))
    }

    /// Layouts fitted to the island's current monitor. Re-reads the monitor,
    /// so a resolution, scaling or taskbar change is picked up here.
    pub fn layouts(&self) -> Result<Layouts> {
        self.apply(false)?;
        let state = self.state();
        state
            .placement
            .map(|p| p.layouts)
            .ok_or(IslandyError::NoMonitor)
    }

    /// Sets the visible frame (logical px) and applies it to the native
    /// window. Returns `true` when the monitor geometry changed since the last
    /// call, i.e. the frontend should refresh its layouts.
    pub fn set_frame(&self, frame: LogicalSize) -> Result<bool> {
        self.state().frame = frame;
        self.apply(false)
    }

    /// Re-reads the monitor and re-applies placement and frame (DPI change).
    pub fn refresh(&self) -> Result<bool> {
        self.apply(true)
    }

    fn apply(&self, force_placement: bool) -> Result<bool> {
        let (area, scale) = self.read_monitor()?;
        let plan = {
            let mut state = self.state();
            Self::plan(&mut state, area, scale, force_placement)
        };
        if let Some(canvas) = plan.move_canvas {
            platform::set_window_bounds(&self.window, canvas)?;
        }
        if let Some(bounds) = plan.bounds {
            platform::set_window_bounds(&self.window, bounds)?;
        }
        if let Some(region) = plan.region {
            platform::set_window_region(&self.window, region)?;
        }
        Ok(plan.placement_changed)
    }

    fn plan(state: &mut WindowState, area: PhysicalRect, scale: f64, force: bool) -> FramePlan {
        let unchanged = state
            .placement
            .is_some_and(|p| p.area == area && p.scale == scale);
        let placement_changed = force || !unchanged;
        if placement_changed {
            let layouts =
                geometry::fit_layouts(geometry::logical_size_of(area, scale), geometry::TOP_MARGIN);
            let canvas = geometry::top_center(area, scale, layouts.canvas(), geometry::TOP_MARGIN);
            state.placement = Some(Placement {
                area,
                scale,
                layouts,
                canvas,
            });
        }
        let Some(p) = state.placement else {
            unreachable!("placement is set above");
        };
        let frame = state.frame.clamp_to(p.layouts.canvas());
        match state.strategy {
            FrameStrategy::Region => FramePlan {
                move_canvas: placement_changed.then_some(p.canvas),
                region: Some(geometry::frame_region(
                    p.canvas.width,
                    p.canvas.height,
                    p.scale,
                    frame,
                )),
                bounds: None,
                placement_changed,
            },
            FrameStrategy::Resize => FramePlan {
                move_canvas: None,
                region: None,
                bounds: Some(geometry::top_center(
                    p.area,
                    p.scale,
                    frame,
                    geometry::TOP_MARGIN,
                )),
                placement_changed,
            },
        }
    }

    /// Makes the island keyboard-interactive (focusable and focused) or
    /// returns it to its default non-activating state.
    ///
    /// The island only takes focus after an explicit user action (click to
    /// pin, opening a widget, or the global shortcut), which is what makes
    /// Escape and click-outside (focus loss) work. Hover never takes focus.
    pub fn set_interactive(&self, interactive: bool) -> Result<()> {
        let window = self.window.clone();
        // Run on the event-loop thread so tao applies its style change before
        // we re-apply ours (tao rewrites GWL_EXSTYLE when `focusable` changes).
        self.window.run_on_main_thread(move || {
            let island = IslandWindow { window };
            if let Err(e) = island.set_interactive_now(interactive) {
                eprintln!("[islandy] failed to change focus state: {e}");
            }
        })?;
        Ok(())
    }

    fn set_interactive_now(&self, interactive: bool) -> Result<()> {
        if interactive {
            let previous = platform::foreground_window();
            let own = platform::window_handle(&self.window);
            {
                let mut state = self.state();
                if state.previous_foreground.is_none() && previous != own {
                    state.previous_foreground = previous;
                }
            }
            self.window.set_focusable(true)?;
            platform::configure_island_window(&self.window)?;
            self.window.set_focus()?;
        } else {
            let previous = self.state().previous_foreground.take();
            let had_focus = platform::is_foreground(&self.window);
            self.window.set_focusable(false)?;
            platform::configure_island_window(&self.window)?;
            if had_focus {
                if let Some(previous) = previous {
                    platform::restore_foreground(previous);
                }
            }
        }
        Ok(())
    }

    pub fn show(&self) -> Result<()> {
        self.window.show()?;
        Ok(())
    }
}
