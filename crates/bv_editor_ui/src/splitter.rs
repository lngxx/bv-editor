//! Resizable splitters between shell zones (docs/DESIGN.md section 9, Phase
//! 1: "resizable splitter ระหว่างโซน"). This is deliberately just resize —
//! moving/undocking panels ("docking" proper) is out of scope until Phase 10.
//!
//! The drag math itself ([`resize_value`]) is a pure function with no ECS or
//! rendering involved, so it has direct unit tests. [`splitter_drag_system`]
//! is the thin ECS wrapper that feeds real input into it.

use bevy_ecs::prelude::*;
use bevy_input::ButtonInput;
use bevy_input::mouse::{AccumulatedMouseMotion, MouseButton};
use bevy_ui::{Interaction, Node, Val};
use bevy_window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

/// Which dimension a splitter resizes on its target node.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitterAxis {
    /// Dragging left/right resizes the target's `width`.
    Horizontal,
    /// Dragging up/down resizes the target's `height`.
    Vertical,
}

/// Marks an entity as a draggable splitter bar. `target` is the sibling panel
/// whose size it controls; the splitter itself has no size opinion beyond
/// being thin and hoverable.
#[derive(Component, Clone, Copy, Debug)]
pub struct Splitter {
    pub target: Entity,
    pub axis: SplitterAxis,
    pub min_px: f32,
    pub max_px: f32,
    /// Whether `target` sits on the *far* side of the splitter from the
    /// origin of its axis — i.e. the splitter is `target`'s left/top edge
    /// rather than its right/bottom edge (e.g. the Components panel's
    /// splitter is its left edge; the bottom row's splitter is its top
    /// edge). For those, dragging right/down needs to *shrink* `target`
    /// rather than grow it, or the panel visibly moves opposite the mouse.
    pub invert: bool,
}

/// Pure resize math: clamp `current + delta` into `[min, max]`. No ECS, no
/// rendering — this is what the Phase 1 "unit test คำนวณ splitter/resize
/// logic ล้วนๆ" bullet is about.
pub fn resize_value(current_px: f32, delta_px: f32, min_px: f32, max_px: f32) -> f32 {
    (current_px + delta_px).clamp(min_px, max_px)
}

fn val_as_px(val: Val) -> Option<f32> {
    match val {
        Val::Px(px) => Some(px),
        _ => None,
    }
}

/// Which splitter (if any) is currently being dragged, shared with
/// [`splitter_cursor_system`] so the resize cursor (docs/UI_FEATURES.md F4)
/// stays locked to the drag's axis even if a fast drag momentarily carries
/// the pointer off the splitter's thin hit area (which would otherwise drop
/// its `Interaction` back to `None` mid-drag).
#[derive(Resource, Default)]
pub struct ActiveSplitterDrag(pub Option<Entity>);

/// Drives every [`Splitter`] from mouse input: press-and-drag a splitter bar
/// to resize its `target`. At most one splitter drags at a time.
pub fn splitter_drag_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    splitters: Query<(Entity, &Interaction, &Splitter)>,
    mut nodes: Query<&mut Node>,
    mut dragging: ResMut<ActiveSplitterDrag>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        dragging.0 = None;
    } else if dragging.0.is_none() {
        dragging.0 = splitters
            .iter()
            .find(|(_, interaction, _)| **interaction == Interaction::Pressed)
            .map(|(entity, ..)| entity);
    }

    let Some(active) = dragging.0 else { return };
    let Ok((_, _, splitter)) = splitters.get(active) else {
        dragging.0 = None;
        return;
    };

    let raw_delta = match splitter.axis {
        SplitterAxis::Horizontal => mouse_motion.delta.x,
        SplitterAxis::Vertical => mouse_motion.delta.y,
    };
    if raw_delta == 0.0 {
        return;
    }
    let delta = if splitter.invert { -raw_delta } else { raw_delta };

    let Ok(mut node) = nodes.get_mut(splitter.target) else {
        return;
    };
    let current = match splitter.axis {
        SplitterAxis::Horizontal => val_as_px(node.width),
        SplitterAxis::Vertical => val_as_px(node.height),
    }
    .unwrap_or(0.0);

    let new_value = resize_value(current, delta, splitter.min_px, splitter.max_px);
    match splitter.axis {
        SplitterAxis::Horizontal => node.width = Val::Px(new_value),
        SplitterAxis::Vertical => node.height = Val::Px(new_value),
    }
}

/// Pure mapping from a splitter's resize axis to the system cursor that
/// communicates it: `↔` for [`SplitterAxis::Horizontal`], `↕` for
/// [`SplitterAxis::Vertical`].
fn cursor_icon_for_axis(axis: SplitterAxis) -> SystemCursorIcon {
    match axis {
        SplitterAxis::Horizontal => SystemCursorIcon::EwResize,
        SplitterAxis::Vertical => SystemCursorIcon::NsResize,
    }
}

/// docs/UI_FEATURES.md F4: shows a resize cursor while hovering or dragging a
/// splitter, so the bar reads as draggable rather than just a thin coloured
/// strip. An active drag ([`ActiveSplitterDrag`]) takes priority over hover
/// so the cursor doesn't flicker back to default mid-drag; falls back to
/// whichever splitter (if any) is currently hovered; clears back to the
/// platform default once neither applies. No-ops under a headless app with
/// no primary window (e.g. tests).
///
/// Only clears `CursorIcon` when the *current* icon is one this system
/// would itself have set (`EwResize`/`NsResize`) — not unconditionally.
/// `bv_editor_ui::EditorUiPlugin` also runs
/// [`crate::scrollbar::scrollbar_cursor_system`] in the same `Update`
/// schedule, with no ordering constraint between the two (nothing requires
/// one), so on any given frame either could run last. An unconditional
/// `remove::<CursorIcon>()` here would then be a coin flip away from
/// wiping out a legitimately-hovered scrollbar thumb's `Grab` cursor
/// whenever no splitter happens to be hovered that same frame — this
/// system should only ever clean up after itself, never after a sibling
/// system it has no relationship with.
pub fn splitter_cursor_system(
    dragging: Res<ActiveSplitterDrag>,
    splitters: Query<(Entity, &Interaction, &Splitter)>,
    window: Query<(Entity, Option<&CursorIcon>), With<PrimaryWindow>>,
    mut commands: Commands,
) {
    let Ok((window, current_icon)) = window.single() else { return };

    let axis = dragging
        .0
        .and_then(|active| splitters.get(active).ok())
        .map(|(_, _, splitter)| splitter.axis)
        .or_else(|| {
            splitters
                .iter()
                .find(|(_, interaction, _)| matches!(interaction, Interaction::Hovered | Interaction::Pressed))
                .map(|(_, _, splitter)| splitter.axis)
        });

    match axis {
        Some(axis) => {
            commands.entity(window).insert(CursorIcon::System(cursor_icon_for_axis(axis)));
        }
        None if matches!(current_icon, Some(CursorIcon::System(SystemCursorIcon::EwResize)) | Some(CursorIcon::System(SystemCursorIcon::NsResize))) => {
            commands.entity(window).remove::<CursorIcon>();
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_moves_by_delta_within_bounds() {
        assert_eq!(resize_value(200.0, 50.0, 100.0, 400.0), 250.0);
        assert_eq!(resize_value(200.0, -50.0, 100.0, 400.0), 150.0);
    }

    #[test]
    fn resize_clamps_to_minimum() {
        assert_eq!(resize_value(120.0, -500.0, 100.0, 400.0), 100.0);
    }

    #[test]
    fn resize_clamps_to_maximum() {
        assert_eq!(resize_value(380.0, 500.0, 100.0, 400.0), 400.0);
    }

    #[test]
    fn zero_delta_is_a_no_op() {
        assert_eq!(resize_value(250.0, 0.0, 100.0, 400.0), 250.0);
    }

    #[test]
    fn cursor_icon_matches_axis() {
        assert_eq!(cursor_icon_for_axis(SplitterAxis::Horizontal), SystemCursorIcon::EwResize);
        assert_eq!(cursor_icon_for_axis(SplitterAxis::Vertical), SystemCursorIcon::NsResize);
    }
}
