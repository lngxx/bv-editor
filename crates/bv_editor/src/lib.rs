//! `bv_editor` — the facade crate. A host game does:
//!
//! ```ignore
//! app.add_plugins(GamePlugin)   // normal game code, unaware of the editor
//!    .add_plugins(EditorPlugin); // one line adds the whole editor
//! ```
//!
//! It re-exports every bv-editor sub-crate so third-party extensions and
//! built-in panels are reachable through the same public API (see
//! docs/DESIGN.md sections 2 and 5) — this crate has no special access that a
//! `cargo add`-ed extension crate wouldn't also have.
//!
//! Phase 0: [`EditorPlugin`] only composes [`EditorCorePlugin`] and logs that
//! it loaded. `EditorUiPlugin`, the built-in panels, gizmos, and the viewport
//! are added here as their phases land.

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

/// The single plugin a host game adds to embed bv-editor.
///
/// Phase 0: composes [`EditorCorePlugin`] and logs successful load; that's
/// the entire Phase 0 milestone (docs/DESIGN.md section 9, "Phase 0").
#[derive(Default)]
pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EditorCorePlugin);
        info!("bv_editor: EditorPlugin loaded successfully");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_plugin_builds_without_panicking() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorPlugin);
        bv_editor_test_utils::step(&mut app, 1);
    }
}
