//! Deterministic, platform-neutral geometry for overlay windows.
//!
//! Window placement is expressed as a semantic size and a stable anchor inside
//! a monitor's usable work area.  No calculation depends on the window's
//! previously reported size or position, so repeated layout applications are
//! idempotent and cannot accumulate DPI rounding drift.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WidgetLayout {
    Compact,
    Expanded,
}

impl WidgetLayout {
    pub(crate) fn from_expanded(expanded: bool) -> Self {
        if expanded {
            Self::Expanded
        } else {
            Self::Compact
        }
    }

    pub(crate) fn logical_size(self) -> LogicalSize {
        match self {
            Self::Compact => LogicalSize::new(56.0, 56.0),
            Self::Expanded => LogicalSize::new(224.0, 56.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WidgetAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    Center,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl WidgetAnchor {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "top-left" => Some(Self::TopLeft),
            "top-center" => Some(Self::TopCenter),
            "top-right" => Some(Self::TopRight),
            "center" => Some(Self::Center),
            "bottom-left" => Some(Self::BottomLeft),
            "bottom-center" => Some(Self::BottomCenter),
            "bottom-right" => Some(Self::BottomRight),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LogicalSize {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl LogicalSize {
    pub(crate) const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalRect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl PhysicalRect {
    pub(crate) const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

const WIDGET_EDGE_MARGIN_LOGICAL: f64 = 24.0;

fn valid_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn scaled_dimension(value: f64, scale: f64, maximum: u32) -> u32 {
    ((value.max(1.0) * valid_scale(scale)).round() as u64).clamp(1, maximum.max(1) as u64) as u32
}

fn clamp_i64(value: i64, min: i64, max: i64) -> i64 {
    if max < min {
        min
    } else {
        value.clamp(min, max)
    }
}

fn as_i32(value: i64) -> i32 {
    value.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// Place a logical-size window at a stable anchor inside a physical work area.
pub(crate) fn anchored_rect(
    work_area: PhysicalRect,
    logical_size: LogicalSize,
    scale: f64,
    anchor: WidgetAnchor,
    margin_logical: f64,
) -> PhysicalRect {
    let scale = valid_scale(scale);
    let width = scaled_dimension(logical_size.width, scale, work_area.width);
    let height = scaled_dimension(logical_size.height, scale, work_area.height);
    let margin = (margin_logical.max(0.0) * scale).round() as i64;

    let left = work_area.x as i64;
    let top = work_area.y as i64;
    let right = left + work_area.width as i64;
    let bottom = top + work_area.height as i64;
    let width_i64 = width as i64;
    let height_i64 = height as i64;

    let centered_x = left + (work_area.width as i64 - width_i64) / 2;
    let centered_y = top + (work_area.height as i64 - height_i64) / 2;
    let inset_left = left + margin;
    let inset_top = top + margin;
    let inset_right = right - width_i64 - margin;
    let inset_bottom = bottom - height_i64 - margin;

    let (raw_x, raw_y) = match anchor {
        WidgetAnchor::TopLeft => (inset_left, inset_top),
        WidgetAnchor::TopCenter => (centered_x, inset_top),
        WidgetAnchor::TopRight => (inset_right, inset_top),
        WidgetAnchor::Center => (centered_x, centered_y),
        WidgetAnchor::BottomLeft => (inset_left, inset_bottom),
        WidgetAnchor::BottomCenter => (centered_x, inset_bottom),
        WidgetAnchor::BottomRight => (inset_right, inset_bottom),
    };

    let x = clamp_i64(raw_x, left, right - width_i64);
    let y = clamp_i64(raw_y, top, bottom - height_i64);

    PhysicalRect::new(as_i32(x), as_i32(y), width, height)
}

pub(crate) fn widget_rect(
    work_area: PhysicalRect,
    scale: f64,
    layout: WidgetLayout,
    anchor: WidgetAnchor,
) -> PhysicalRect {
    anchored_rect(
        work_area,
        layout.logical_size(),
        scale,
        anchor,
        WIDGET_EDGE_MARGIN_LOGICAL,
    )
}

/// Place a panel above a widget when possible, otherwise below it, then clamp
/// the result to the same monitor work area.
pub(crate) fn adjacent_panel_rect(
    work_area: PhysicalRect,
    widget: PhysicalRect,
    panel_size: LogicalSize,
    scale: f64,
    gap_logical: f64,
    margin_logical: f64,
) -> PhysicalRect {
    let scale = valid_scale(scale);
    let width = scaled_dimension(panel_size.width, scale, work_area.width);
    let height = scaled_dimension(panel_size.height, scale, work_area.height);
    let gap = (gap_logical.max(0.0) * scale).round() as i64;
    let margin = (margin_logical.max(0.0) * scale).round() as i64;

    let work_left = work_area.x as i64;
    let work_top = work_area.y as i64;
    let work_right = work_left + work_area.width as i64;
    let work_bottom = work_top + work_area.height as i64;
    let width_i64 = width as i64;
    let height_i64 = height as i64;

    let widget_left = widget.x as i64;
    let widget_top = widget.y as i64;
    let widget_bottom = widget_top + widget.height as i64;
    let widget_center_x = widget_left + widget.width as i64 / 2;

    let raw_x = widget_center_x - width_i64 / 2;
    let above_y = widget_top - gap - height_i64;
    let below_y = widget_bottom + gap;
    let min_y = work_top + margin;
    let max_y = work_bottom - height_i64 - margin;

    let raw_y = if above_y >= min_y {
        above_y
    } else if below_y <= max_y {
        below_y
    } else {
        // Neither side fits with the preferred margin. Pick the side with more
        // available room before applying the final work-area clamp.
        let room_above = widget_top - work_top;
        let room_below = work_bottom - widget_bottom;
        if room_above >= room_below {
            above_y
        } else {
            below_y
        }
    };

    let x = clamp_i64(raw_x, work_left, work_right - width_i64);
    let y = clamp_i64(raw_y, work_top, work_bottom - height_i64);

    PhysicalRect::new(as_i32(x), as_i32(y), width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_center_is_stable_at_fractional_scale() {
        let work_area = PhysicalRect::new(0, 0, 2880, 1720);
        let rect = widget_rect(
            work_area,
            1.75,
            WidgetLayout::Expanded,
            WidgetAnchor::BottomCenter,
        );

        assert_eq!(rect, PhysicalRect::new(1244, 1580, 392, 98));
        assert_eq!(rect.x + rect.width as i32 / 2, 1440);
    }

    #[test]
    fn negative_monitor_origins_are_preserved() {
        let work_area = PhysicalRect::new(-1920, 32, 1920, 1048);
        let rect = widget_rect(
            work_area,
            1.0,
            WidgetLayout::Compact,
            WidgetAnchor::BottomRight,
        );

        assert_eq!(rect, PhysicalRect::new(-80, 1000, 56, 56));
    }

    #[test]
    fn work_area_not_full_monitor_controls_the_bottom_edge() {
        let work_area = PhysicalRect::new(0, 40, 2560, 1360);
        let rect = widget_rect(
            work_area,
            1.5,
            WidgetLayout::Expanded,
            WidgetAnchor::BottomCenter,
        );

        assert_eq!(rect.y + rect.height as i32, 1364);
        assert_eq!(1400 - (rect.y + rect.height as i32), 36);
    }

    #[test]
    fn every_anchor_is_inside_the_work_area() {
        let work_area = PhysicalRect::new(-300, -100, 600, 400);
        let anchors = [
            WidgetAnchor::TopLeft,
            WidgetAnchor::TopCenter,
            WidgetAnchor::TopRight,
            WidgetAnchor::Center,
            WidgetAnchor::BottomLeft,
            WidgetAnchor::BottomCenter,
            WidgetAnchor::BottomRight,
        ];

        for anchor in anchors {
            let rect = widget_rect(work_area, 1.25, WidgetLayout::Expanded, anchor);
            assert!(rect.x >= work_area.x);
            assert!(rect.y >= work_area.y);
            assert!(rect.x + rect.width as i32 <= work_area.x + work_area.width as i32);
            assert!(rect.y + rect.height as i32 <= work_area.y + work_area.height as i32);
        }
    }

    #[test]
    fn applying_the_same_layout_is_idempotent() {
        let work_area = PhysicalRect::new(100, 50, 1920, 1040);
        let first = widget_rect(
            work_area,
            1.5,
            WidgetLayout::Expanded,
            WidgetAnchor::BottomCenter,
        );
        let second = widget_rect(
            work_area,
            1.5,
            WidgetLayout::Expanded,
            WidgetAnchor::BottomCenter,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn adjacent_panel_flips_below_near_the_top() {
        let work_area = PhysicalRect::new(0, 0, 1920, 1080);
        let widget = PhysicalRect::new(848, 24, 224, 56);
        let panel = adjacent_panel_rect(
            work_area,
            widget,
            LogicalSize::new(320.0, 220.0),
            1.0,
            10.0,
            12.0,
        );

        assert_eq!(panel, PhysicalRect::new(800, 90, 320, 220));
    }

    #[test]
    fn oversized_windows_are_fitted_and_clamped() {
        let work_area = PhysicalRect::new(10, 20, 100, 80);
        let rect = anchored_rect(
            work_area,
            LogicalSize::new(520.0, 260.0),
            2.0,
            WidgetAnchor::BottomCenter,
            24.0,
        );

        assert_eq!(rect, PhysicalRect::new(10, 20, 100, 80));
    }
}
