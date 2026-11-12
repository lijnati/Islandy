use std::sync::{Mutex, MutexGuard};

use serde::Serialize;
use tauri::{AppHandle, Manager, Monitor, Runtime, WebviewWindow};

use super::geometry::{self, Align, Layouts, LogicalSize, PhysicalRect, PillSizes, PlaceOptions};
use crate::{
    error::{IslandyError, Result},
    platform,
    services::settings::models::{Anchor, IslandSettings, SizePreset, PRIMARY_MONITOR},
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
#[derive(Debug, Clone, PartialEq)]
struct Placement {
    monitor: MonitorGeometry,
    options: PlaceOptions,
    pills: PillSizes,
    layouts: Layouts,
    /// Native window bounds under [`FrameStrategy::Region`] (the canvas).
    canvas: PhysicalRect,
    /// The horizontal offset actually applied after clamping (logical px).
    applied_offset: f64,
}

/// What Islandy reads from the target monitor.
#[derive(Debug, Clone, PartialEq)]
struct MonitorGeometry {
    /// Work area (excludes the taskbar), physical px.
    area: PhysicalRect,
    /// Whole monitor, physical px (fullscreen detection).
    bounds: PhysicalRect,
    scale: f64,
    name: Option<String>,
}

/// User placement preferences (from `IslandSettings`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementConfig {
    pub anchor: Anchor,
    pub offset_x: f64,
    pub top_margin: f64,
    /// `"primary"` or a monitor name.
    pub monitor: String,
    pub size: SizePreset,
}

impl Default for PlacementConfig {
    fn default() -> Self {
        Self {
            anchor: Anchor::TopCenter,
            offset_x: 0.0,
            top_margin: geometry::TOP_MARGIN,
            monitor: PRIMARY_MONITOR.to_owned(),
            size: SizePreset::Standard,
        }
    }
}

impl PlacementConfig {
    pub fn from_settings(s: &IslandSettings) -> Self {
        Self {
            anchor: s.anchor,
            offset_x: f64::from(s.offset_x),
            top_margin: f64::from(s.top_margin),
            monitor: s.monitor.clone(),
            size: s.size,
        }
    }
}

/// A monitor the island can be placed on (for the settings UI).
/// Mirrored by `MonitorInfo` in `src/services/types.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub id: String,
    pub label: String,
    pub primary: bool,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    /// The island is on this monitor now.
    pub current: bool,
}

/// Mutable native-window state, managed by Tauri (`app.manage`).
pub struct WindowState {
    strategy: FrameStrategy,
    config: PlacementConfig,
    /// A live offset while the user drags the pill (not persisted yet).
    preview_offset: Option<f64>,
    placement: Option<Placement>,
    /// The logical frame size most recently requested by the frontend.
    frame: LogicalSize,
    /// Window that was in the foreground before the island took focus, so it
    /// can be given focus back when the island is dismissed with the keyboard.
    previous_foreground: Option<isize>,
    /// Hidden from the tray ("Hide Islandy").
    user_hidden: bool,
    /// Hidden while a fullscreen app covers the island's monitor.
    fullscreen_hidden: bool,
}

impl WindowState {
    pub fn new(config: PlacementConfig, start_hidden: bool) -> Mutex<Self> {
        Mutex::new(Self {
            strategy: FrameStrategy::from_env(),
            config,
            preview_offset: None,
            placement: None,
            frame: geometry::size::COLLAPSED,
            previous_foreground: None,
            user_hidden: start_hidden,
            fullscreen_hidden: false,
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

    /// The configured monitor if it is connected, else the primary monitor,
    /// else the window's current monitor, else any monitor. Monitor
    /// coordinates come from Windows as-is (secondary monitors can be at
    /// negative positions; the primary one is not assumed to be at 0,0).
    fn target_monitor(&self, wanted: &str) -> Result<Monitor> {
        if wanted != PRIMARY_MONITOR {
            if let Some(monitor) = self
                .window
                .available_monitors()?
                .into_iter()
                .find(|m| m.name().is_some_and(|n| n == wanted))
            {
                return Ok(monitor);
            }
        }
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

    fn read_monitor(&self, wanted: &str) -> Result<MonitorGeometry> {
        let monitor = self.target_monitor(wanted)?;
        let work_area = monitor.work_area();
        let area = PhysicalRect {
            x: work_area.position.x,
            y: work_area.position.y,
            width: work_area.size.width,
            height: work_area.size.height,
        };
        let bounds = PhysicalRect {
            x: monitor.position().x,
            y: monitor.position().y,
            width: monitor.size().width,
            height: monitor.size().height,
        };
        Ok(MonitorGeometry {
            area,
            bounds,
            scale: monitor.scale_factor(),
            name: monitor.name().cloned(),
        })
    }

    /// Layouts fitted to the island's current monitor. Re-reads the monitor,
    /// so a resolution, scaling or taskbar change is picked up here.
    pub fn layouts(&self) -> Result<Layouts> {
        self.apply(false)?;
        let state = self.state();
        state
            .placement
            .as_ref()
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

    /// Re-reads the monitor and re-applies placement only if something
    /// changed (resolution, scaling, taskbar, monitor (dis)connected).
    pub fn refresh_if_changed(&self) -> Result<bool> {
        self.apply(false)
    }

    /// New user placement preferences; applied immediately.
    pub fn configure(&self, config: PlacementConfig) -> Result<()> {
        {
            let mut state = self.state();
            if state.config == config {
                return Ok(());
            }
            state.config = config;
            state.preview_offset = None;
        }
        self.apply(true).map(|_| ())
    }

    /// Moves the island to a temporary offset while the user drags it and
    /// returns the offset actually applied (clamped on screen). `None` ends
    /// the preview (the caller persists the final value).
    pub fn preview_offset(&self, offset: Option<f64>) -> Result<f64> {
        self.state().preview_offset = offset.filter(|o| o.is_finite());
        self.apply(true)?;
        let state = self.state();
        Ok(state.placement.as_ref().map_or(0.0, |p| p.applied_offset))
    }

    /// Monitors the island can be placed on.
    pub fn monitors(&self) -> Result<Vec<MonitorInfo>> {
        let primary = self
            .window
            .primary_monitor()?
            .and_then(|m| m.name().cloned());
        let current = self
            .state()
            .placement
            .as_ref()
            .and_then(|p| p.monitor.name.clone());
        Ok(self
            .window
            .available_monitors()?
            .into_iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let id = m.name()?.clone();
                let size = m.size();
                let is_primary = primary.as_deref() == Some(id.as_str());
                Some(MonitorInfo {
                    label: format!(
                        "Display {} · {}×{} · {}%",
                        i + 1,
                        size.width,
                        size.height,
                        (m.scale_factor() * 100.0).round()
                    ),
                    primary: is_primary,
                    width: size.width,
                    height: size.height,
                    scale: m.scale_factor(),
                    current: current.as_deref() == Some(id.as_str()),
                    id,
                })
            })
            .collect())
    }

    /// The whole monitor the island is on (physical px).
    pub fn monitor_bounds(&self) -> Option<PhysicalRect> {
        self.state().placement.as_ref().map(|p| p.monitor.bounds)
    }

    /// A short description for diagnostics (no names or titles).
    pub fn describe(&self) -> String {
        let state = self.state();
        match state.placement.as_ref() {
            Some(p) => format!(
                "{:?} on {}, work area {}×{} @ {}%, offset {}",
                state.config.anchor,
                if state.config.monitor == PRIMARY_MONITOR {
                    "primary monitor"
                } else {
                    "selected monitor"
                },
                p.monitor.area.width,
                p.monitor.area.height,
                (p.monitor.scale * 100.0).round(),
                p.applied_offset.round()
            ),
            None => "not placed".into(),
        }
    }

    fn apply(&self, force_placement: bool) -> Result<bool> {
        let wanted = self.state().config.monitor.clone();
        let monitor = self.read_monitor(&wanted)?;
        let plan = {
            let mut state = self.state();
            Self::plan(&mut state, monitor, force_placement)
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

    fn plan(state: &mut WindowState, monitor: MonitorGeometry, force: bool) -> FramePlan {
        let options = PlaceOptions {
            anchor: state.config.anchor,
            offset_x: state.preview_offset.unwrap_or(state.config.offset_x),
            top_margin: state.config.top_margin,
        };
        let pills = PillSizes::for_preset(state.config.size);
        let unchanged = state
            .placement
            .as_ref()
            .is_some_and(|p| p.monitor == monitor && p.options == options && p.pills == pills);
        let placement_changed = force || !unchanged;
        if placement_changed {
            let mut layouts = geometry::fit_layouts(
                geometry::logical_size_of(monitor.area, monitor.scale),
                options.top_margin,
                pills,
            );
            layouts.align = Align::from(options.anchor);
            let (canvas, applied_offset) =
                geometry::place(monitor.area, monitor.scale, layouts.canvas(), options);
            state.placement = Some(Placement {
                monitor,
                options,
                pills,
                layouts,
                canvas,
                applied_offset,
            });
        }
        let Some(p) = state.placement.as_ref() else {
            unreachable!("placement is set above");
        };
        let frame = state.frame.clamp_to(p.layouts.canvas());
        let align = p.layouts.align;
        match state.strategy {
            FrameStrategy::Region => FramePlan {
                move_canvas: placement_changed.then_some(p.canvas),
                region: Some(geometry::frame_region(
                    p.canvas.width,
                    p.canvas.height,
                    p.monitor.scale,
                    frame,
                    align,
                )),
                bounds: None,
                placement_changed,
            },
            FrameStrategy::Resize => FramePlan {
                move_canvas: None,
                region: None,
                bounds: Some(geometry::frame_bounds(
                    p.canvas,
                    p.monitor.scale,
                    frame,
                    align,
                )),
                placement_changed,
            },
        }
    }

    /* ------------------------------------------------------------------ */
    /* Visibility                                                          */
    /* ------------------------------------------------------------------ */

    pub fn is_user_hidden(&self) -> bool {
        self.state().user_hidden
    }

    /// Shows or hides the island from the tray. Returns the new visibility.
    pub fn set_user_visible(&self, visible: bool) -> Result<bool> {
        self.state().user_hidden = !visible;
        self.sync_visibility()
    }

    /// Hides the island while a fullscreen app covers its monitor.
    /// Returns `true` if this changed anything.
    pub fn set_fullscreen_hidden(&self, hidden: bool) -> Result<bool> {
        {
            let mut state = self.state();
            if state.fullscreen_hidden == hidden {
                return Ok(false);
            }
            state.fullscreen_hidden = hidden;
        }
        self.sync_visibility().map(|_| true)
    }

    fn sync_visibility(&self) -> Result<bool> {
        let visible = {
            let state = self.state();
            !state.user_hidden && !state.fullscreen_hidden
        };
        if visible {
            self.window.show()?;
            // tao may rewrite extended styles when showing.
            platform::configure_island_window(&self.window)?;
        } else {
            self.window.hide()?;
        }
        Ok(visible)
    }

    pub fn window_handle(&self) -> Option<isize> {
        platform::window_handle(&self.window)
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
                crate::log_warn!("window", "failed to change focus state: {e}");
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

    /// First show after start-up (respects "start hidden").
    pub fn show(&self) -> Result<()> {
        self.sync_visibility().map(|_| ())
    }
}
