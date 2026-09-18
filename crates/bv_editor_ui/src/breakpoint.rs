//! Pure, render-independent layout sizing — deliberately has no dependency on
//! `bevy_ui`/`bevy_window` types so it can be unit tested without an `App` at
//! all (docs/DESIGN.md section 9, Phase 1 test bullet: "unit test คำนวณ
//! splitter/resize logic ล้วนๆ ไม่พึ่ง render").
//!
//! Phase 1 keeps this to picking the *initial* side-panel width from the
//! window's width at startup. It does not fight the user's own drag-resize
//! afterward — reacting continuously to window resizes is not part of the
//! Phase 1 scope ("fixed layout first").

/// A coarse window-width bucket the shell picks its initial panel sizes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayoutBreakpoint {
    /// Narrow window (e.g. a laptop half-screen). Side panels start at their
    /// minimum usable width so the viewport still gets most of the space.
    Compact,
    /// A normal desktop window.
    Normal,
    /// A wide/high-resolution monitor.
    Wide,
}

const COMPACT_MAX_WIDTH: f32 = 900.0;
const NORMAL_MAX_WIDTH: f32 = 1400.0;

/// Classify a window width into a [`LayoutBreakpoint`].
pub fn breakpoint_for_width(window_width: f32) -> LayoutBreakpoint {
    if window_width < COMPACT_MAX_WIDTH {
        LayoutBreakpoint::Compact
    } else if window_width < NORMAL_MAX_WIDTH {
        LayoutBreakpoint::Normal
    } else {
        LayoutBreakpoint::Wide
    }
}

/// Initial width (in px) for the left/right side panels at a given breakpoint.
pub fn side_panel_width_px(breakpoint: LayoutBreakpoint) -> f32 {
    match breakpoint {
        LayoutBreakpoint::Compact => 180.0,
        LayoutBreakpoint::Normal => 260.0,
        LayoutBreakpoint::Wide => 320.0,
    }
}

/// Initial height (in px) for the bottom panel row at a given breakpoint.
pub fn bottom_panel_height_px(breakpoint: LayoutBreakpoint) -> f32 {
    match breakpoint {
        LayoutBreakpoint::Compact => 120.0,
        LayoutBreakpoint::Normal => 180.0,
        LayoutBreakpoint::Wide => 220.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_widths() {
        assert_eq!(breakpoint_for_width(640.0), LayoutBreakpoint::Compact);
        assert_eq!(breakpoint_for_width(1024.0), LayoutBreakpoint::Normal);
        assert_eq!(breakpoint_for_width(1920.0), LayoutBreakpoint::Wide);
    }

    #[test]
    fn boundaries_are_exclusive_on_the_lower_bucket() {
        // Right at a boundary, the wider bucket wins (`<`, not `<=`, against the ceiling).
        assert_eq!(breakpoint_for_width(899.9), LayoutBreakpoint::Compact);
        assert_eq!(breakpoint_for_width(900.0), LayoutBreakpoint::Normal);
        assert_eq!(breakpoint_for_width(1399.9), LayoutBreakpoint::Normal);
        assert_eq!(breakpoint_for_width(1400.0), LayoutBreakpoint::Wide);
    }

    #[test]
    fn wider_breakpoints_get_more_generous_panels() {
        for f in [side_panel_width_px as fn(LayoutBreakpoint) -> f32, bottom_panel_height_px] {
            assert!(f(LayoutBreakpoint::Compact) < f(LayoutBreakpoint::Normal));
            assert!(f(LayoutBreakpoint::Normal) < f(LayoutBreakpoint::Wide));
        }
    }
}
