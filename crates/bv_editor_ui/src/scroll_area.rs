//! [`spawn_scroll_area`] — the composed "scrollable content + its track(s)/
//! thumb(s)" widget, built on the primitives in [`crate::scrollbar`].
//!
//! Extracted after `bv_editor_scene_panel` and `bv_editor_inspector_panel`
//! each hand-rolled their own copy of this shape (docs/UI_FEATURES.md
//! F2/F3) and had quietly drifted apart: a `flex_shrink` bug (a fixed-width
//! track being crushed by its flex container's default shrink behavior) had
//! to be found and fixed twice, once per copy, because there was no single
//! place to fix it.

use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_ui::prelude::*;

use crate::scrollbar::{ScrollbarAxis, ScrollbarThumb};

/// Which axes a [`spawn_scroll_area`] call scrolls along.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollAxes {
    /// Vertical track only (e.g. the Scene Tree: a plain list of rows).
    Vertical,
    /// Horizontal track only.
    Horizontal,
    /// Both — an "L-shaped" scroll pane (vertical track beside the content,
    /// horizontal track below it), e.g. the Components panel: field rows
    /// that can be both taller *and* wider than the panel.
    Both,
}

/// Visual/layout knobs for [`spawn_scroll_area`]. `Default` matches what
/// `bv_editor_scene_panel`/`bv_editor_inspector_panel` already used before
/// this was extracted, so existing call sites only need to set `margin`.
#[derive(Clone, Copy, Debug)]
pub struct ScrollAreaStyle {
    pub track_thickness_px: f32,
    pub track_background: Color,
    pub thumb_background: Color,
    /// Margin on the scroll area's own outer node (e.g. spacing below a
    /// panel's toolbar). Not really "what a scroll area is", but every
    /// current caller needs one, so it's a parameter here rather than
    /// forcing callers to wrap the returned tree in yet another
    /// margin-only node just to get the same spacing back.
    pub margin: UiRect,
}

impl Default for ScrollAreaStyle {
    fn default() -> Self {
        Self { track_thickness_px: 12.0, track_background: Color::srgb(0.12, 0.12, 0.13), thumb_background: Color::srgb(0.35, 0.35, 0.4), margin: UiRect::DEFAULT }
    }
}

fn spawn_track(commands: &mut Commands, parent: Entity, axis: ScrollbarAxis, body: Entity, style: &ScrollAreaStyle) {
    // `flex_shrink: 0.0` on both: a `Node`'s default of `1.0` would
    // otherwise let the flex container crush this fixed-thickness track to
    // make room for `body`'s content (which can legitimately want to be
    // much larger than the visible area along either axis) — the exact bug
    // this module was extracted to stop from happening twice.
    let track_node = match axis {
        ScrollbarAxis::Vertical => {
            Node { width: Val::Px(style.track_thickness_px), height: Val::Percent(100.0), flex_shrink: 0.0, margin: UiRect::left(Val::Px(2.0)), ..Default::default() }
        }
        ScrollbarAxis::Horizontal => {
            Node { width: Val::Percent(100.0), height: Val::Px(style.track_thickness_px), flex_shrink: 0.0, margin: UiRect::top(Val::Px(2.0)), ..Default::default() }
        }
    };
    let track = commands.spawn((track_node, BackgroundColor(style.track_background), ChildOf(parent))).id();

    let thumb_node = match axis {
        ScrollbarAxis::Vertical => Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), top: Val::Px(0.0), ..Default::default() },
        ScrollbarAxis::Horizontal => Node { position_type: PositionType::Absolute, height: Val::Percent(100.0), left: Val::Px(0.0), ..Default::default() },
    };
    commands.spawn((ScrollbarThumb { target: body, axis }, Interaction::default(), thumb_node, BackgroundColor(style.thumb_background), ChildOf(track)));
}

/// Spawns a complete scrollable content area: one or two `Overflow::scroll`
/// axes on a fresh "body" node, each with a matching track + [`ScrollbarThumb`]
/// wired up ready for [`crate::ScrollbarPlugin`]'s systems to drive.
///
/// Returns the *body* entity — a plain, empty `Node` (column layout,
/// `Interaction` already attached, `overflow`/`min_width`/`min_height`/
/// `flex_grow` already set correctly for `axes`) that the caller spawns
/// their own scrollable content into as children, and tags with their own
/// marker component via `commands.entity(body).insert(YourMarker)` — the
/// same "reserve the id, insert real components later" idiom
/// [`crate::shell`] already uses for forward-referenced entities.
pub fn spawn_scroll_area(commands: &mut Commands, parent: Entity, axes: ScrollAxes, style: ScrollAreaStyle) -> Entity {
    let wants_vertical = matches!(axes, ScrollAxes::Vertical | ScrollAxes::Both);
    let wants_horizontal = matches!(axes, ScrollAxes::Horizontal | ScrollAxes::Both);
    let overflow = match axes {
        ScrollAxes::Vertical => Overflow::scroll_y(),
        ScrollAxes::Horizontal => Overflow::scroll_x(),
        ScrollAxes::Both => Overflow::scroll(),
    };

    let root = commands
        .spawn((Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_height: Val::Px(0.0), margin: style.margin, ..Default::default() }, ChildOf(parent)))
        .id();

    let content_row = commands
        .spawn((Node { flex_direction: FlexDirection::Row, flex_grow: 1.0, min_height: Val::Px(0.0), ..Default::default() }, ChildOf(root)))
        .id();

    let body = commands
        .spawn((
            Interaction::default(),
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: if wants_horizontal { Val::Px(0.0) } else { Val::Auto },
                min_height: if wants_vertical { Val::Px(0.0) } else { Val::Auto },
                overflow,
                ..Default::default()
            },
            ChildOf(content_row),
        ))
        .id();

    if wants_vertical {
        spawn_track(commands, content_row, ScrollbarAxis::Vertical, body, &style);
    }
    if wants_horizontal {
        spawn_track(commands, root, ScrollbarAxis::Horizontal, body, &style);
    }

    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    fn thumbs_of(app: &mut bevy_app::App, body: Entity) -> Vec<ScrollbarAxis> {
        let world = app.world_mut();
        let mut thumbs = world.query::<&ScrollbarThumb>();
        thumbs.iter(world).filter(|t| t.target == body).map(|t| t.axis).collect()
    }

    #[test]
    fn vertical_only_gets_exactly_one_vertical_thumb() {
        let mut app = bv_editor_test_utils::headless_app();
        let parent = app.world_mut().spawn(Node::default()).id();
        let body = app
            .world_mut()
            .run_system_once(move |mut commands: Commands| spawn_scroll_area(&mut commands, parent, ScrollAxes::Vertical, ScrollAreaStyle::default()))
            .expect("spawning should not fail");

        assert_eq!(thumbs_of(&mut app, body), vec![ScrollbarAxis::Vertical]);
        let node = app.world().get::<Node>(body).unwrap();
        assert_eq!(node.overflow, Overflow::scroll_y());
        assert_eq!(node.min_height, Val::Px(0.0), "must be able to shrink below its rows' natural height for Overflow::scroll_y to clip anything");
    }

    #[test]
    fn both_axes_gets_one_thumb_per_axis() {
        let mut app = bv_editor_test_utils::headless_app();
        let parent = app.world_mut().spawn(Node::default()).id();
        let body = app
            .world_mut()
            .run_system_once(move |mut commands: Commands| spawn_scroll_area(&mut commands, parent, ScrollAxes::Both, ScrollAreaStyle::default()))
            .expect("spawning should not fail");

        let mut axes = thumbs_of(&mut app, body);
        axes.sort_by_key(|a| format!("{a:?}"));
        assert_eq!(axes, vec![ScrollbarAxis::Horizontal, ScrollbarAxis::Vertical]);
        let node = app.world().get::<Node>(body).unwrap();
        assert_eq!(node.overflow, Overflow::scroll());
        assert_eq!(node.min_width, Val::Px(0.0));
        assert_eq!(node.min_height, Val::Px(0.0));
    }

    #[test]
    fn every_track_opts_out_of_flex_shrink() {
        // Regression test for the bug this module was extracted to stop
        // from recurring: a track's fixed thickness must not be crushable
        // by its flex container's default shrink behavior.
        let mut app = bv_editor_test_utils::headless_app();
        let parent = app.world_mut().spawn(Node::default()).id();
        let body = app
            .world_mut()
            .run_system_once(move |mut commands: Commands| spawn_scroll_area(&mut commands, parent, ScrollAxes::Both, ScrollAreaStyle::default()))
            .expect("spawning should not fail");

        let world = app.world_mut();
        let mut thumbs = world.query::<(&ScrollbarThumb, &ChildOf)>();
        let track_entities: Vec<Entity> = thumbs.iter(world).filter(|(t, _)| t.target == body).map(|(_, child_of)| child_of.parent()).collect();
        assert_eq!(track_entities.len(), 2);
        for track in track_entities {
            let node = world.get::<Node>(track).unwrap();
            assert_eq!(node.flex_shrink, 0.0, "track {track:?} must not be shrinkable");
        }
    }
}
