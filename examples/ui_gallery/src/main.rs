//! `ui_gallery` — not a phase-progression game like `minimal_game`; a
//! standalone host app whose only purpose is to give a human something to
//! look at and click on while testing bv-editor's own custom UI widgets
//! (docs/UI_FEATURES.md). Nothing here exercises standard Bevy UI — only
//! the editor chrome bv-editor itself adds:
//!
//! - **Scene Tree row hover/selection highlight** (F1) — hover any row,
//!   click to select, notice the two colors never fight over the same row.
//! - **Splitters between shell zones** (F4) — drag the thin bars between
//!   Scene Tree/Viewport/Components and between Project Files/Console.
//! - **Drag-and-drop reparenting** in the Scene Tree — drag a row onto
//!   another to reparent it (`bv_editor_ui`'s generic drag-and-drop
//!   framework, docs/DESIGN.md section 8.5).
//! - **Every `bv_editor_reflect_ui` field editor kind at once** — select
//!   "Crate A" or "Crate B" (both carry [`WidgetShowcase`]) to see String,
//!   Bool (click-to-toggle), F32, Vec3 (three F32 sub-fields), Color (four
//!   F32 channel sub-fields), and the read-only `{:?}` fallback for a field
//!   type with no dedicated editor yet, side by side in the Components panel.
//!
//! The scene hierarchy is intentionally wider/deeper than `minimal_game`'s
//! (multiple groups, a nested child, and a handful of flat "Marker N" roots)
//! so there's enough Scene Tree row variety to test hover/selection across
//! many rows now, and to double as the manual test case once F2 (Scene Tree
//! scrollbar) lands.

use bevy::prelude::*;
use bv_editor::EditorPlugin;

/// Touches every field kind `bv_editor_reflect_ui` knows how to render, so
/// selecting an entity that has it shows every editor widget at once:
/// `label` (String), `enabled` (Bool), `speed` (F32), `offset` (Vec3, three
/// F32 sub-fields), `tint` (Color, four F32 channel sub-fields), and `uv`
/// (Vec2 — not a field kind the Inspector has a dedicated editor for, so it
/// renders as the read-only `{:?}` fallback).
#[derive(Component, Reflect)]
#[reflect(Component)]
struct WidgetShowcase {
    label: String,
    enabled: bool,
    speed: f32,
    offset: Vec3,
    tint: Color,
    uv: Vec2,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "bv-editor — UI Widget Gallery".into(), ..default() }),
            ..default()
        }))
        .add_plugins(EditorPlugin::default())
        .register_type::<WidgetShowcase>()
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let cube_mesh = meshes.add(Cuboid::default());
    let crate_material = materials.add(Color::srgb(0.8, 0.6, 0.3));

    commands.spawn((Name::new("Props"), Transform::default(), Visibility::default())).with_children(|group| {
        group
            .spawn((
                Name::new("Crate A"),
                Mesh3d(cube_mesh.clone()),
                MeshMaterial3d(crate_material.clone()),
                Transform::from_xyz(-2.0, 0.5, 0.0),
            ))
            .insert(WidgetShowcase {
                label: "crate_a".to_string(),
                enabled: true,
                speed: 1.5,
                offset: Vec3::new(0.1, 0.2, 0.3),
                tint: Color::srgb(0.8, 0.2, 0.2),
                uv: Vec2::new(0.0, 1.0),
            })
            .with_children(|crate_a| {
                crate_a.spawn((Name::new("Crate A Lid"), Transform::from_xyz(0.0, 0.6, 0.0)));
            });

        group
            .spawn((
                Name::new("Crate B"),
                Mesh3d(cube_mesh.clone()),
                MeshMaterial3d(crate_material.clone()),
                Transform::from_xyz(0.0, 0.5, 0.0),
            ))
            .insert(WidgetShowcase {
                label: "crate_b".to_string(),
                enabled: false,
                speed: 0.0,
                offset: Vec3::ZERO,
                tint: Color::srgb(0.2, 0.6, 0.8),
                uv: Vec2::ZERO,
            });

        group.spawn((Name::new("Crate C"), Mesh3d(cube_mesh), MeshMaterial3d(crate_material), Transform::from_xyz(2.0, 0.5, 0.0)));
    });

    commands.spawn((Name::new("Lighting"), Transform::default(), Visibility::default())).with_children(|group| {
        group.spawn((Name::new("Key Light"), PointLight { shadow_maps_enabled: true, ..default() }, Transform::from_xyz(4.0, 8.0, 4.0)));
        group.spawn((Name::new("Fill Light"), PointLight::default(), Transform::from_xyz(-4.0, 5.0, -2.0)));
    });

    commands.spawn((Name::new("Camera Rig"), Transform::default(), Visibility::default())).with_children(|group| {
        group.spawn((Name::new("Main Camera"), Camera3d::default(), Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y)));
    });

    // Flat extra roots purely to pad the Scene Tree's row count — good for
    // eyeballing hover/selection across many rows now, and doubles as the
    // manual test case once F2 (Scene Tree scrollbar) lands.
    for i in 1..=8 {
        commands.spawn((Name::new(format!("Marker {i}")), Transform::from_xyz(i as f32, 0.0, -3.0)));
    }
}
