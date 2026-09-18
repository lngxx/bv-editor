//! `minimal_game` — an empty host game (a cube and a light) used to exercise
//! bv-editor at every phase, per docs/DESIGN.md section 9.
//!
//! Phase 0 milestone: this must open a normal Bevy window and not crash with
//! `EditorPlugin` added, on top of the normal game code below (which knows
//! nothing about the editor).

use bevy::prelude::*;
use bv_editor::EditorPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EditorPlugin)
        .add_systems(Startup, setup)
        .run();
}

/// Normal game setup: a cube, a light, and a camera. Doesn't know the editor exists.
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.3, 0.3))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));

    commands.spawn((
        PointLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
