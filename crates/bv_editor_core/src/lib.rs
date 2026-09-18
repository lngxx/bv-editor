//! `bv_editor_core` — editor state, schedule sets, central resources, and the
//! `EditorCorePlugin` entrypoint.
//!
//! Phase 0: only [`EditorCorePlugin`] exists, and it does nothing but confirm
//! it loaded. `EditorState`, `Selection`, `HotkeyRegistry`, and the
//! `EditorOnly` marker described in docs/DESIGN.md (sections 7-8) arrive in
//! later phases as the crates that need them are built.

use bevy_app::{App, Plugin};
use bevy_log::info;

/// Registers editor-wide state, schedule sets, and central resources.
///
/// This is the crate that every other bv-editor crate will eventually depend
/// on for shared types (`Selection`, `EditorState`, `EditorOnly`, ...). The
/// top-level `EditorPlugin` that a host game adds lives in the `bv_editor`
/// facade crate and composes this plugin together with the rest of the
/// editor (see docs/DESIGN.md section 2).
#[derive(Default)]
pub struct EditorCorePlugin;

impl Plugin for EditorCorePlugin {
    fn build(&self, _app: &mut App) {
        info!("bv_editor_core: EditorCorePlugin loaded");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_core_plugin_builds_without_panicking() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorCorePlugin);
        bv_editor_test_utils::step(&mut app, 1);
    }
}
