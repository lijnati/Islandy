//! Pure placement math. All inputs/outputs are explicit about pixel units:
//! sizes are requested in *logical* pixels (design tokens) and converted to
//! *physical* pixels with the target monitor's scale factor.

/// A size in logical (DPI-independent) pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

/// Island size presets. Phase 1 adds `PEEK` / `EXPANDED` here.
pub mod size {
    use super::LogicalSize;

    /// Resting pill. Keep in sync with the initial size in `tauri.conf.json`.
    pub const COLLAPSED: LogicalSize = LogicalSize {
        width: 220.0,
        height: 44.0,
    };
}

/// Gap between the top of the monitor's work area and the island, in logical px.
pub const TOP_MARGIN: f64 = 8.0;

/// A rectangle in physical pixels, in virtual-desktop coordinates
/// (secondary monitors can have negative origins).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Computes the physical bounds that center `size` horizontally at the top of
/// `area`, scaled by `scale_factor`. The result is clamped to stay inside `area`.
pub fn top_center(
    area: PhysicalRect,
    scale_factor: f64,
    size: LogicalSize,
    top_margin: f64,
) -> PhysicalRect {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
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
}
