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
mod dock;
mod icon;
mod scroll_area;
mod scrollbar;
mod shell;
mod splitter;

pub use breakpoint::{bottom_panel_height_px, breakpoint_for_width, side_panel_width_px, LayoutBreakpoint};
pub use dnd::{drag_and_drop_system, DragAndDropPlugin, DragDropped, DragPayload, DragSource, DragState, DropTarget};
pub use dock::{
    dock_drag_drop_system, dock_tab_click_system, DockDropTarget, DockEdge, DockLayout, DockLayoutDirty, DockNode, DockRegion, DockSlots, DockTabButton, DockZone, PanelId, DEFAULT_SPLIT_SIZE_PX,
};
pub use icon::{spawn_icon, IconId, IconPlugin, IconRegistry, PLACEHOLDER_ICON};
pub use scroll_area::{spawn_scroll_area, ScrollAreaStyle, ScrollAxes};
pub use scrollbar::{clamp_scroll, scrollbar_cursor_system, scrollbar_drag_system, sync_scrollbar_thumb_system, thumb_geometry, wheel_scroll_system, ScrollbarAxis, ScrollbarPlugin, ScrollbarThumb};
pub use shell::{
    dock_drag_cursor_system, dock_drop_target_highlight_system, spawn_editor_shell, AssetsPanelSlot, ConsolePanelSlot, DockRegionRoot, EditorShellEntities, EditorShellRoot, InspectorPanelSlot,
    ScenePanelSlot, StatusBarSlot, ToolbarSlot, ViewportSlot,
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
        if !app.is_plugin_added::<IconPlugin>() {
            app.add_plugins(IconPlugin);
        }
        app.insert_resource(bevy_ui::UiScale(self.ui_scale));
        app.init_resource::<splitter::ActiveSplitterDrag>();
        app.add_systems(Startup, spawn_shell_on_startup.in_set(EditorShellSet));
        app.add_systems(Update, (splitter_drag_system, splitter_cursor_system).chain());
        shell::add_dock_systems(app);
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
    use bevy_asset::{AssetApp, AssetPlugin};
    use bevy_image::Image;
    use bevy_ui::Node;

    /// `EditorUiPlugin` now includes `IconPlugin` (F6), whose `Startup`
    /// system needs `Assets<Image>` for the built-in placeholder icon's
    /// procedural sprite sheet — same reasoning as `bv_editor_viewport`'s
    /// own `setup()`: the first thing in this crate to need asset support
    /// adds `AssetPlugin` itself rather than pushing that onto
    /// `bv_editor_test_utils` for tests that don't need it.
    fn setup() -> bevy_app::App {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Image>();
        app
    }

    /// Finds `region`'s resizable dock region box (docs/UI_FEATURES.md F5)
    /// by its [`DockRegionRoot`] marker — the entity that used to be
    /// findable via `With<ScenePanelSlot>`/`With<InspectorPanelSlot>`
    /// before docking split "the resizable box" from "the content slot"
    /// into two separate entities. Tests that go through the real
    /// `EditorUiPlugin::Startup` system (rather than calling
    /// `spawn_editor_shell` directly and keeping its returned
    /// `EditorShellEntities`) need this to get back to the box at all.
    fn find_region_box(world: &mut bevy_ecs::world::World, region: dock::DockRegion) -> Entity {
        let mut query = world.query::<(Entity, &DockRegionRoot)>();
        query.iter(world).find(|(_, r)| r.0 == region).map(|(e, _)| e).expect("the region's box should exist")
    }

    #[test]
    fn plugin_builds_without_panicking_headless() {
        let mut app = setup();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);
    }

    #[test]
    fn headless_shell_defaults_to_normal_breakpoint() {
        // No `Window` resource exists under `MinimalPlugins`, so startup
        // should fall back to `LayoutBreakpoint::Normal` rather than panic.
        let mut app = setup();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let left_region = find_region_box(world, dock::DockRegion::Left);
        let node = world.get::<Node>(left_region).unwrap();
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

        // Project Files/Console no longer have stable entity ids of their
        // own — docking (F5) means either could move elsewhere — so unlike
        // `scene`/`inspector` above, find their default split via its
        // `Splitter` rather than an `EditorShellEntities` field: the one
        // whose target isn't the already-known left/right region box.
        let world = app.world_mut();
        let mut splitters = world.query::<&Splitter>();
        let bottom_split_target = splitters
            .iter(world)
            .find(|s| s.axis == SplitterAxis::Horizontal && s.target != entities.scene_panel && s.target != entities.inspector_panel)
            .map(|s| s.target)
            .expect("the bottom region's default Assets/Console splitter should exist");
        let bottom_split_bounds = world.get::<Node>(bottom_split_target).expect("the bottom region's split target should be a real Node");
        assert!(matches!(bottom_split_bounds.min_width, Val::Px(px) if px > 0.0), "bottom-row panels must have a width floor (F7 gap #3)");
        assert!(matches!(bottom_split_bounds.max_width, Val::Px(px) if px > 0.0));

        // The bottom row itself (not exposed as a named `EditorShellEntities`
        // field) is whatever the vertical `Splitter`'s target is.
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

        let mut app = setup();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let target = find_region_box(world, dock::DockRegion::Left);

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

        let mut app = setup();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let target = find_region_box(world, dock::DockRegion::Right);

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

        let mut app = setup();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let window = app.world_mut().spawn((Window::default(), PrimaryWindow)).id();

        let world = app.world_mut();
        let target = find_region_box(world, dock::DockRegion::Left);
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

    #[test]
    fn hovering_and_dragging_a_dock_tab_sets_the_grab_and_grabbing_cursors() {
        // docs/UI_FEATURES.md F5: dragging a dock tab had no cursor
        // feedback at all before this — unlike splitters/scrollbar thumbs,
        // which already show one.
        //
        // Drives the press/release through `bv_editor_test_utils::simulate_click`/
        // `simulate_release` (real `MouseButtonInput` messages), not a direct
        // `ButtonInput::press()` call: `drag_and_drop_system`'s drag-start
        // detection reads `just_pressed`, and with the real `InputPlugin`
        // active (via `EditorUiPlugin`'s full app, unlike `crate::dnd`'s own
        // bare-`World` tests), a direct write to that flag is silently wiped
        // by `mouse_button_input_system` in `PreUpdate` before any `Update`
        // system — this one included — ever observes it (see that crate's
        // own module doc comment for the full explanation).
        use bevy_ui::Interaction;
        use bevy_window::{CursorIcon, PrimaryWindow, SystemCursorIcon, Window};

        let mut app = setup();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let window = app.world_mut().spawn((Window::default(), PrimaryWindow)).id();

        let world = app.world_mut();
        let mut tab_buttons = world.query::<(Entity, &dock::DockTabButton)>();
        let (scene_tab, _) = tab_buttons.iter(world).find(|(_, b)| b.0 == dock::PanelId::Scene).expect("the default Scene Tree tab button should exist");

        world.entity_mut(scene_tab).insert(Interaction::Hovered);
        bv_editor_test_utils::step(&mut app, 1);

        let icon = app.world().get::<CursorIcon>(window).expect("hovering a dock tab should set a cursor icon");
        assert_eq!(*icon, CursorIcon::System(SystemCursorIcon::Grab));

        // Press-and-hold without releasing: a real drag in progress.
        app.world_mut().entity_mut(scene_tab).insert(Interaction::Pressed);
        bv_editor_test_utils::simulate_click(&mut app, bevy_math::Vec2::ZERO);
        bv_editor_test_utils::step(&mut app, 1);

        let icon = app.world().get::<CursorIcon>(window).expect("dragging a dock tab should set a cursor icon");
        assert_eq!(*icon, CursorIcon::System(SystemCursorIcon::Grabbing));

        // Release over empty space (no drop target hovered) and stop
        // hovering the tab: the cursor should clear, not get stuck.
        app.world_mut().entity_mut(scene_tab).insert(Interaction::None);
        bv_editor_test_utils::simulate_release(&mut app);
        bv_editor_test_utils::step(&mut app, 1);

        assert!(app.world().get::<CursorIcon>(window).is_none(), "cursor icon should clear once the drag ends and nothing is hovered");
    }

    #[test]
    fn dragging_a_dock_tab_onto_another_panel_merges_them_through_the_real_editor_ui_plugin() {
        // Regression test for the actual reported bug: dragging a dock tab
        // never merged anything, because `dock_tab_click_system` used to
        // fire on `just_pressed` — the very frame a drag starts — setting
        // `DockLayoutDirty` and triggering a full chrome rebuild mid-drag
        // that despawned the tab button entity the drag depended on. Every
        // `dock.rs` test up to this point drove `dock_drag_drop_system` and
        // `dock_tab_click_system` in isolation (bare `World`s, one message
        // at a time), which is exactly why none of them caught it — this
        // one drives a real press-hold-move-release sequence through all
        // three systems together (`dock_tab_click_system`,
        // `dock_drag_drop_system`, `rebuild_dock_ui_system`) the way a real
        // drag actually happens, with a `step()` in between each stage so a
        // mid-drag rebuild (if the bug were still there) would have a
        // chance to invalidate the entities this test keeps using.
        use bevy_ui::Interaction;

        let mut app = setup();
        app.add_plugins(EditorUiPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let mut tab_buttons = world.query::<(Entity, &dock::DockTabButton)>();
        let console_tab = tab_buttons.iter(world).find(|(_, b)| b.0 == dock::PanelId::Console).map(|(e, _)| e).expect("the default Console tab button should exist");
        let scene_tab = tab_buttons.iter(world).find(|(_, b)| b.0 == dock::PanelId::Scene).map(|(e, _)| e).expect("the default Scene Tree tab button should exist");

        // Press on Console's tab (in the bottom region) and hold.
        world.entity_mut(console_tab).insert(Interaction::Pressed);
        bv_editor_test_utils::simulate_click(&mut app, bevy_math::Vec2::ZERO);
        bv_editor_test_utils::step(&mut app, 1); // the frame the old bug rebuilt mid-drag on

        assert_eq!(
            app.world().resource::<dock::DockLayout>().region_of(dock::PanelId::Console),
            Some(dock::DockRegion::Bottom),
            "still mid-drag, nothing should have moved yet"
        );
        let world = app.world_mut();
        assert!(world.get_entity(console_tab).is_ok(), "the tab button being dragged must survive the press frame, not get despawned by a premature rebuild");

        // Drag over to Scene's tab (in the left region) and release there.
        world.entity_mut(console_tab).insert(Interaction::None);
        world.entity_mut(scene_tab).insert(Interaction::Hovered);
        bv_editor_test_utils::step(&mut app, 1);
        bv_editor_test_utils::simulate_release(&mut app);
        bv_editor_test_utils::step(&mut app, 1);

        assert_eq!(app.world().resource::<dock::DockLayout>().region_of(dock::PanelId::Console), Some(dock::DockRegion::Left), "dropping Console's tab onto Scene Tree's should have merged them into the left region");
    }
}
