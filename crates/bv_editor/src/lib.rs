//! `bv_editor` — the facade crate. A host game does:
//!
//! ```ignore
//! app.add_plugins(GamePlugin)   // normal game code, unaware of the editor
//!    .add_plugins(EditorPlugin::default()); // one line adds the whole editor
//! ```
//!
//! It re-exports every bv-editor sub-crate so third-party extensions and
//! built-in panels are reachable through the same public API (see
//! docs/DESIGN.md sections 2 and 5) — this crate has no special access that a
//! `cargo add`-ed extension crate wouldn't also have.
//!
//! Phase 0 gave [`EditorPlugin`] just [`EditorCorePlugin`] and a log line.
//! Phase 1 added [`EditorUiPlugin`], which spawns the fixed shell layout (top
//! toolbar, Scene Tree, viewport, Components, Project Files, Console, status
//! bar) with resizable splitters between zones. Phase 2 adds
//! [`bv_editor_scene_panel::ScenePanelPlugin`], which fills the Scene Tree
//! slot in with real content. Phase 3 adds [`InspectorPanelPlugin`], which
//! fills the Components slot with a `bevy_reflect`-driven field editor for
//! the selected entity. The other built-in panels, gizmos, and the viewport
//! itself are still empty crates and land in later phases.

use bevy::app::{App, Plugin};
use bevy::log::info;

pub use bv_editor_core as core;
pub use bv_editor_ui as ui;
pub use bv_editor_reflect_ui as reflect_ui;
pub use bv_editor_extension as extension;
pub use bv_editor_undo as undo;
pub use bv_editor_scene_panel as scene_panel;
pub use bv_editor_inspector_panel as inspector_panel;
pub use bv_editor_assets_panel as assets_panel;
pub use bv_editor_console_panel as console_panel;
pub use bv_editor_gizmo_api as gizmo_api;
pub use bv_editor_gizmos_builtin as gizmos_builtin;
pub use bv_editor_viewport as viewport;

pub use bv_editor_core::EditorCorePlugin;
pub use bv_editor_inspector_panel::InspectorPanelPlugin;
pub use bv_editor_scene_panel::ScenePanelPlugin;
pub use bv_editor_ui::EditorUiPlugin;
pub use bv_editor_viewport::ViewportPlugin;

/// The single plugin a host game adds to embed bv-editor.
///
/// Composes [`EditorCorePlugin`] (state, `HotkeyRegistry`), [`EditorUiPlugin`]
/// (the shell layout), [`ScenePanelPlugin`] (the Scene Tree), and
/// [`ViewportPlugin`] (the 3D preview camera), and logs successful load.
///
/// `ui_scale` forwards to [`EditorUiPlugin::ui_scale`] — the whole shell's
/// size, uniformly, defaulting to half the unscaled `bevy_ui` size:
/// `app.add_plugins(EditorPlugin { ui_scale: 0.7, ..default() })` to change it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorPlugin {
    pub ui_scale: f32,
}

impl Default for EditorPlugin {
    fn default() -> Self {
        Self { ui_scale: EditorUiPlugin::default().ui_scale }
    }
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EditorCorePlugin);
        app.add_plugins(EditorUiPlugin { ui_scale: self.ui_scale });
        app.add_plugins(ScenePanelPlugin);
        app.add_plugins(InspectorPanelPlugin);
        app.add_plugins(ViewportPlugin);
        info!("bv_editor: EditorPlugin loaded successfully");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::image::Image;

    /// `bv_editor_test_utils::headless_app()` is `MinimalPlugins` and has no
    /// asset support; `EditorPlugin` now includes `ViewportPlugin`, which
    /// needs `Assets<Image>` for its render target, so the real composed
    /// plugin can't be tested headlessly without adding that first.
    fn setup() -> App {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Image>();
        app
    }

    #[test]
    fn editor_plugin_builds_without_panicking() {
        let mut app = setup();
        app.add_plugins(EditorPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);
    }

    /// Regression test for a real bug the individual crates' own tests all
    /// missed: `EditorUiPlugin` and `ScenePanelPlugin` each register a
    /// `Startup` system (spawn the shell; spawn the Scene Tree's chrome into
    /// one of the shell's slots), and `Startup` systems from *different*
    /// plugins have no guaranteed relative order unless something says so
    /// (see `bv_editor_ui::EditorShellSet`). `bv_editor_scene_panel`'s own
    /// tests never caught this because they hand-spawn a bare
    /// `ScenePanelSlot` before adding `ScenePanelPlugin`, sidestepping the
    /// ordering question entirely — only composing the two plugins for real,
    /// the way `EditorPlugin` does, exercises it.
    #[test]
    fn scene_panel_chrome_is_actually_spawned_through_the_real_editor_plugin() {
        use bevy::ecs::hierarchy::Children;
        use bevy::ecs::query::With;

        let mut app = setup();
        app.add_plugins(EditorPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let mut slots = world.query_filtered::<&Children, With<bv_editor_ui::ScenePanelSlot>>();
        let children = slots.single(world).expect("ScenePanelSlot should exist");
        // Phase 1 alone gives it just the title text; Phase 2's chrome adds
        // the Add/Delete toolbar row and the (possibly still-empty) rows
        // container on top of that.
        assert!(children.len() >= 3, "expected the Scene Tree's own chrome on top of the Phase 1 title, got {} children", children.len());
    }

    /// Same regression guard as `scene_panel_chrome_is_actually_spawned_through_the_real_editor_plugin`,
    /// for `InspectorPanelPlugin`'s own `Startup` chrome-spawn system.
    #[test]
    fn inspector_panel_chrome_is_actually_spawned_through_the_real_editor_plugin() {
        use bevy::ecs::hierarchy::Children;
        use bevy::ecs::query::With;

        let mut app = setup();
        app.add_plugins(EditorPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let mut slots = world.query_filtered::<&Children, With<bv_editor_ui::InspectorPanelSlot>>();
        let children = slots.single(world).expect("InspectorPanelSlot should exist");
        // Phase 1 alone gives it just the title text; Phase 3's chrome adds
        // the (possibly still-empty) body container on top of that.
        assert!(children.len() >= 2, "expected the Inspector's own chrome on top of the Phase 1 title, got {} children", children.len());
    }

    /// End-to-end: composed through the real `EditorPlugin` (not a bare
    /// slot, per the ordering guard above), selecting an entity with
    /// `Transform` should make its fields show up in the Components panel.
    #[test]
    fn selecting_an_entity_through_the_real_editor_plugin_populates_the_inspector() {
        use bevy::transform::components::Transform;

        let mut app = setup();
        app.add_plugins(EditorPlugin::default());
        bv_editor_test_utils::step(&mut app, 1);

        let cube = app.world_mut().spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();
        app.world_mut().resource_mut::<bv_editor_core::Selection>().select_only(cube);
        bv_editor_test_utils::step(&mut app, 1);

        let world = app.world_mut();
        let mut fields = world.query::<&bv_editor_reflect_ui::FieldHandle>();
        assert!(fields.iter(world).count() > 0, "expected the Inspector to render at least one field for the selected entity's Transform");
    }
}
