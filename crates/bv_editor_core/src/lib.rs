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
//! `Selection` and the `EditorOnly` marker (section 8.3) are still not here;
//! they land with the crates that first need them (Phase 2 and Phase 4).

mod hotkey;

pub use hotkey::{HotkeyAppExt, HotkeyDescriptor, HotkeyId, HotkeyRegistry};

use bevy_app::{App, Plugin};
use bevy_log::info;
use bevy_state::app::AppExtStates;
use bevy_state::state::States;

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
}
