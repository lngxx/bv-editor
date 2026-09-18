//! `bv_editor_core` — editor state, schedule sets, central resources, and the
//! `EditorCorePlugin` entrypoint.
//!
//! Phase 0 only had a plugin that logged its own load. Phase 1 adds the
//! first two pieces of shared state every later phase builds on top of:
//!
//! - [`EditorState`] — Editing / Paused / Playing, referenced throughout
//!   docs/DESIGN.md (sections 7.1, 8.2, 8.6) even though the actual play/pause
//!   *behavior* (toggling it, hiding editor UI, swapping cameras) isn't wired
//!   up until Phase 9. The type exists now so nothing that depends on it
//!   (starting with `HotkeyRegistry` below) needs to change shape later.
//! - [`hotkey::HotkeyRegistry`] / [`hotkey::HotkeyAppExt`] — the central
//!   place hotkeys get reserved (section 8.2).
//!
//! Phase 2 adds the other two pieces of shared state sections 6 and 8.3
//! describe:
//!
//! - [`Selection`] — the one resource both the Scene Tree (Phase 2) and,
//!   later, viewport picking (Phase 4) write to.
//! - [`EditorOnly`] — marks entities the editor itself created (its own
//!   camera, gizmos, grid lines) so the Scene Tree can skip them now and
//!   scene export (Phase 6) can filter them out later.

mod hotkey;
mod selection;

pub use hotkey::{HotkeyAppExt, HotkeyDescriptor, HotkeyId, HotkeyRegistry};
pub use selection::Selection;

use bevy_app::{App, Plugin};
use bevy_ecs::component::Component;
use bevy_log::info;
use bevy_state::app::AppExtStates;
use bevy_state::state::States;

/// Marks an entity as created by the editor itself rather than the host
/// game or a loaded scene: the editor's own camera, gizmo meshes, grid
/// lines, and Scene Tree UI rows. Never shown in the Scene Tree (Phase 2)
/// and must never be included when exporting a scene (Phase 6, mandatory —
/// see docs/DESIGN.md section 8.3).
#[derive(Component, Clone, Copy, Default, Debug)]
pub struct EditorOnly;

/// The editor's own high-level mode, independent of whatever state machine
/// the host game runs. `Editing`/`Paused` are equivalent for most systems
/// today (both mean "the editor is visible and driving the world"); Phase 9
/// is what gives `Paused` and `Playing` real meaning (running the host
/// game's own systems, hiding editor chrome, swapping to the game camera).
#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum EditorState {
    #[default]
    Editing,
    Paused,
    Playing,
}

/// Registers editor-wide state, schedule sets, and central resources.
///
/// The top-level `EditorPlugin` that a host game adds lives in the
/// `bv_editor` facade crate and composes this plugin together with the rest
/// of the editor (see docs/DESIGN.md section 2).
#[derive(Default)]
pub struct EditorCorePlugin;

impl Plugin for EditorCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<EditorState>();
        app.init_resource::<HotkeyRegistry>();
        app.init_resource::<Selection>();
        info!("bv_editor_core: EditorCorePlugin loaded");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_state::state::State;

    #[test]
    fn editor_core_plugin_builds_without_panicking() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorCorePlugin);
        bv_editor_test_utils::step(&mut app, 1);
    }

    #[test]
    fn editor_state_defaults_to_editing() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorCorePlugin);
        bv_editor_test_utils::step(&mut app, 1);

        assert_eq!(*app.world().resource::<State<EditorState>>().get(), EditorState::Editing);
    }

    #[test]
    fn hotkey_registry_resource_exists_after_build() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorCorePlugin);

        assert!(app.world().get_resource::<HotkeyRegistry>().is_some());
    }

    #[test]
    fn selection_resource_exists_after_build() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(EditorCorePlugin);

        assert!(app.world().get_resource::<Selection>().is_some());
    }
}
