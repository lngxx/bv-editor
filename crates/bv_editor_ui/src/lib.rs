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
mod shell;
mod splitter;

pub use breakpoint::{bottom_panel_height_px, breakpoint_for_width, side_panel_width_px, LayoutBreakpoint};
pub use dnd::{drag_and_drop_system, DragAndDropPlugin, DragDropped, DragPayload, DragSource, DragState, DropTarget};
pub use shell::{
    spawn_editor_shell, AssetsPanelSlot, ConsolePanelSlot, EditorShellEntities, EditorShellRoot,
    InspectorPanelSlot, ScenePanelSlot, StatusBarSlot, ToolbarSlot, ViewportSlot,
};
pub use splitter::{resize_value, splitter_drag_system, Splitter, SplitterAxis};

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
        app.insert_resource(bevy_ui::UiScale(self.ui_scale));
        app.add_systems(Startup, spawn_shell_on_startup.in_set(EditorShellSet));
        app.add_systems(Update, splitter_drag_system);
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
}
