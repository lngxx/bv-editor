//! `minimal_game` — a small host game (a group of cubes and a light) used to
//! exercise bv-editor at every phase, per docs/DESIGN.md section 9.
//!
//! Phase 0 milestone: this must open a normal Bevy window and not crash with
//! `EditorPlugin` added, on top of the normal game code below (which knows
//! nothing about the editor). Phase 2 added a small named parent/child
//! hierarchy (rather than one anonymous cube) so the Scene Tree panel has
//! something real to display.

use bevy::prelude::*;
use bv_editor::EditorPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EditorPlugin)
        .add_systems(Startup, setup)
        .run();
}

/// Normal game setup: a small cube hierarchy, a light, and a camera. Doesn't
/// know the editor exists.
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube_mesh = meshes.add(Cuboid::default());
    let cube_material = materials.add(Color::srgb(0.8, 0.3, 0.3));

    commands
        .spawn((Name::new("Cubes"), Transform::from_xyz(0.0, 0.5, 0.0), Visibility::default()))
        .with_children(|group| {
            group.spawn((
                Name::new("Cube A"),
                Mesh3d(cube_mesh.clone()),
                MeshMaterial3d(cube_material.clone()),
                Transform::from_xyz(-1.0, 0.0, 0.0),
            ));
            group.spawn((
                Name::new("Cube B"),
                Mesh3d(cube_mesh),
                MeshMaterial3d(cube_material),
                Transform::from_xyz(1.0, 0.0, 0.0),
            ));
        });

    commands.spawn((
        Name::new("Sun"),
        PointLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    commands.spawn((
        Name::new("Main Camera"),
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
