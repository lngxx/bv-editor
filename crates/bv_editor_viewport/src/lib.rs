//! `bv_editor_viewport` — renders a 3D preview of the scene into the shell's
//! Viewport slot (docs/DESIGN.md sections 4, 7.1, and 9 "Phase 4").
//!
//! This is a deliberately minimal slice of Phase 4, not the whole thing: an
//! [`EditorCamera`] at a fixed default position renders into the Viewport
//! panel through `bevy_ui`'s own [`ViewportNode`] widget, which also handles
//! resizing the render target to match the panel automatically whenever it
//! changes size — no custom resize system needed (this sidesteps the exact
//! risk docs/DESIGN.md section 10 calls out: "ต้อง handle resize ของ Image
//! ให้ตรงกับขนาด panel ทุกเฟรมที่ panel เปลี่ยนขนาด").
//!
//! Still missing, and still real Phase 4 scope: the orbit/pan/zoom camera
//! controller, the extensible gizmo API (section 7) and its manipulators,
//! and viewport picking. The camera also isn't yet gated to
//! `EditorState::Editing`/`Paused` (section 7.1) — nothing transitions the
//! state to `Playing` until Phase 9 wires that up, so that gate would be
//! untested dead code today.

use bevy_app::{App, Plugin, Startup};
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::{Camera, Camera3d, RenderTarget};
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_math::Vec3;
use bevy_render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy_transform::components::Transform;
use bevy_ui::widget::ViewportNode;
use bevy_ui::{Node, Val};

use bv_editor_core::{EditorCamera, EditorOnly};
use bv_editor_ui::{EditorShellSet, ViewportSlot};

/// Default render target resolution before the first UI layout pass; after
/// that, `ViewportNode`'s own system keeps it matched to the panel's actual
/// size.
const DEFAULT_VIEWPORT_SIZE: (u32, u32) = (960, 540);

/// Where the editor camera starts, framing the world origin. A fixed
/// preview position for now; the orbit/pan/zoom controller is still to come.
const DEFAULT_CAMERA_POS: Vec3 = Vec3::new(6.0, 5.0, 10.0);

/// Spawns the editor's own preview camera and wires it into the shell's
/// Viewport slot.
#[derive(Default)]
pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        // `ViewportSlot` is spawned by `bv_editor_ui`'s own `Startup` system;
        // see `bv_editor_scene_panel` for why this ordering is required
        // rather than assumed.
        app.add_systems(Startup, spawn_editor_camera_and_viewport.after(EditorShellSet));
    }
}

fn spawn_editor_camera_and_viewport(mut commands: Commands, mut images: ResMut<Assets<Image>>, slots: Query<Entity, With<ViewportSlot>>) {
    let Ok(slot) = slots.single() else { return };

    let size = Extent3d { width: DEFAULT_VIEWPORT_SIZE.0, height: DEFAULT_VIEWPORT_SIZE.1, depth_or_array_layers: 1 };
    let mut image = Image::new_fill(size, TextureDimension::D2, &[20, 20, 22, 255], TextureFormat::Bgra8UnormSrgb, RenderAssetUsages::default());
    // Required to use the image as a render target rather than a plain asset.
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let image_handle = images.add(image);

    let camera = commands
        .spawn((
            EditorCamera,
            EditorOnly,
            Camera3d::default(),
            Camera::default(),
            RenderTarget::Image(image_handle.into()),
            Transform::from_translation(DEFAULT_CAMERA_POS).looking_at(Vec3::ZERO, Vec3::Y),
        ))
        .id();

    // A content child below the Phase 1 "Viewport" title, matching the
    // title-then-content pattern the Scene Tree panel already uses.
    commands.entity(slot).with_children(|panel| {
        panel.spawn((ViewportNode::new(camera), Node { flex_grow: 1.0, width: Val::Percent(100.0), ..Default::default() }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::{AssetApp, AssetPlugin};
    use bv_editor_test_utils::{headless_app, step};

    /// `MinimalPlugins` doesn't include asset support; this crate is the
    /// first to need `Assets<Image>` (for the render target), so it adds
    /// `AssetPlugin` + registers `Image` itself rather than pushing that
    /// onto `bv_editor_test_utils` for phases that don't need it.
    fn setup() -> App {
        let mut app = headless_app();
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Image>();
        app.world_mut().spawn(ViewportSlot);
        app.add_plugins(ViewportPlugin);
        app
    }

    #[test]
    fn builds_without_panicking() {
        let mut app = setup();
        step(&mut app, 1);
    }

    #[test]
    fn spawns_an_editor_camera_and_a_viewport_node_in_the_slot() {
        let mut app = setup();
        step(&mut app, 1);

        let world = app.world_mut();
        let mut cameras = world.query_filtered::<Entity, (With<EditorCamera>, With<EditorOnly>, With<Camera3d>)>();
        assert_eq!(cameras.iter(world).count(), 1, "exactly one editor camera should exist");
        let camera = cameras.iter(world).next().unwrap();

        let mut viewport_nodes = world.query::<&ViewportNode>();
        let node = viewport_nodes.iter(world).next().expect("a ViewportNode should have been spawned");
        assert_eq!(node.camera, Some(camera), "the ViewportNode should point at the spawned editor camera");
    }

    #[test]
    fn does_nothing_if_no_viewport_slot_exists() {
        // Same "startup ordering" caution as bv_editor_scene_panel: if this
        // ever ran before `bv_editor_ui` spawned the slot, it must no-op
        // rather than panic.
        let mut app = headless_app();
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Image>();
        app.add_plugins(ViewportPlugin);
        step(&mut app, 1);

        let world = app.world_mut();
        let mut cameras = world.query::<&EditorCamera>();
        assert_eq!(cameras.iter(world).count(), 0);
    }
}
