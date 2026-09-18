//! `HotkeyRegistry` — the central place every extension reserves a hotkey
//! through, instead of reading `ButtonInput<KeyCode>` directly.
//!
//! See docs/DESIGN.md section 8.2. Phase 1 only builds the mechanism: there
//! is no real hotkey registered by bv-editor itself yet (that starts once
//! panels/manipulators exist), but every later phase is expected to reserve
//! through [`HotkeyAppExt::register_hotkey`] from day one so nobody shortcuts
//! straight to `KeyCode` checks.

use std::collections::HashMap;

use bevy_app::App;
use bevy_ecs::resource::Resource;
use bevy_input::keyboard::KeyCode;
use bevy_log::warn;

use crate::EditorState;

/// Stable identity for a hotkey reservation, e.g. `"scene_panel.delete_entity"`.
/// Used for collision warnings today, and will back a future "Keybindings" UI.
pub type HotkeyId = &'static str;

/// A single hotkey reservation: which key, in which [`EditorState`], owned by whom.
#[derive(Debug, Clone, Copy)]
pub struct HotkeyDescriptor {
    pub id: HotkeyId,
    pub key: KeyCode,
    pub when: EditorState,
}

/// Central registry of reserved hotkeys, keyed by `(key, state)` so the same
/// key can mean different things in different [`EditorState`]s without
/// colliding.
#[derive(Resource, Default, Debug)]
pub struct HotkeyRegistry {
    entries: HashMap<(KeyCode, EditorState), HotkeyId>,
}

impl HotkeyRegistry {
    /// Reserve a hotkey. If the `(key, when)` pair is already taken, logs a
    /// startup warning naming both the existing and the rejected owner
    /// instead of silently letting one of them never fire.
    pub fn register(&mut self, descriptor: HotkeyDescriptor) {
        let slot = (descriptor.key, descriptor.when);
        if let Some(existing) = self.entries.get(&slot) {
            warn!(
                "HotkeyRegistry: {:?} in {:?} is already reserved by `{existing}` — `{}` will not fire",
                descriptor.key, descriptor.when, descriptor.id
            );
            return;
        }
        self.entries.insert(slot, descriptor.id);
    }

    /// The id that owns `key` in `when`, if any.
    pub fn owner(&self, key: KeyCode, when: EditorState) -> Option<HotkeyId> {
        self.entries.get(&(key, when)).copied()
    }

    /// Whether `key` is reserved by anyone in `when`.
    pub fn is_registered(&self, key: KeyCode, when: EditorState) -> bool {
        self.entries.contains_key(&(key, when))
    }

    /// Number of reservations currently held, across all states.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has reserved a hotkey yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// `app.register_hotkey(HotkeyDescriptor { .. })` — the only sanctioned way
/// to reserve a hotkey, per docs/DESIGN.md section 8.2.
pub trait HotkeyAppExt {
    fn register_hotkey(&mut self, descriptor: HotkeyDescriptor) -> &mut Self;
}

impl HotkeyAppExt for App {
    fn register_hotkey(&mut self, descriptor: HotkeyDescriptor) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(HotkeyRegistry::default)
            .register(descriptor);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_distinct_keys() {
        let mut registry = HotkeyRegistry::default();
        registry.register(HotkeyDescriptor { id: "a", key: KeyCode::KeyA, when: EditorState::Editing });
        registry.register(HotkeyDescriptor { id: "b", key: KeyCode::KeyB, when: EditorState::Editing });

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.owner(KeyCode::KeyA, EditorState::Editing), Some("a"));
        assert_eq!(registry.owner(KeyCode::KeyB, EditorState::Editing), Some("b"));
    }

    #[test]
    fn same_key_in_different_states_does_not_collide() {
        let mut registry = HotkeyRegistry::default();
        registry.register(HotkeyDescriptor { id: "editing.delete", key: KeyCode::Delete, when: EditorState::Editing });
        registry.register(HotkeyDescriptor { id: "paused.delete", key: KeyCode::Delete, when: EditorState::Paused });

        assert_eq!(registry.owner(KeyCode::Delete, EditorState::Editing), Some("editing.delete"));
        assert_eq!(registry.owner(KeyCode::Delete, EditorState::Paused), Some("paused.delete"));
        assert!(!registry.is_registered(KeyCode::Delete, EditorState::Playing));
    }

    #[test]
    fn colliding_reservation_keeps_the_first_owner() {
        let mut registry = HotkeyRegistry::default();
        registry.register(HotkeyDescriptor { id: "first", key: KeyCode::KeyA, when: EditorState::Editing });
        registry.register(HotkeyDescriptor { id: "second", key: KeyCode::KeyA, when: EditorState::Editing });

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.owner(KeyCode::KeyA, EditorState::Editing), Some("first"));
    }

    #[test]
    fn register_hotkey_app_ext_inserts_the_resource_lazily() {
        let mut app = bv_editor_test_utils::headless_app();
        app.register_hotkey(HotkeyDescriptor { id: "a", key: KeyCode::KeyA, when: EditorState::Editing });

        let registry = app.world().resource::<HotkeyRegistry>();
        assert_eq!(registry.owner(KeyCode::KeyA, EditorState::Editing), Some("a"));
    }
}
