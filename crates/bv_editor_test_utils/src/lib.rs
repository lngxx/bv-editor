//! `bv_editor_test_utils` — headless testing harness for bv-editor.
//!
//! Every phase from Phase 0 onward is expected to have an automated test
//! built on top of this crate rather than a "look at it and see" milestone
//! (docs/DESIGN.md section 8.1). It wraps `App` + `MinimalPlugins` so tests
//! run with no window and no GPU, and gives phases a couple of standard ways
//! to advance time and fake input:
//!
//! - [`headless_app`] — an `App` with `MinimalPlugins` + `InputPlugin` + `StatesPlugin`, no window.
//! - [`step`] — call `App::update()` a fixed number of times.
//! - [`simulate_click`] — press the left mouse button and record a world-space position.
//! - [`simulate_key`] — press a keyboard key.
//!
//! `simulate_click`/`simulate_key` send real `MouseButtonInput`/`KeyboardInput`
//! events rather than writing the `ButtonInput` resources directly. That
//! matters: `bevy_input`'s own `mouse_button_input_system`/`keyboard_input_system`
//! unconditionally clear `just_pressed`/`just_released` at the start of every
//! `PreUpdate`, then replay that frame's queued events to repopulate them —
//! so a direct `ButtonInput::press()` call between two `step()`s is silently
//! wiped before any `Update` system (this crate's own systems included) ever
//! observes `just_pressed`, even though `pressed()` (unaffected by the
//! per-frame clear) looks correct. Sending the event instead lets the real
//! input pipeline set both correctly. There is still no camera, viewport, or
//! picking system to interpret [`SimulatedClick`]'s world position (that
//! lands in Phase 4); Phase 2's Scene Tree only needs `ButtonInput` itself.

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::mouse::{MouseButton, MouseButtonInput};
use bevy::input::{ButtonState, InputPlugin};
use bevy::math::Vec2;
use bevy::state::app::StatesPlugin;
use bevy::MinimalPlugins;

/// Build an `App` with `MinimalPlugins` + `InputPlugin` + `StatesPlugin` and no
/// window — safe to run in CI without a GPU or a display. `StatesPlugin` is
/// included because `EditorState` (`bv_editor_core`) and every state-gated
/// system after it need the `StateTransition` schedule to exist, and
/// `MinimalPlugins` alone does not provide it.
pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(InputPlugin);
    app.add_plugins(StatesPlugin);
    app
}

/// Call `App::update()` `frames` times.
pub fn step(app: &mut App, frames: u32) {
    for _ in 0..frames {
        app.update();
    }
}

/// The most recent simulated pointer click, in world space.
///
/// Phase 0 placeholder: real viewport/picking integration lands in Phase 4,
/// at which point this can be read by (or replaced with) whatever the
/// picking system produces.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct SimulatedClick(pub Vec2);

/// Simulate a left mouse click at `world_pos`. Call this, then [`step`] once:
/// `just_pressed(MouseButton::Left)` is true during that step (and `pressed`
/// stays true afterward, until something sends a release).
pub fn simulate_click(app: &mut App, world_pos: Vec2) {
    app.world_mut().insert_resource(SimulatedClick(world_pos));
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window: Entity::PLACEHOLDER,
    });
}

/// Simulate releasing the left mouse button (e.g. to finish a drag started
/// with [`simulate_click`]). Call this, then [`step`] once:
/// `just_released(MouseButton::Left)` is true during that step.
pub fn simulate_release(app: &mut App) {
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Released,
        window: Entity::PLACEHOLDER,
    });
}

/// Simulate a key press. Call this, then [`step`] once: `just_pressed(key)`
/// is true during that step.
pub fn simulate_key(app: &mut App, key: KeyCode) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::ButtonInput;

    #[test]
    fn headless_app_steps_without_panicking() {
        let mut app = headless_app();
        step(&mut app, 5);
    }

    #[test]
    fn simulate_click_presses_mouse_and_records_position() {
        let mut app = headless_app();
        simulate_click(&mut app, Vec2::new(1.0, 2.0));
        step(&mut app, 1);

        assert_eq!(*app.world().resource::<SimulatedClick>(), SimulatedClick(Vec2::new(1.0, 2.0)));
        // Both must hold: `just_pressed` is what click-once handlers (Phase 2's
        // Scene Tree, "Add Entity", "Delete") check, and it's the one that a
        // direct `ButtonInput::press()` call would get silently wrong.
        assert!(app.world().resource::<ButtonInput<MouseButton>>().just_pressed(MouseButton::Left));
        assert!(app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    }

    #[test]
    fn simulated_click_is_only_just_pressed_for_one_frame() {
        let mut app = headless_app();
        simulate_click(&mut app, Vec2::ZERO);
        step(&mut app, 1);
        step(&mut app, 1);

        assert!(!app.world().resource::<ButtonInput<MouseButton>>().just_pressed(MouseButton::Left));
        assert!(app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    }

    #[test]
    fn simulate_release_clears_pressed_and_sets_just_released() {
        let mut app = headless_app();
        simulate_click(&mut app, Vec2::ZERO);
        step(&mut app, 1);

        simulate_release(&mut app);
        step(&mut app, 1);

        assert!(app.world().resource::<ButtonInput<MouseButton>>().just_released(MouseButton::Left));
        assert!(!app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    }

    #[test]
    fn simulate_key_presses_key() {
        let mut app = headless_app();
        simulate_key(&mut app, KeyCode::Space);
        step(&mut app, 1);

        assert!(app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::Space));
        assert!(app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::Space));
    }
}
