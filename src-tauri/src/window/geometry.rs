//! Pure placement math. All inputs/outputs are explicit about pixel units:
//! sizes are requested in *logical* pixels (design tokens) and converted to
//! *physical* pixels with the target monitor's scale factor.
//!
//! Nothing here touches a window, so every rule is unit-tested.

use serde::Serialize;

use crate::services::settings::models::{Anchor, SizePreset};

/// A size in logical (DPI-independent) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

impl LogicalSize {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    /// Component-wise maximum: the smallest size that contains both.
    pub fn union(self, other: Self) -> Self {
        Self::new(self.width.max(other.width), self.height.max(other.height))
    }

    /// Component-wise minimum.
    pub fn clamp_to(self, max: Self) -> Self {
        Self::new(self.width.min(max.width), self.height.min(max.height))
    }

    /// Validates a size received over IPC: finite and strictly positive.
    pub fn validated(width: f64, height: f64) -> Option<Self> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        (ok(width) && ok(height)).then_some(Self::new(width, height))
    }
}

/// Design-target sizes for each island layout. These are *targets*: the
/// sizes actually used are fitted to the monitor by [`fit_layouts`].
pub mod size {
    use super::LogicalSize;

    /// Resting pill (standard preset). Keep in sync with the initial size in
    /// `tauri.conf.json`.
    pub const COLLAPSED: LogicalSize = LogicalSize::new(220.0, 44.0);
    /// The resting pill while ambient activity is shown (e.g. a Claude Code
    /// session working or waiting): wide enough for "project · status".
    pub const AMBIENT: LogicalSize = LogicalSize::new(300.0, 44.0);
    /// Temporary contextual pill (a wider, slightly taller capsule).
    pub const PEEK: LogicalSize = LogicalSize::new(360.0, 48.0);
    /// Expanded dashboard (also used by widget detail views).
    pub const EXPANDED: LogicalSize = LogicalSize::new(820.0, 248.0);
}

/// The pill sizes of one size preset (the dashboard keeps its size).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PillSizes {
    pub collapsed: LogicalSize,
    pub ambient: LogicalSize,
    pub peek: LogicalSize,
}

impl PillSizes {
    pub fn for_preset(preset: SizePreset) -> Self {
        let s = |w, h| LogicalSize::new(w, h);
        match preset {
            SizePreset::Compact => Self {
                collapsed: s(200.0, 40.0),
                ambient: s(280.0, 40.0),
                peek: s(340.0, 44.0),
            },
            SizePreset::Standard => Self {
                collapsed: size::COLLAPSED,
                ambient: size::AMBIENT,
                peek: size::PEEK,
            },
            SizePreset::Comfortable => Self {
                collapsed: s(260.0, 48.0),
                ambient: s(330.0, 48.0),
                peek: s(390.0, 52.0),
            },
        }
    }
}

/// Default gap between the top of the monitor's work area and the island,
/// in logical px (user-configurable, see `IslandSettings::top_margin`).
pub const TOP_MARGIN: f64 = 8.0;

/// Minimum gap kept between the island and the work area's left/right/bottom edges.
pub const EDGE_MARGIN: f64 = 12.0;

/// How the visible surface is aligned inside the native canvas; follows the
/// anchor (a left-anchored island grows to the right).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    Start,
    #[default]
    Center,
    End,
}

impl From<Anchor> for Align {
    fn from(anchor: Anchor) -> Self {
        match anchor {
            Anchor::TopLeft => Align::Start,
            Anchor::TopCenter => Align::Center,
            Anchor::TopRight => Align::End,
        }
    }
}

/// The fitted logical size of every layout on one monitor, plus the
/// alignment the frontend uses to position the surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Layouts {
    pub collapsed: LogicalSize,
    pub ambient: LogicalSize,
    pub peek: LogicalSize,
    pub expanded: LogicalSize,
    pub align: Align,
}

impl Layouts {
    /// The smallest size containing every layout: the native canvas size.
    pub fn canvas(&self) -> LogicalSize {
        self.collapsed
            .union(self.ambient)
            .union(self.peek)
            .union(self.expanded)
    }
}

/// A rectangle in physical pixels, in virtual-desktop coordinates
/// (secondary monitors can have negative origins), or relative to a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

fn sanitize_scale(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

/// Converts a physical work area to its logical size.
pub fn logical_size_of(area: PhysicalRect, scale_factor: f64) -> LogicalSize {
    let scale = sanitize_scale(scale_factor);
    LogicalSize::new(
        f64::from(area.width) / scale,
        f64::from(area.height) / scale,
    )
}

/// Fits every design-target layout into a work area of `area` logical px.
///
/// Layouts shrink (never grow) so they keep [`EDGE_MARGIN`] from the sides
/// and bottom and `top_margin` from the top. Widths are whole logical pixels
/// so CSS layout and native geometry agree.
pub fn fit_layouts(area: LogicalSize, top_margin: f64, pills: PillSizes) -> Layouts {
    let max_width = (area.width - 2.0 * EDGE_MARGIN).floor().max(1.0);
    let max_height = (area.height - top_margin.max(0.0) - EDGE_MARGIN)
        .floor()
        .max(1.0);
    let fit = |s: LogicalSize| LogicalSize::new(s.width.min(max_width), s.height.min(max_height));
    Layouts {
        collapsed: fit(pills.collapsed),
        ambient: fit(pills.ambient),
        peek: fit(pills.peek),
        expanded: fit(size::EXPANDED),
        align: Align::Center,
    }
}

/// Where to put the island on its monitor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaceOptions {
    pub anchor: Anchor,
    /// Logical px from the anchor position (positive = right).
    pub offset_x: f64,
    /// Logical px below the top of the work area.
    pub top_margin: f64,
}

impl Default for PlaceOptions {
    fn default() -> Self {
        Self {
            anchor: Anchor::TopCenter,
            offset_x: 0.0,
            top_margin: TOP_MARGIN,
        }
    }
}

/// Physical bounds for `size` at the top of `area`, positioned by `options`
/// and clamped so it stays [`EDGE_MARGIN`] inside the work area (or centered
/// if it can't). Returns the bounds and the offset actually applied (logical
/// px, after clamping).
pub fn place(
    area: PhysicalRect,
    scale_factor: f64,
    size: LogicalSize,
    options: PlaceOptions,
) -> (PhysicalRect, f64) {
    let scale = sanitize_scale(scale_factor);
    let to_physical = |logical: f64| (logical * scale).round().max(1.0) as u32;

    let width = to_physical(size.width).min(area.width.max(1));
    let height = to_physical(size.height).min(area.height.max(1));
    let margin = (options.top_margin.max(0.0) * scale).round() as u32;
    let free = area.width.saturating_sub(width) as i32;
    let edge = ((EDGE_MARGIN * scale).round() as i32).min(free / 2);

    let base = match options.anchor {
        Anchor::TopCenter => area.x + free / 2,
        Anchor::TopLeft => area.x + edge,
        Anchor::TopRight => area.x + free - edge,
    };
    let offset = if options.offset_x.is_finite() {
        (options.offset_x * scale).round().clamp(-1e7, 1e7) as i32
    } else {
        0
    };
    let x = (base + offset).clamp(area.x + edge, area.x + free - edge);
    let max_margin = area.height.saturating_sub(height);
    let y = area.y + margin.min(max_margin) as i32;
    let rect = PhysicalRect {
        x,
        y,
        width,
        height,
    };
    (rect, f64::from(x - base) / scale)
}

/// Bounds that center `size` horizontally at the top of `area` (the default
/// placement), clamped inside `area`.
#[cfg(test)]
pub fn top_center(
    area: PhysicalRect,
    scale_factor: f64,
    size: LogicalSize,
    top_margin: f64,
) -> PhysicalRect {
    place(
        area,
        scale_factor,
        size,
        PlaceOptions {
            anchor: Anchor::TopCenter,
            offset_x: 0.0,
            top_margin,
        },
    )
    .0
}

/// The hit/visible region, relative to a window of `window_width` ×
/// `window_height` physical px, occupied by a surface of logical size `frame`
/// that the frontend aligns (`align`) at the top of the window.
///
/// The region is one physical pixel wider on each side and one pixel taller
/// than the exact surface, to absorb rounding differences between CSS layout
/// and this calculation (otherwise a border pixel could be clipped).
pub fn frame_region(
    window_width: u32,
    window_height: u32,
    scale_factor: f64,
    frame: LogicalSize,
    align: Align,
) -> PhysicalRect {
    let scale = sanitize_scale(scale_factor);
    let to_physical = |logical: f64| (logical.max(0.0) * scale).round() as u32;
    let width = (to_physical(frame.width) + 2).clamp(1, window_width.max(1));
    let height = (to_physical(frame.height) + 1).clamp(1, window_height.max(1));
    let x = match align {
        Align::Start => 0,
        Align::Center => window_width.saturating_sub(width) / 2,
        Align::End => window_width.saturating_sub(width),
    };
    PhysicalRect {
        x: x as i32,
        y: 0,
        width,
        height,
    }
}

/// Bounds of a native window sized exactly to `frame`, aligned inside
/// `canvas` like the surface would be (used by the resize strategy).
pub fn frame_bounds(
    canvas: PhysicalRect,
    scale_factor: f64,
    frame: LogicalSize,
    align: Align,
) -> PhysicalRect {
    let scale = sanitize_scale(scale_factor);
    let to_physical = |logical: f64| (logical.max(0.0) * scale).round().max(1.0) as u32;
    let width = to_physical(frame.width).min(canvas.width.max(1));
    let height = to_physical(frame.height).min(canvas.height.max(1));
    let x = match align {
        Align::Start => 0,
        Align::Center => canvas.width.saturating_sub(width) / 2,
        Align::End => canvas.width.saturating_sub(width),
    };
    PhysicalRect {
        x: canvas.x + x as i32,
        y: canvas.y,
        width,
        height,
    }
}

/// Whether a window rectangle (`left, top, right, bottom`) covers the whole
/// `monitor`: the signature of a fullscreen window.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn covers(window: (i32, i32, i32, i32), monitor: PhysicalRect) -> bool {
    let (left, top, right, bottom) = window;
    monitor.width > 0
        && monitor.height > 0
        && left <= monitor.x
        && top <= monitor.y
        && right >= monitor.x + monitor.width as i32
        && bottom >= monitor.y + monitor.height as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard() -> PillSizes {
        PillSizes::for_preset(SizePreset::Standard)
    }

    fn opts(anchor: Anchor, offset_x: f64) -> PlaceOptions {
        PlaceOptions {
            anchor,
            offset_x,
            top_margin: TOP_MARGIN,
        }
    }

    const fn area(x: i32, y: i32, width: u32, height: u32) -> PhysicalRect {
        PhysicalRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn centers_at_100_percent() {
        let r = top_center(area(0, 0, 1920, 1040), 1.0, size::COLLAPSED, TOP_MARGIN);
        assert_eq!(r, area(850, 8, 220, 44));
    }

    #[test]
    fn scales_to_physical_pixels_at_150_percent() {
        let r = top_center(area(0, 0, 2560, 1400), 1.5, size::COLLAPSED, TOP_MARGIN);
        assert_eq!(r, area(1115, 12, 330, 66));
    }

    #[test]
    fn rounds_fractional_scales() {
        // 125%: 220 * 1.25 = 275, 44 * 1.25 = 55, 8 * 1.25 = 10
        let r = top_center(area(0, 0, 1920, 1032), 1.25, size::COLLAPSED, TOP_MARGIN);
        assert_eq!(r, area(822, 10, 275, 55));
        // 175%: 385 x 77, margin 14
        let r = top_center(area(0, 0, 3840, 2100), 1.75, size::COLLAPSED, TOP_MARGIN);
        assert_eq!(r, area(1727, 14, 385, 77));
    }

    #[test]
    fn respects_work_area_offset_and_negative_origins() {
        // Monitor left of the primary, with a top-docked taskbar (work area starts at y=48).
        let r = top_center(
            area(-1920, 48, 1920, 1032),
            1.0,
            size::COLLAPSED,
            TOP_MARGIN,
        );
        assert_eq!(r, area(-1070, 56, 220, 44));
    }

    #[test]
    fn expanded_grows_outward_from_the_same_center() {
        for (a, scale) in [
            (area(0, 0, 1920, 1040), 1.0),
            (area(-2560, -300, 2560, 1400), 1.5),
            (area(1920, 0, 3840, 2100), 1.75),
        ] {
            let collapsed = top_center(a, scale, size::COLLAPSED, TOP_MARGIN);
            let expanded = top_center(a, scale, size::EXPANDED, TOP_MARGIN);
            let center = |r: PhysicalRect| 2 * r.x + r.width as i32;
            // Centers agree to within one physical pixel (integer rounding).
            assert!(
                (center(collapsed) - center(expanded)).abs() <= 1,
                "{a:?} @ {scale}"
            );
            assert_eq!(collapsed.y, expanded.y, "top edge stays anchored");
            assert!(
                expanded.x >= a.x && expanded.x + expanded.width as i32 <= a.x + a.width as i32
            );
        }
    }

    #[test]
    fn clamps_to_tiny_areas() {
        let r = top_center(area(10, 10, 100, 30), 1.0, size::COLLAPSED, TOP_MARGIN);
        assert_eq!(r, area(10, 10, 100, 30));
    }

    #[test]
    fn survives_degenerate_zero_sized_areas() {
        let r = top_center(area(0, 0, 0, 0), 2.0, size::COLLAPSED, TOP_MARGIN);
        assert_eq!(r, area(0, 0, 1, 1));
    }

    #[test]
    fn treats_invalid_scale_as_one() {
        for scale in [0.0, -2.0, f64::NAN, f64::INFINITY] {
            let r = top_center(area(0, 0, 1920, 1080), scale, size::COLLAPSED, TOP_MARGIN);
            assert_eq!((r.width, r.height), (220, 44));
        }
    }

    #[test]
    fn layouts_keep_design_targets_on_large_monitors() {
        let l = fit_layouts(LogicalSize::new(1920.0, 1040.0), TOP_MARGIN, standard());
        assert_eq!(l.collapsed, size::COLLAPSED);
        assert_eq!(l.peek, size::PEEK);
        assert_eq!(l.expanded, size::EXPANDED);
        assert_eq!(l.canvas(), size::EXPANDED);
    }

    #[test]
    fn expanded_layout_narrows_on_small_monitors() {
        // 1024 px at 150% scaling is ~683 logical px wide.
        let logical = logical_size_of(area(0, 0, 1024, 728), 1.5);
        let l = fit_layouts(logical, TOP_MARGIN, standard());
        assert_eq!(l.expanded.width, (1024.0f64 / 1.5 - 24.0).floor());
        assert_eq!(l.expanded.height, size::EXPANDED.height);
        assert_eq!(l.collapsed, size::COLLAPSED, "small layouts are untouched");
        assert_eq!(l.ambient, size::AMBIENT);
        // And it stays on screen once converted back to physical pixels.
        let r = top_center(area(0, 0, 1024, 728), 1.5, l.expanded, TOP_MARGIN);
        assert!(r.x >= 12 && r.x + r.width as i32 <= 1024 - 12);
    }

    #[test]
    fn layouts_never_exceed_short_or_tiny_work_areas() {
        let l = fit_layouts(LogicalSize::new(300.0, 200.0), TOP_MARGIN, standard());
        assert_eq!(l.expanded, LogicalSize::new(276.0, 180.0));
        let l = fit_layouts(LogicalSize::new(0.0, 0.0), TOP_MARGIN, standard());
        assert_eq!(l.expanded, LogicalSize::new(1.0, 1.0));
    }

    #[test]
    fn frame_region_is_centered_and_padded() {
        // 820 x 248 canvas at 100%; collapsed pill in the middle.
        let r = frame_region(820, 248, 1.0, size::COLLAPSED, Align::Center);
        assert_eq!(r, area(299, 0, 222, 45));
        // At 150%: canvas 1230 x 372, pill 330 x 66.
        let r = frame_region(1230, 372, 1.5, size::COLLAPSED, Align::Center);
        assert_eq!(r, area(449, 0, 332, 67));
        // Full canvas is clamped to the window.
        let r = frame_region(820, 248, 1.0, size::EXPANDED, Align::Center);
        assert_eq!(r, area(0, 0, 820, 248));
    }

    #[test]
    fn frame_region_contains_the_css_centered_surface() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let canvas = top_center(area(0, 0, 3840, 2100), scale, size::EXPANDED, 0.0);
            for frame in [size::COLLAPSED, size::PEEK, LogicalSize::new(341.0, 47.0)] {
                let r = frame_region(canvas.width, canvas.height, scale, frame, Align::Center);
                // CSS centers the surface at (canvas - frame) / 2 in physical px.
                let left = (f64::from(canvas.width) - frame.width * scale) / 2.0;
                let right = left + frame.width * scale;
                assert!(f64::from(r.x) <= left.floor(), "{scale} {frame:?}");
                assert!(
                    f64::from(r.x) + f64::from(r.width) >= right.ceil(),
                    "{scale} {frame:?}"
                );
                assert!(f64::from(r.height) >= (frame.height * scale).ceil());
            }
        }
    }

    #[test]
    fn validates_ipc_sizes() {
        assert!(LogicalSize::validated(220.0, 44.0).is_some());
        for (w, h) in [
            (0.0, 44.0),
            (-1.0, 44.0),
            (f64::NAN, 44.0),
            (220.0, f64::INFINITY),
        ] {
            assert!(LogicalSize::validated(w, h).is_none());
        }
    }

    #[test]
    fn anchors_place_the_canvas_at_the_edges_with_a_margin() {
        let a = area(0, 0, 1920, 1040);
        let (left, off) = place(a, 1.0, size::EXPANDED, opts(Anchor::TopLeft, 0.0));
        assert_eq!((left.x, off), (12, 0.0));
        let (right, _) = place(a, 1.0, size::EXPANDED, opts(Anchor::TopRight, 0.0));
        assert_eq!(right.x + right.width as i32, 1920 - 12);
        // 150% on a monitor left of the primary (negative origin).
        let b = area(-2560, -200, 2560, 1400);
        let (left, _) = place(b, 1.5, size::EXPANDED, opts(Anchor::TopLeft, 0.0));
        assert_eq!(left.x, -2560 + 18);
        let (right, _) = place(b, 1.5, size::EXPANDED, opts(Anchor::TopRight, 0.0));
        assert_eq!(right.x + right.width as i32, -18);
        assert_eq!(right.y, -200 + 12);
    }

    #[test]
    fn offsets_move_the_island_and_are_clamped_on_screen() {
        let a = area(0, 0, 1920, 1040);
        let (r, applied) = place(a, 1.0, size::EXPANDED, opts(Anchor::TopCenter, -100.0));
        assert_eq!((r.x, applied), (450, -100.0));
        // Far beyond the edge: clamped 12 px inside, and the applied offset says so.
        let (r, applied) = place(a, 1.0, size::EXPANDED, opts(Anchor::TopCenter, 5000.0));
        assert_eq!(r.x + r.width as i32, 1908);
        assert_eq!(applied, 538.0);
        let (r, applied) = place(a, 1.25, size::EXPANDED, opts(Anchor::TopLeft, -40.0));
        assert_eq!((r.x, applied), (15, 0.0), "can't go left of the left edge");
        // Non-finite offsets are ignored.
        let (r, _) = place(a, 1.0, size::EXPANDED, opts(Anchor::TopCenter, f64::NAN));
        assert_eq!(r.x, 550);
    }

    #[test]
    fn island_stays_inside_every_monitor_layout() {
        let monitors = [
            (area(0, 0, 1920, 1040), 1.0),
            (area(0, 0, 1920, 1032), 1.25),
            (area(-1920, 48, 1920, 1032), 1.0),
            (area(1920, -360, 2560, 1400), 1.5),
            (area(3840, 0, 1024, 728), 1.5),
            (area(0, 0, 800, 560), 1.75),
        ];
        for (a, scale) in monitors {
            for anchor in [Anchor::TopLeft, Anchor::TopCenter, Anchor::TopRight] {
                for offset in [-3000.0, -50.0, 0.0, 50.0, 3000.0] {
                    let layouts = fit_layouts(logical_size_of(a, scale), TOP_MARGIN, standard());
                    let (r, _) = place(a, scale, layouts.canvas(), opts(anchor, offset));
                    assert!(r.x >= a.x, "{a:?} {scale} {anchor:?} {offset}");
                    assert!(r.x + r.width as i32 <= a.x + a.width as i32);
                    assert!(r.y >= a.y && r.y + r.height as i32 <= a.y + a.height as i32);
                }
            }
        }
    }

    #[test]
    fn size_presets_keep_proportions() {
        for preset in [
            SizePreset::Compact,
            SizePreset::Standard,
            SizePreset::Comfortable,
        ] {
            let p = PillSizes::for_preset(preset);
            assert!(p.ambient.width > p.collapsed.width);
            assert_eq!(p.ambient.height, p.collapsed.height);
            assert!(p.peek.width > p.ambient.width && p.peek.height > p.collapsed.height);
            let l = fit_layouts(LogicalSize::new(1920.0, 1040.0), TOP_MARGIN, p);
            assert_eq!(l.canvas(), size::EXPANDED, "the canvas stays the dashboard");
        }
        assert_eq!(
            PillSizes::for_preset(SizePreset::Compact).collapsed,
            LogicalSize::new(200.0, 40.0)
        );
        assert_eq!(
            PillSizes::for_preset(SizePreset::Comfortable).collapsed,
            LogicalSize::new(260.0, 48.0)
        );
    }

    #[test]
    fn regions_follow_the_alignment() {
        assert_eq!(
            frame_region(820, 248, 1.0, size::COLLAPSED, Align::Start),
            area(0, 0, 222, 45)
        );
        assert_eq!(
            frame_region(820, 248, 1.0, size::COLLAPSED, Align::End),
            area(598, 0, 222, 45)
        );
        let canvas = area(100, 8, 820, 248);
        assert_eq!(
            frame_bounds(canvas, 1.0, size::COLLAPSED, Align::End),
            area(700, 8, 220, 44)
        );
        assert_eq!(
            frame_bounds(canvas, 1.0, size::COLLAPSED, Align::Center),
            area(400, 8, 220, 44)
        );
        assert_eq!(
            frame_bounds(canvas, 1.0, size::COLLAPSED, Align::Start),
            area(100, 8, 220, 44)
        );
    }

    #[test]
    fn detects_windows_covering_a_monitor() {
        let m = area(-1920, 0, 1920, 1080);
        assert!(covers((-1920, 0, 0, 1080), m));
        assert!(covers((-1928, -8, 8, 1088), m), "invisible borders");
        assert!(
            !covers((-1920, 0, 0, 1040), m),
            "work area only (maximized)"
        );
        assert!(!covers((0, 0, 1920, 1080), m), "another monitor");
        assert!(!covers((0, 0, 0, 0), area(0, 0, 0, 0)));
    }
}
