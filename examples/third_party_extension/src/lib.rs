//! `third_party_extension` — stands in for a real, out-of-workspace user of
//! the bv-editor extension API (docs/DESIGN.md sections 3 and 5). It lives
//! under `examples/` for convenience, but is built as an ordinary crate that
//! only depends on public bv-editor APIs, the same way a real third party
//! would with `cargo add`.
//!
//! Phase 0: just a `Plugin` that logs. Once `bv_editor_extension` exists
//! (Phase 7), this crate registers a real panel through `EditorAppExt` to
//! prove the extension API works for someone outside the core crates
//! ("dogfooding" — the built-in panels get refactored to use the very same
//! API in that phase).

use bevy_app::{App, Plugin};
use bevy_log::info;

/// A minimal example extension plugin.
#[derive(Default)]
pub struct ThirdPartyExtensionPlugin;

impl Plugin for ThirdPartyExtensionPlugin {
    fn build(&self, _app: &mut App) {
        info!("third_party_extension: ThirdPartyExtensionPlugin loaded");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_plugin_builds_without_panicking() {
        let mut app = bv_editor_test_utils::headless_app();
        app.add_plugins(ThirdPartyExtensionPlugin);
        bv_editor_test_utils::step(&mut app, 1);
    }
}
