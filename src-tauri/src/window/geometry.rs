//! Pure placement math. All inputs/outputs are explicit about pixel units:
//! sizes are requested in *logical* pixels (design tokens) and converted to
//! *physical* pixels with the target monitor's scale factor.
//!
//! Nothing here touches a window, so every rule is unit-tested.

use serde::Serialize;

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

    /// Resting pill. Keep in sync with the initial size in `tauri.conf.json`.
    pub const COLLAPSED: LogicalSize = LogicalSize::new(220.0, 44.0);
    /// The resting pill while ambient activity is shown (e.g. a Claude Code
    /// session working or waiting): wide enough for "project · status".
    pub const AMBIENT: LogicalSize = LogicalSize::new(300.0, 44.0);
    /// Temporary contextual pill (a wider, slightly taller capsule).
    pub const PEEK: LogicalSize = LogicalSize::new(360.0, 48.0);
    /// Expanded dashboard (also used by widget detail views).
    pub const EXPANDED: LogicalSize = LogicalSize::new(820.0, 248.0);
}

/// Gap between the top of the monitor's work area and the island, in logical px.
/// This is the single place to change the island's top offset (0–8 px suits the design).
pub const TOP_MARGIN: f64 = 8.0;

/// Minimum gap kept between the island and the work area's left/right/bottom edges.
pub const EDGE_MARGIN: f64 = 12.0;

/// The fitted logical size of every layout on one monitor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Layouts {
    pub collapsed: LogicalSize,
    pub ambient: LogicalSize,
    pub peek: LogicalSize,
    pub expanded: LogicalSize,
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
pub fn fit_layouts(area: LogicalSize, top_margin: f64) -> Layouts {
    let max_width = (area.width - 2.0 * EDGE_MARGIN).floor().max(1.0);
    let max_height = (area.height - top_margin.max(0.0) - EDGE_MARGIN)
        .floor()
        .max(1.0);
    let fit = |s: LogicalSize| LogicalSize::new(s.width.min(max_width), s.height.min(max_height));
    Layouts {
        collapsed: fit(size::COLLAPSED),
        ambient: fit(size::AMBIENT),
        peek: fit(size::PEEK),
        expanded: fit(size::EXPANDED),
    }
}

/// Computes the physical bounds that center `size` horizontally at the top of
/// `area`, scaled by `scale_factor`. The result is clamped to stay inside `area`.
pub fn top_center(
    area: PhysicalRect,
    scale_factor: f64,
    size: LogicalSize,
    top_margin: f64,
) -> PhysicalRect {
    let scale = sanitize_scale(scale_factor);
    let to_physical = |logical: f64| (logical * scale).round().max(1.0) as u32;

    let width = to_physical(size.width).min(area.width.max(1));
    let height = to_physical(size.height).min(area.height.max(1));
    let margin = (top_margin.max(0.0) * scale).round() as u32;

    let x = area.x + (area.width.saturating_sub(width) / 2) as i32;
    let max_margin = area.height.saturating_sub(height);
    let y = area.y + margin.min(max_margin) as i32;

    PhysicalRect {
        x,
        y,
        width,
        height,
    }
}

/// The hit/visible region, relative to a window of `window_width` ×
/// `window_height` physical px, occupied by a surface of logical size `frame`
/// that the frontend centers horizontally at the top of the window.
///
/// The region is one physical pixel wider on each side and one pixel taller
/// than the exact surface, to absorb rounding differences between CSS layout
/// and this calculation (otherwise a border pixel could be clipped).
pub fn frame_region(
    window_width: u32,
    window_height: u32,
    scale_factor: f64,
    frame: LogicalSize,
) -> PhysicalRect {
    let scale = sanitize_scale(scale_factor);
    let to_physical = |logical: f64| (logical.max(0.0) * scale).round() as u32;
    let width = (to_physical(frame.width) + 2).clamp(1, window_width.max(1));
    let height = (to_physical(frame.height) + 1).clamp(1, window_height.max(1));
    PhysicalRect {
        x: (window_width.saturating_sub(width) / 2) as i32,
        y: 0,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let l = fit_layouts(LogicalSize::new(1920.0, 1040.0), TOP_MARGIN);
        assert_eq!(l.collapsed, size::COLLAPSED);
        assert_eq!(l.peek, size::PEEK);
        assert_eq!(l.expanded, size::EXPANDED);
        assert_eq!(l.canvas(), size::EXPANDED);
    }

    #[test]
    fn expanded_layout_narrows_on_small_monitors() {
        // 1024 px at 150% scaling is ~683 logical px wide.
        let logical = logical_size_of(area(0, 0, 1024, 728), 1.5);
        let l = fit_layouts(logical, TOP_MARGIN);
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
        let l = fit_layouts(LogicalSize::new(300.0, 200.0), TOP_MARGIN);
        assert_eq!(l.expanded, LogicalSize::new(276.0, 180.0));
        let l = fit_layouts(LogicalSize::new(0.0, 0.0), TOP_MARGIN);
        assert_eq!(l.expanded, LogicalSize::new(1.0, 1.0));
    }

    #[test]
    fn frame_region_is_centered_and_padded() {
        // 820 x 248 canvas at 100%; collapsed pill in the middle.
        let r = frame_region(820, 248, 1.0, size::COLLAPSED);
        assert_eq!(r, area(299, 0, 222, 45));
        // At 150%: canvas 1230 x 372, pill 330 x 66.
        let r = frame_region(1230, 372, 1.5, size::COLLAPSED);
        assert_eq!(r, area(449, 0, 332, 67));
        // Full canvas is clamped to the window.
        let r = frame_region(820, 248, 1.0, size::EXPANDED);
        assert_eq!(r, area(0, 0, 820, 248));
    }

    #[test]
    fn frame_region_contains_the_css_centered_surface() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let canvas = top_center(area(0, 0, 3840, 2100), scale, size::EXPANDED, 0.0);
            for frame in [size::COLLAPSED, size::PEEK, LogicalSize::new(341.0, 47.0)] {
                let r = frame_region(canvas.width, canvas.height, scale, frame);
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
}
