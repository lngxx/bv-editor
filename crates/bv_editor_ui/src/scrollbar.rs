//! A hand-rolled vertical scrollbar (docs/UI_FEATURES.md F2/F3).
//!
//! `bevy_ui` already has everything needed for the *clipping/content-window*
//! half of scrolling built in — `Overflow::scroll_y()` + `ScrollPosition`
//! (a `Node` gets a default `ScrollPosition` for free; that part is used
//! directly here, "built-in first" per docs/DESIGN.md section 4. What it
//! doesn't have is a draggable scrollbar *thumb* widget.
//!
//! `bevy_ui_widgets` (pinned to the same Bevy version, docs/DESIGN.md
//! section 4) does ship one (`bevy_ui_widgets::scrollbar`), but it's built
//! entirely on `bevy_picking`'s `Pointer<Press>`/`Pointer<Drag>`/
//! `Pointer<Scroll>` observer events, not the `Interaction` +
//! `ButtonInput`/`AccumulatedMouseMotion` primitives every other widget in
//! this crate uses ([`crate::splitter`], [`crate::dnd`]). Pulling it in
//! would mean two different input paradigms doing the same drag gesture,
//! and — same reasoning as [`crate::dnd`]'s module doc — it wouldn't work in
//! this project's headless tests without also standing up a full picking
//! backend (`bv_editor_test_utils::headless_app()` has none). This hand-rolls
//! the same shape (`ScrollbarThumb` + drag/wheel/sync systems) on the same
//! primitives [`crate::splitter`] already uses instead.

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bevy_input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseButton, MouseScrollUnit};
use bevy_input::ButtonInput;
use bevy_math::Vec2;
use bevy_ui::{ComputedNode, Display, Interaction, Node, ScrollPosition, Val};

/// Thumb never shrinks below this, in logical pixels, even when the content
/// is much taller than the visible area — otherwise it'd disappear entirely
/// for a very long Scene Tree/Components list.
const MIN_THUMB_PX: f32 = 20.0;

/// Marks a UI node as the draggable thumb of a vertical scrollbar. `target`
/// is the scrollable container this thumb scrolls — its `Node` should set
/// `overflow: Overflow::scroll_y()` (see the module doc). The thumb's own
/// parent ([`bevy_ecs::hierarchy::ChildOf`]) is read each frame as the
/// scrollbar's track: whatever entity the thumb is spawned as a child of
/// defines the track's visual bounds/length, no separate `track` field needed.
#[derive(Component, Clone, Copy, Debug)]
pub struct ScrollbarThumb {
    pub target: Entity,
}

/// Convert a `ComputedNode`'s (physical-pixel) size into logical pixels —
/// the same unit `Val::Px` and `ScrollPosition` use, so thumb geometry and
/// `Node` writes can mix freely without a caller needing to think about
/// `UiScale` at all.
fn logical_size(node: &ComputedNode) -> Vec2 {
    node.size() * node.inverse_scale_factor()
}

fn logical_content_size(node: &ComputedNode) -> Vec2 {
    node.content_size() * node.inverse_scale_factor()
}

/// Pure thumb-geometry math: given the track's length, how much of the
/// content is visible at once, how tall the content actually is, and the
/// current scroll offset (all in the same unit, logical pixels in this
/// crate), return the thumb's `(length, offset)` along the track. No ECS —
/// same shape as [`crate::splitter::resize_value`], so this is
/// unit-testable without a running `App`.
///
/// Content that already fits (`content_length <= visible_length`) reports a
/// thumb that fills the whole track; callers hide it entirely in that case
/// (see [`sync_scrollbar_thumb_system`]) rather than showing a useless
/// full-length one.
pub fn thumb_geometry(track_length: f32, visible_length: f32, content_length: f32, scroll: f32, min_thumb: f32) -> (f32, f32) {
    if content_length <= visible_length || content_length <= 0.0 || track_length <= 0.0 {
        return (track_length, 0.0);
    }
    let thumb_length = (track_length * visible_length / content_length).clamp(min_thumb.min(track_length), track_length);
    let max_scroll = content_length - visible_length;
    let max_thumb_offset = (track_length - thumb_length).max(0.0);
    let offset = (scroll / max_scroll).clamp(0.0, 1.0) * max_thumb_offset;
    (thumb_length, offset)
}

/// Pure scroll-position math: clamp `current + delta` into the valid range
/// `[0, max(content_length - visible_length, 0)]`. Shared by wheel and
/// thumb-drag input.
pub fn clamp_scroll(current: f32, delta: f32, visible_length: f32, content_length: f32) -> f32 {
    let max_scroll = (content_length - visible_length).max(0.0);
    (current + delta).clamp(0.0, max_scroll)
}

/// Drives whichever [`ScrollbarThumb`] is currently held: press-and-drag it
/// to scroll its `target`. At most one thumb drags at a time — same
/// `Local<Option<Entity>>` pattern as [`crate::splitter::splitter_drag_system`].
pub fn scrollbar_drag_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    thumbs: Query<(Entity, &Interaction, &ScrollbarThumb, &ChildOf)>,
    tracks: Query<&ComputedNode>,
    mut targets: Query<(&ComputedNode, &mut ScrollPosition)>,
    mut dragging: Local<Option<Entity>>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        *dragging = None;
    } else if dragging.is_none() {
        *dragging = thumbs.iter().find(|(_, interaction, ..)| **interaction == Interaction::Pressed).map(|(entity, ..)| entity);
    }

    let Some(active) = *dragging else { return };
    let Ok((_, _, thumb, child_of)) = thumbs.get(active) else {
        *dragging = None;
        return;
    };
    if mouse_motion.delta.y == 0.0 {
        return;
    }
    let Ok(track_node) = tracks.get(child_of.parent()) else { return };
    let Ok((target_node, mut scroll)) = targets.get_mut(thumb.target) else { return };

    let track_length = logical_size(track_node).y;
    let visible = logical_size(target_node).y;
    let content = logical_content_size(target_node).y;
    if track_length <= 0.0 || content <= visible {
        return;
    }
    // Moving the cursor by `delta` (screen space) along the track should
    // move the *content* by the same proportion of the track that the
    // cursor covered — `content / track_length` converts one into the other
    // (the same ratio `bevy_ui_widgets`' own scrollbar drag uses).
    let scroll_delta = mouse_motion.delta.y * (content / track_length);
    scroll.0.y = clamp_scroll(scroll.0.y, scroll_delta, visible, content);
}

/// Scrolls whichever scrollable container the cursor is currently over in
/// response to the mouse wheel. `containers` is every entity with
/// `Interaction` + `ComputedNode` + `ScrollPosition` (i.e. an
/// `Overflow::scroll_y()` container with `Interaction::default()` added) —
/// only ones currently `Hovered` or `Pressed` react.
pub fn wheel_scroll_system(scroll_input: Res<AccumulatedMouseScroll>, mut containers: Query<(&Interaction, &ComputedNode, &mut ScrollPosition)>) {
    if scroll_input.delta.y == 0.0 {
        return;
    }
    let delta_px = match scroll_input.unit {
        MouseScrollUnit::Line => scroll_input.delta.y * MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
        MouseScrollUnit::Pixel => scroll_input.delta.y,
    };

    for (interaction, node, mut scroll) in &mut containers {
        if !matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            continue;
        }
        let visible = logical_size(node).y;
        let content = logical_content_size(node).y;
        // Positive wheel delta conventionally scrolls content up (view moves
        // down), i.e. *decreases* the remaining-below offset — negate it.
        scroll.0.y = clamp_scroll(scroll.0.y, -delta_px, visible, content);
    }
}

/// Keeps every [`ScrollbarThumb`]'s size/position in sync with its target's
/// scroll state every frame: hides the thumb (`Display::None`) entirely
/// when the content already fits, otherwise sizes/positions it via
/// [`thumb_geometry`].
pub fn sync_scrollbar_thumb_system(
    thumbs: Query<(Entity, &ScrollbarThumb, &ChildOf)>,
    tracks: Query<&ComputedNode>,
    targets: Query<(&ComputedNode, &ScrollPosition)>,
    mut thumb_nodes: Query<&mut Node>,
) {
    for (thumb_entity, thumb, child_of) in &thumbs {
        let Ok(track_node) = tracks.get(child_of.parent()) else { continue };
        let Ok((target_node, scroll)) = targets.get(thumb.target) else { continue };
        let Ok(mut node) = thumb_nodes.get_mut(thumb_entity) else { continue };

        let track_length = logical_size(track_node).y;
        let visible = logical_size(target_node).y;
        let content = logical_content_size(target_node).y;

        if content <= visible || track_length <= 0.0 {
            node.display = Display::None;
            continue;
        }
        node.display = Display::Flex;
        let (thumb_length, offset) = thumb_geometry(track_length, visible, content, scroll.0.y, MIN_THUMB_PX);
        node.height = Val::Px(thumb_length);
        node.top = Val::Px(offset);
    }
}

/// Registers the three systems above. A consumer that spawns
/// [`ScrollbarThumb`]s outside the full shell (e.g. a panel crate's own
/// tests) should add this itself, guarded by
/// `app.is_plugin_added::<ScrollbarPlugin>()`, the same convention
/// [`crate::dnd::DragAndDropPlugin`] uses.
#[derive(Default)]
pub struct ScrollbarPlugin;

impl Plugin for ScrollbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (scrollbar_drag_system, wheel_scroll_system, sync_scrollbar_thumb_system));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_fills_the_track_when_content_already_fits() {
        assert_eq!(thumb_geometry(200.0, 150.0, 100.0, 0.0, 20.0), (200.0, 0.0));
    }

    #[test]
    fn thumb_length_is_proportional_to_visible_over_content() {
        // track 200px, 100px visible of 400px content -> thumb is 1/4 of the track.
        let (length, _offset) = thumb_geometry(200.0, 100.0, 400.0, 0.0, 20.0);
        assert_eq!(length, 50.0);
    }

    #[test]
    fn thumb_never_shrinks_below_the_minimum() {
        let (length, _offset) = thumb_geometry(200.0, 10.0, 4000.0, 0.0, 20.0);
        assert_eq!(length, 20.0);
    }

    #[test]
    fn thumb_offset_tracks_scroll_position_proportionally() {
        // content 400, visible 100 -> max_scroll=300, thumb_length=50 -> max_thumb_offset=150.
        // Scrolled halfway (150/300) should put the thumb halfway along its own range (75).
        let (_length, offset) = thumb_geometry(200.0, 100.0, 400.0, 150.0, 20.0);
        assert_eq!(offset, 75.0);
    }

    #[test]
    fn clamp_scroll_moves_by_delta_within_bounds() {
        assert_eq!(clamp_scroll(50.0, 30.0, 100.0, 300.0), 80.0);
        assert_eq!(clamp_scroll(10.0, -50.0, 100.0, 300.0), 0.0);
    }

    #[test]
    fn clamp_scroll_clamps_to_the_maximum() {
        assert_eq!(clamp_scroll(190.0, 50.0, 100.0, 300.0), 200.0);
    }

    #[test]
    fn clamp_scroll_range_is_zero_when_content_already_fits() {
        assert_eq!(clamp_scroll(0.0, 50.0, 200.0, 100.0), 0.0);
    }

    fn computed_node(size_y: f32, content_size_y: f32) -> ComputedNode {
        ComputedNode { size: Vec2::new(100.0, size_y), content_size: Vec2::new(100.0, content_size_y), inverse_scale_factor: 1.0, ..Default::default() }
    }

    fn setup_scrollbar(app: &mut App, visible: f32, content: f32, track_length: f32) -> (Entity, Entity, Entity) {
        let target = app.world_mut().spawn((Node::default(), computed_node(visible, content))).id();
        let track = app.world_mut().spawn((Node::default(), computed_node(track_length, track_length))).id();
        let thumb = app
            .world_mut()
            .spawn((ScrollbarThumb { target }, Interaction::default(), Node::default(), bevy_ecs::hierarchy::ChildOf(track)))
            .id();
        (target, track, thumb)
    }

    #[test]
    fn sync_hides_the_thumb_when_content_fits() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(ScrollbarPlugin);
        let (_target, _track, thumb) = setup_scrollbar(&mut app, 200.0, 100.0, 200.0);
        bv_editor_test_utils::step(&mut app, 1);

        assert_eq!(app.world().get::<Node>(thumb).unwrap().display, Display::None);
    }

    #[test]
    fn sync_sizes_the_thumb_when_content_overflows() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(ScrollbarPlugin);
        let (_target, _track, thumb) = setup_scrollbar(&mut app, 100.0, 400.0, 200.0);
        bv_editor_test_utils::step(&mut app, 1);

        let node = app.world().get::<Node>(thumb).unwrap();
        assert_eq!(node.display, Display::Flex);
        assert_eq!(node.height, Val::Px(50.0)); // track 200 * visible 100 / content 400
    }

    #[test]
    fn wheel_scrolls_the_hovered_container_only() {
        use bevy_input::mouse::MouseWheel;
        use bevy_input::touch::TouchPhase;

        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(ScrollbarPlugin);
        let hovered = app.world_mut().spawn((Node::default(), Interaction::Hovered, computed_node(100.0, 400.0))).id();
        let not_hovered = app.world_mut().spawn((Node::default(), Interaction::None, computed_node(100.0, 400.0))).id();
        bv_editor_test_utils::step(&mut app, 1);

        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: -30.0,
            window: Entity::PLACEHOLDER,
            phase: TouchPhase::Moved,
        });
        bv_editor_test_utils::step(&mut app, 1);

        assert_eq!(app.world().get::<ScrollPosition>(hovered).unwrap().0.y, 30.0);
        assert_eq!(app.world().get::<ScrollPosition>(not_hovered).unwrap().0.y, 0.0);
    }

    #[test]
    fn dragging_the_thumb_scrolls_the_target() {
        use bevy_input::mouse::MouseMotion;

        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(ScrollbarPlugin);
        let (target, _track, thumb) = setup_scrollbar(&mut app, 100.0, 400.0, 200.0);
        bv_editor_test_utils::step(&mut app, 1);

        app.world_mut().entity_mut(thumb).insert(Interaction::Pressed);
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        // Same reasoning as `splitter`'s own drag test: send a `MouseMotion`
        // event rather than writing `AccumulatedMouseMotion` directly, since
        // `bevy_input` rebuilds that resource from queued events every frame.
        app.world_mut().write_message(MouseMotion { delta: Vec2::new(0.0, 25.0) });
        bv_editor_test_utils::step(&mut app, 1);

        // Track 200px, content 400px -> dragging the thumb 25px moves the
        // content by 25 * (400/200) = 50px.
        assert_eq!(app.world().get::<ScrollPosition>(target).unwrap().0.y, 50.0);
    }
}
