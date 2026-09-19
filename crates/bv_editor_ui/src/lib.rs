//! `bv_editor_ui` — docking/shell framework on `bevy_ui`.
//!
//! Phase 1 (docs/DESIGN.md section 9): a fixed layout skeleton — top toolbar,
//! left panel, center viewport, right panel, bottom panel row, bottom status
//! bar — with resizable splitters between zones, and empty titled boxes
//! where each built-in panel will mount its real content starting Phase 2.
//! No tabs and no drag-to-dock yet; that's Phase 10.
//!
//! - [`shell`] — the node tree itself ([`spawn_editor_shell`]) and the
//!   `*Slot` marker components later phases query for.
//! - [`splitter`] — the drag-to-resize mechanism between zones.
//! - [`breakpoint`] — pure sizing math, independent of `bevy_ui`, used to
//!   pick the shell's *initial* panel sizes from the window's width.
//! - [`dnd`] — the generic drag-and-drop framework (section 8.5), first used
//!   by the Scene Tree (Phase 2) to reparent by dragging a row.

mod breakpoint;
mod dnd;
mod scrollbar;
mod shell;
mod splitter;

pub use breakpoint::{bottom_panel_height_px, breakpoint_for_width, side_panel_width_px, LayoutBreakpoint};
pub use dnd::{drag_and_drop_system, DragAndDropPlugin, DragDropped, DragPayload, DragSource, DragState, DropTarget};
pub use scrollbar::{clamp_scroll, scrollbar_drag_system, sync_scrollbar_thumb_system, thumb_geometry, wheel_scroll_system, ScrollbarAxis, ScrollbarPlugin, ScrollbarThumb};
pub use shell::{
    spawn_editor_shell, AssetsPanelSlot, ConsolePanelSlot, EditorShellEntities, EditorShellRoot,
    InspectorPanelSlot, ScenePanelSlot, StatusBarSlot, ToolbarSlot, ViewportSlot,
};
pub use splitter::{resize_value, splitter_cursor_system, splitter_drag_system, Splitter, SplitterAxis};

use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::prelude::*;
use bevy_window::{PrimaryWindow, Window};

/// The system set [`spawn_shell_on_startup`] runs in. Anything that adds
/// content under one of the shell's `*Slot`s during `Startup` (the Scene
/// Tree, from Phase 2 on) must order itself `.after(EditorShellSet)` —
/// `Startup` systems from different plugins have no guaranteed relative
/// order otherwise, so without this a panel's own startup spawn can run
/// before the slot it looks for exists.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorShellSet;

/// Spawns the shell on startup, drives its splitters every frame, and runs
/// the generic drag-and-drop framework consumers (the Scene Tree, from Phase
/// 2 on) build on top of.
///
/// `ui_scale` sets `bevy_ui`'s own [`UiScale`] resource, which uniformly
/// scales every panel's layout *and* text — the whole shell is built from
/// `bevy_ui`'s default sizes (20px text, etc.), which reads oversized at
/// typical desktop window sizes, so this defaults to `0.5` rather than
/// `bevy_ui`'s own default of `1.0`. Override it per host game with
/// `EditorUiPlugin { ui_scale: 0.7 }` (or via [`bv_editor::EditorPlugin`]'s
/// own `ui_scale` field, which forwards here) — [`UiScale`] itself is also
/// just an ordinary resource, so `ResMut<UiScale>` works too if something
/// needs to change it at runtime (e.g. a future user-facing zoom setting).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorUiPlugin {
    pub ui_scale: f32,
}

impl Default for EditorUiPlugin {
    fn default() -> Self {
        Self { ui_scale: 0.5 }
    }
}

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<DragAndDropPlugin>() {
            app.add_plugins(DragAndDropPlugin);
        }
        if !app.is_plugin_added::<ScrollbarPlugin>() {
            app.add_plugins(ScrollbarPlugin);
        }
        app.insert_resource(bevy_ui::UiScale(self.ui_scale));
        app.init_resource::<splitter::ActiveSplitterDrag>();
        app.add_systems(Startup, spawn_shell_on_startup.in_set(EditorShellSet));
        app.add_systems(Update, (splitter_drag_system, splitter_cursor_system).chain());
    }
}

/// Picks the initial breakpoint from the primary window's width, if there is
/// one (there isn't in a headless test), and spawns the shell.
fn spawn_shell_on_startup(mut commands: Commands, windows: Query<&Window, With<PrimaryWindow>>) {
    let breakpoint = windows
        .single()
        .map(|window| breakpoint_for_width(window.width()))
        .unwrap_or(LayoutBreakpoint::Normal);
    spawn_editor_shell(&mut commands, breakpoint);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui::Node;

    #[test]
    fn plugin_builds_without_panicking_headless() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);
    }

    #[test]
    fn headless_shell_defaults_to_normal_breakpoint() {
        // No `Window` resource exists under `MinimalPlugins`, so startup
        // should fall back to `LayoutBreakpoint::Normal` rather than panic.
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let mut query = world.query_filtered::<&Node, With<ScenePanelSlot>>();
        let node = query.single(world).expect("scene panel slot should exist");
        assert_eq!(node.width, bevy_ui::Val::Px(side_panel_width_px(LayoutBreakpoint::Normal)));
    }

    #[test]
    fn spawn_editor_shell_sizes_panels_per_breakpoint() {
        use bevy_ecs::system::RunSystemOnce;

        let mut app = bv_editor_test_utils::headless_app();
        let entities = app
            .world_mut()
            .run_system_once(|mut commands: Commands| spawn_editor_shell(&mut commands, LayoutBreakpoint::Compact))
            .expect("spawning the shell should not fail");

        let world = app.world();
        let scene_width = world.get::<Node>(entities.scene_panel).unwrap().width;
        let inspector_width = world.get::<Node>(entities.inspector_panel).unwrap().width;
        let expected = bevy_ui::Val::Px(side_panel_width_px(LayoutBreakpoint::Compact));
        assert_eq!(scene_width, expected);
        assert_eq!(inspector_width, expected);
    }

    #[test]
    fn panels_carry_real_min_max_size_bounds() {
        // docs/UI_FEATURES.md F7: these must be real `Node.min_*`/`max_*`
        // constraints (enforced by `bevy_ui`'s own layout engine every
        // frame), not just numbers a splitter happens to clamp against
        // while it's being dragged.
        use bevy_ecs::system::RunSystemOnce;
        use bevy_ui::Val;

        let mut app = bv_editor_test_utils::headless_app();
        let entities = app
            .world_mut()
            .run_system_once(|mut commands: Commands| spawn_editor_shell(&mut commands, LayoutBreakpoint::Normal))
            .expect("spawning the shell should not fail");

        let world = app.world();
        let scene = world.get::<Node>(entities.scene_panel).unwrap();
        let inspector = world.get::<Node>(entities.inspector_panel).unwrap();
        for panel in [scene, inspector] {
            assert!(matches!(panel.min_width, Val::Px(px) if px > 0.0));
            assert!(matches!(panel.max_width, Val::Px(px) if px > 0.0));
        }

        let viewport = world.get::<Node>(entities.viewport).unwrap();
        assert!(matches!(viewport.min_width, Val::Px(px) if px > 0.0), "viewport must have its own width floor (F7 gap #2)");
        assert!(matches!(viewport.min_height, Val::Px(px) if px > 0.0), "viewport must have its own height floor (F7 gap #2)");

        let assets = world.get::<Node>(entities.assets_panel).unwrap();
        let console = world.get::<Node>(entities.console_panel).unwrap();
        for panel in [assets, console] {
            assert!(matches!(panel.min_width, Val::Px(px) if px > 0.0), "bottom-row panels must have a width floor (F7 gap #3)");
        }

        // The bottom row itself (not exposed as a named `EditorShellEntities`
        // field) is whatever the vertical `Splitter`'s target is.
        let world = app.world_mut();
        let mut splitters = world.query::<&Splitter>();
        let bottom_row = splitters.iter(world).find(|s| s.axis == SplitterAxis::Vertical).map(|s| s.target).expect("the bottom row's vertical splitter should exist");
        let bottom_row_bounds = world.get::<Node>(bottom_row).expect("the bottom row's target should be a real Node");
        assert!(matches!(bottom_row_bounds.min_height, Val::Px(px) if px > 0.0));
        assert!(matches!(bottom_row_bounds.max_height, Val::Px(px) if px > 0.0));
    }

    #[test]
    fn splitter_drag_resizes_its_target() {
        use bevy_input::mouse::{MouseButton, MouseMotion};
        use bevy_input::ButtonInput;
        use bevy_math::Vec2;
        use bevy_ui::{Interaction, Val};

        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let mut scene_panels = world.query_filtered::<Entity, With<ScenePanelSlot>>();
        let target = scene_panels.single(world).expect("scene panel slot should exist");

        let mut splitters = world.query::<(Entity, &Splitter)>();
        let splitter_entity = splitters
            .iter(world)
            .find(|(_, s)| s.target == target)
            .map(|(e, _)| e)
            .expect("a splitter targeting the scene panel should exist");

        world.entity_mut(splitter_entity).insert(Interaction::Pressed);
        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        // Send a `MouseMotion` event rather than writing `AccumulatedMouseMotion`
        // directly: bevy_input's own `PreUpdate` system rebuilds that resource
        // from queued `MouseMotion` events every frame (zeroing it first), so a
        // direct write is wiped before `splitter_drag_system` (in `Update`) runs.
        world.write_message(MouseMotion { delta: Vec2::new(40.0, 0.0) });

        let before = world.get::<Node>(target).unwrap().width;
        bv_editor_test_utils::step(&mut app, 1);
        let after = app.world().get::<Node>(target).unwrap().width;

        assert_eq!(before, Val::Px(side_panel_width_px(LayoutBreakpoint::Normal)));
        assert_eq!(after, Val::Px(side_panel_width_px(LayoutBreakpoint::Normal) + 40.0));
    }

    #[test]
    fn inverted_splitter_drag_resizes_its_target_opposite_the_mouse() {
        // The Components panel's splitter is its *left* edge (target is to
        // the right of the splitter, unlike the Scene Tree above where the
        // target is to the left) — dragging the mouse right must shrink it,
        // not grow it, or the panel visibly moves the opposite way from the
        // mouse. Same idea for the bottom row's splitter (its *top* edge):
        // dragging down must shrink it.
        use bevy_input::mouse::{MouseButton, MouseMotion};
        use bevy_input::ButtonInput;
        use bevy_math::Vec2;
        use bevy_ui::{Interaction, Val};

        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let mut inspector_panels = world.query_filtered::<Entity, With<InspectorPanelSlot>>();
        let target = inspector_panels.single(world).expect("inspector panel slot should exist");

        let mut splitters = world.query::<(Entity, &Splitter)>();
        let splitter_entity = splitters
            .iter(world)
            .find(|(_, s)| s.target == target)
            .map(|(e, _)| e)
            .expect("a splitter targeting the inspector panel should exist");

        world.entity_mut(splitter_entity).insert(Interaction::Pressed);
        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        world.write_message(MouseMotion { delta: Vec2::new(40.0, 0.0) });

        let before = world.get::<Node>(target).unwrap().width;
        bv_editor_test_utils::step(&mut app, 1);
        let after = app.world().get::<Node>(target).unwrap().width;

        assert_eq!(before, Val::Px(side_panel_width_px(LayoutBreakpoint::Normal)));
        assert_eq!(
            after,
            Val::Px(side_panel_width_px(LayoutBreakpoint::Normal) - 40.0),
            "dragging right on the Components panel's left-edge splitter should shrink it, not grow it"
        );
    }

    #[test]
    fn hovering_a_splitter_sets_the_resize_cursor_and_clears_it_after() {
        // docs/UI_FEATURES.md F4: hovering (not just dragging) a splitter
        // should show a resize cursor, and it should go back to nothing once
        // the pointer leaves.
        use bevy_ui::Interaction;
        use bevy_window::{CursorIcon, PrimaryWindow, SystemCursorIcon, Window};

        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let window = app.world_mut().spawn((Window::default(), PrimaryWindow)).id();

        let world = app.world_mut();
        let mut scene_panels = world.query_filtered::<Entity, With<ScenePanelSlot>>();
        let target = scene_panels.single(world).expect("scene panel slot should exist");
        let mut splitters = world.query::<(Entity, &Splitter)>();
        let splitter_entity = splitters
            .iter(world)
            .find(|(_, s)| s.target == target)
            .map(|(e, _)| e)
            .expect("a splitter targeting the scene panel should exist");

        world.entity_mut(splitter_entity).insert(Interaction::Hovered);
        bv_editor_test_utils::step(&mut app, 1);

        let icon = app
            .world()
            .get::<CursorIcon>(window)
            .expect("hovering a splitter should set a cursor icon");
        assert_eq!(*icon, CursorIcon::System(SystemCursorIcon::EwResize));

        app.world_mut().entity_mut(splitter_entity).insert(Interaction::None);
        bv_editor_test_utils::step(&mut app, 1);

        assert!(
            app.world().get::<CursorIcon>(window).is_none(),
            "cursor icon should clear once nothing is hovered/dragging"
        );
    }
}
