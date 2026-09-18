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

/// Drives every [`Splitter`] from mouse input: press-and-drag a splitter bar
/// to resize its `target`. At most one splitter drags at a time.
pub fn splitter_drag_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    splitters: Query<(Entity, &Interaction, &Splitter)>,
    mut nodes: Query<&mut Node>,
    mut dragging: Local<Option<Entity>>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        *dragging = None;
    } else if dragging.is_none() {
        *dragging = splitters
            .iter()
            .find(|(_, interaction, _)| **interaction == Interaction::Pressed)
            .map(|(entity, ..)| entity);
    }

    let Some(active) = *dragging else { return };
    let Ok((_, _, splitter)) = splitters.get(active) else {
        *dragging = None;
        return;
    };

    let delta = match splitter.axis {
        SplitterAxis::Horizontal => mouse_motion.delta.x,
        SplitterAxis::Vertical => mouse_motion.delta.y,
    };
    if delta == 0.0 {
        return;
    }

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
}
