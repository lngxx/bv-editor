//! docs/UI_FEATURES.md F6: a way to register icons — bv-editor's own
//! built-ins and, later, extension-supplied ones — and draw one in
//! `bevy_ui`, without needing an icon-font build step or asset files that
//! don't exist in this repo yet (docs/DESIGN.md section 4: built-in `bevy`
//! mechanisms first).
//!
//! Chosen shape: a flat sprite sheet per `Handle<Image>` plus a pixel
//! [`Rect`] per icon, fed straight into [`ImageNode::rect`] — its own doc
//! comment calls this out as "an easy one-off alternative to using a
//! [`TextureAtlas`](bevy_image::TextureAtlas)", which is exactly what's
//! needed here and skips registering a whole extra `TextureAtlasLayout`
//! asset type. Only [`IconPlugin`]'s single built-in placeholder icon is
//! implemented so far — wiring real icons into the toolbar, a panel/tab
//! title bar (F5), or Scene Tree rows is deliberately left for whichever of
//! those lands next, per docs/UI_FEATURES.md's own advice not to wait on
//! every call site before landing the registry itself.

use std::collections::HashMap;

use bevy_app::{App, Plugin, Startup};
use bevy_asset::{Assets, Handle, RenderAssetUsages};
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_math::{Rect, Vec2};
use bevy_ui::widget::ImageNode;
use bevy_ui::{Node, Val};
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

/// Identifies a registered icon. A plain string key (rather than a closed
/// enum) so extensions can register their own icons without `bv_editor_ui`
/// knowing about them ahead of time — the same reasoning
/// `bv_editor_core::HotkeyRegistry` uses for its string action ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IconId(pub &'static str);

/// Where one icon lives: a sprite-sheet handle plus its pixel rect within
/// it. Several `IconId`s commonly share one `image` (one sheet, many icons).
#[derive(Debug, Clone)]
struct IconEntry {
    image: Handle<Image>,
    rect: Rect,
}

/// Maps [`IconId`] to where to find it. Built-in icons ([`IconPlugin`]) and
/// extension-registered icons share this one registry, keyed by string id.
#[derive(Resource, Default)]
pub struct IconRegistry {
    icons: HashMap<IconId, IconEntry>,
}

impl IconRegistry {
    /// Registers `id` to the pixel `rect` of `image`. Overwrites whatever
    /// was previously registered under `id`, same as re-registering a
    /// hotkey action.
    pub fn register(&mut self, id: IconId, image: Handle<Image>, rect: Rect) {
        self.icons.insert(id, IconEntry { image, rect });
    }

    /// Builds the [`ImageNode`] for `id`, or `None` if nothing registered
    /// it — callers treat that as "draw nothing", the same caution the
    /// shell's `*Slot`-less startup systems already use for a missing slot.
    pub fn image_node(&self, id: IconId) -> Option<ImageNode> {
        let entry = self.icons.get(&id)?;
        Some(ImageNode { image: entry.image.clone(), rect: Some(entry.rect), ..Default::default() })
    }
}

/// Spawns a square icon widget for `id` as a child of `parent`, `size_px` on
/// each side. Returns `None` (spawning nothing) if `id` isn't registered —
/// an unregistered icon id is a caller bug to fix, not a state to render a
/// fallback for.
pub fn spawn_icon(commands: &mut Commands, parent: Entity, registry: &IconRegistry, id: IconId, size_px: f32) -> Option<Entity> {
    let image_node = registry.image_node(id)?;
    Some(
        commands
            .spawn((image_node, Node { width: Val::Px(size_px), height: Val::Px(size_px), flex_shrink: 0.0, ..Default::default() }, ChildOf(parent)))
            .id(),
    )
}

/// One built-in icon shipped with `bv_editor` itself, so built-in panels
/// have something to point at before any extension registers its own. A
/// flat colored square, not real icon art — there's no icon design in this
/// repo yet — it exists to prove the registry/sprite-sheet/render pipeline
/// end-to-end, not to look good.
pub const PLACEHOLDER_ICON: IconId = IconId("bv_editor.placeholder");

const BUILT_IN_ICON_PX: u32 = 16;

/// Registers [`PLACEHOLDER_ICON`] against a freshly generated 1-icon sprite
/// sheet. Procedural rather than loaded from a file for the same reason
/// `bv_editor_viewport`'s render-target image is procedural: no asset
/// pipeline exists in this repo yet, and a flat square needs no external
/// tool to produce.
fn register_built_in_icons(mut registry: ResMut<IconRegistry>, mut images: ResMut<Assets<Image>>) {
    let size = Extent3d { width: BUILT_IN_ICON_PX, height: BUILT_IN_ICON_PX, depth_or_array_layers: 1 };
    let image = Image::new_fill(size, TextureDimension::D2, &[0xC0, 0xC0, 0xC0, 0xFF], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    let handle = images.add(image);

    let rect = Rect { min: Vec2::ZERO, max: Vec2::splat(BUILT_IN_ICON_PX as f32) };
    registry.register(PLACEHOLDER_ICON, handle, rect);
}

/// Adds [`IconRegistry`] and registers bv-editor's built-in icon(s). Its own
/// plugin (rather than folded straight into [`crate::EditorUiPlugin`]'s
/// `build`) so it can be added idempotently the same way
/// `DragAndDropPlugin`/`ScrollbarPlugin` are — `EditorUiPlugin` adds it by
/// default.
#[derive(Default)]
pub struct IconPlugin;

impl Plugin for IconPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IconRegistry>();
        app.add_systems(Startup, register_built_in_icons);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::{AssetApp, AssetPlugin};
    use bevy_ecs::hierarchy::Children;
    use bevy_ecs::system::RunSystemOnce;
    use bv_editor_test_utils::{headless_app, step};

    fn setup() -> bevy_app::App {
        let mut app = headless_app();
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Image>();
        app
    }

    #[test]
    fn unregistered_icon_resolves_to_nothing() {
        let registry = IconRegistry::default();
        assert!(registry.image_node(IconId("does.not.exist")).is_none());
    }

    #[test]
    fn registering_an_icon_makes_it_resolve_to_an_image_node_with_the_right_rect() {
        let mut app = setup();
        let rect = Rect { min: Vec2::new(4.0, 0.0), max: Vec2::new(20.0, 16.0) };
        let (registry, handle) = app
            .world_mut()
            .run_system_once(move |mut images: ResMut<Assets<Image>>| {
                let handle = images.add(Image::default());
                let mut registry = IconRegistry::default();
                registry.register(IconId("test.icon"), handle.clone(), rect);
                (registry, handle)
            })
            .expect("system should run");

        let node = registry.image_node(IconId("test.icon")).expect("registered icon should resolve");
        assert_eq!(node.image, handle);
        assert_eq!(node.rect, Some(rect));
    }

    #[test]
    fn icon_plugin_registers_the_placeholder_icon_at_startup() {
        let mut app = setup();
        app.add_plugins(IconPlugin);
        step(&mut app, 1);

        let registry = app.world().resource::<IconRegistry>();
        assert!(registry.image_node(PLACEHOLDER_ICON).is_some(), "IconPlugin should register the built-in placeholder icon at startup");
    }

    #[test]
    fn spawn_icon_spawns_a_correctly_sized_image_node_child() {
        let mut app = setup();
        app.add_plugins(IconPlugin);
        step(&mut app, 1);

        let parent = app.world_mut().spawn(Node::default()).id();
        let spawned = app
            .world_mut()
            .run_system_once(move |mut commands: Commands, registry: Res<IconRegistry>| spawn_icon(&mut commands, parent, &registry, PLACEHOLDER_ICON, 16.0))
            .expect("system should run")
            .expect("placeholder icon should be registered");

        let world = app.world();
        let node = world.get::<Node>(spawned).unwrap();
        assert_eq!(node.width, Val::Px(16.0));
        assert_eq!(node.height, Val::Px(16.0));
        assert!(world.get::<ImageNode>(spawned).unwrap().rect.is_some());

        let children = world.get::<Children>(parent).expect("parent should have gained the icon as a child");
        assert!(children.contains(&spawned));
    }

    #[test]
    fn spawn_icon_spawns_nothing_for_an_unregistered_id() {
        let mut app = setup();
        app.init_resource::<IconRegistry>();
        let parent = app.world_mut().spawn(Node::default()).id();

        let spawned = app
            .world_mut()
            .run_system_once(move |mut commands: Commands, registry: Res<IconRegistry>| spawn_icon(&mut commands, parent, &registry, IconId("does.not.exist"), 16.0))
            .expect("system should run");

        assert!(spawned.is_none());
        assert!(app.world().get::<Children>(parent).is_none(), "no child should have been spawned");
    }
}
