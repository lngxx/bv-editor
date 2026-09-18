//! A small, generic drag-and-drop framework (docs/DESIGN.md section 8.5):
//! reparenting a Scene Tree row (Phase 2), dragging an asset into the
//! viewport or onto an inspector field (Phase 5), and reordering something
//! else later all want "pick this up, carry a payload, drop it on a target"
//! — this exists so each of those is a couple of marker components instead
//! of its own mouse-drag state machine.
//!
//! Built on the same `Interaction` + `ButtonInput<MouseButton>` primitives as
//! [`crate::splitter`], not `bevy_picking`'s pointer-drag events — consistent
//! with the rest of Phase 1/2, and it works the same way in a headless test
//! (set `Interaction` directly) as it does under a real cursor.

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bevy_input::ButtonInput;
use bevy_input::mouse::MouseButton;
use bevy_ui::Interaction;

/// What's being dragged. An enum rather than `Box<dyn Any>` (section 8.5
/// allows either) — no downcasting, just add a variant per new payload kind
/// as later phases need one (e.g. an asset handle in Phase 5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragPayload {
    /// Dragging a Scene Tree row: the entity it represents (Phase 2).
    Entity(Entity),
}

/// Marks a UI node that can be picked up and dragged, carrying `payload`.
#[derive(Component, Clone, Copy, Debug)]
pub struct DragSource {
    pub payload: DragPayload,
}

/// Marks a UI node as a place a drag can be dropped onto.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct DropTarget;

/// What's currently being dragged, and which [`DropTarget`] it's hovering,
/// if any. Read this (rather than raw input) to show drop-target highlight
/// styling while a drag is in progress.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DragState {
    pub payload: Option<DragPayload>,
    pub hovered_target: Option<Entity>,
}

/// Fired the frame a drag is released over a [`DropTarget`]. Add a system
/// after [`drag_and_drop_system`] (e.g. via `.chain()`, as
/// `bv_editor_scene_panel` does for reparenting) that reads this to act on
/// completed drops.
#[derive(Message, Clone, Copy, Debug)]
pub struct DragDropped {
    pub payload: DragPayload,
    pub target: Entity,
}

/// Drives [`DragState`] and emits [`DragDropped`] from mouse input:
/// press-and-hold a [`DragSource`], move over a [`DropTarget`], release to
/// drop. At most one drag is tracked at a time.
pub fn drag_and_drop_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    sources: Query<(&Interaction, &DragSource)>,
    targets: Query<(Entity, &Interaction), With<DropTarget>>,
    mut state: ResMut<DragState>,
    mut dropped: MessageWriter<DragDropped>,
) {
    if mouse_buttons.just_pressed(MouseButton::Left) && state.payload.is_none() {
        state.payload = sources
            .iter()
            .find(|(interaction, _)| **interaction == Interaction::Pressed)
            .map(|(_, source)| source.payload);
    }

    if state.payload.is_some() {
        state.hovered_target = targets
            .iter()
            .find(|(_, interaction)| matches!(interaction, Interaction::Hovered | Interaction::Pressed))
            .map(|(entity, _)| entity);
    }

    if mouse_buttons.just_released(MouseButton::Left) {
        if let (Some(payload), Some(target)) = (state.payload, state.hovered_target) {
            dropped.write(DragDropped { payload, target });
        }
        state.payload = None;
        state.hovered_target = None;
    }
}

/// Registers the drag-and-drop framework's resource, message, and driving
/// system. [`crate::EditorUiPlugin`] adds this, but so does anything that
/// *consumes* [`DragDropped`] (`bv_editor_scene_panel`'s `ScenePanelPlugin`,
/// from Phase 2 on) — Bevy panics if the exact same plugin is added twice,
/// so a consumer should guard with `app.is_plugin_added::<DragAndDropPlugin>()`
/// first. That's what lets an extension crate depend on this framework and
/// still work on its own (e.g. in its own tests) without the full shell.
#[derive(Default)]
pub struct DragAndDropPlugin;

impl Plugin for DragAndDropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DragState>();
        app.add_message::<DragDropped>();
        app.add_systems(Update, drag_and_drop_system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::MessageCursor;

    fn setup() -> (bevy_ecs::world::World, Entity, Entity) {
        let mut world = bevy_ecs::world::World::new();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<DragState>();
        world.init_resource::<Messages<DragDropped>>();

        let source = world
            .spawn((Interaction::default(), DragSource { payload: DragPayload::Entity(Entity::PLACEHOLDER) }))
            .id();
        let target = world.spawn((Interaction::default(), DropTarget)).id();
        (world, source, target)
    }

    use bevy_ecs::message::Messages;

    #[test]
    fn press_on_source_starts_the_drag() {
        let (mut world, source, _target) = setup();
        world.entity_mut(source).insert(Interaction::Pressed);
        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(drag_and_drop_system);
        schedule.run(&mut world);

        assert_eq!(world.resource::<DragState>().payload, Some(DragPayload::Entity(Entity::PLACEHOLDER)));
    }

    #[test]
    fn hovering_a_target_mid_drag_is_tracked() {
        let (mut world, source, target) = setup();
        world.entity_mut(source).insert(Interaction::Pressed);
        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(drag_and_drop_system);
        schedule.run(&mut world);

        world.entity_mut(target).insert(Interaction::Hovered);
        schedule.run(&mut world);

        assert_eq!(world.resource::<DragState>().hovered_target, Some(target));
    }

    #[test]
    fn releasing_over_a_target_emits_drag_dropped_and_clears_state() {
        let (mut world, source, target) = setup();
        world.entity_mut(source).insert(Interaction::Pressed);
        world.entity_mut(target).insert(Interaction::Hovered);
        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(drag_and_drop_system);
        schedule.run(&mut world);

        world.resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
        schedule.run(&mut world);

        let mut cursor = MessageCursor::<DragDropped>::default();
        let events: Vec<_> = cursor.read(world.resource::<Messages<DragDropped>>()).collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target, target);
        assert_eq!(events[0].payload, DragPayload::Entity(Entity::PLACEHOLDER));

        let state = world.resource::<DragState>();
        assert_eq!(state.payload, None);
        assert_eq!(state.hovered_target, None);
    }

    #[test]
    fn releasing_without_a_hovered_target_drops_nothing() {
        let (mut world, source, _target) = setup();
        world.entity_mut(source).insert(Interaction::Pressed);
        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(drag_and_drop_system);
        schedule.run(&mut world);

        world.resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
        schedule.run(&mut world);

        let mut cursor = MessageCursor::<DragDropped>::default();
        let events: Vec<_> = cursor.read(world.resource::<Messages<DragDropped>>()).collect();
        assert!(events.is_empty());
    }
}
