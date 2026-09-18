//! `bv_editor_test_utils` — headless testing harness for bv-editor.
//!
//! Every phase from Phase 0 onward is expected to have an automated test
//! built on top of this crate rather than a "look at it and see" milestone
//! (docs/DESIGN.md section 8.1). It wraps `App` + `MinimalPlugins` so tests
//! run with no window and no GPU, and gives phases a couple of standard ways
//! to advance time and fake input:
//!
//! - [`headless_app`] — an `App` with `MinimalPlugins` + `InputPlugin`, no window.
//! - [`step`] — call `App::update()` a fixed number of times.
//! - [`simulate_click`] — press the left mouse button and record a world-space position.
//! - [`simulate_key`] — press a keyboard key.
//!
//! `simulate_click`/`simulate_key` only touch the raw `ButtonInput` resources
//! for now: there is no camera, viewport, or picking system yet to consume
//! them (those land in Phase 2 and Phase 4). Panels and the viewport will
//! read [`SimulatedClick`] and the standard `ButtonInput` resources once they
//! exist.

use bevy::app::App;
use bevy::ecs::resource::Resource;
use bevy::input::{ButtonInput, InputPlugin, keyboard::KeyCode, mouse::MouseButton};
use bevy::math::Vec2;
use bevy::MinimalPlugins;

/// Build an `App` with `MinimalPlugins` + `InputPlugin` and no window — safe
/// to run in CI without a GPU or a display.
pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(InputPlugin);
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

/// Simulate a left mouse click at `world_pos`.
pub fn simulate_click(app: &mut App, world_pos: Vec2) {
    app.world_mut().insert_resource(SimulatedClick(world_pos));
    let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    mouse.press(MouseButton::Left);
}

/// Simulate a key press.
pub fn simulate_key(app: &mut App, key: KeyCode) {
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.press(key);
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    }

    #[test]
    fn simulate_key_presses_key() {
        let mut app = headless_app();
        simulate_key(&mut app, KeyCode::Space);
        step(&mut app, 1);

        assert!(app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::Space));
    }
}
