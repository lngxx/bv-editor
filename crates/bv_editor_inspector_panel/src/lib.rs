//! `bv_editor_inspector_panel` — built-in Inspector panel, driven by
//! `bevy_reflect` (docs/DESIGN.md section 9, Phase 3).
//!
//! Mounts into [`bv_editor_ui::InspectorPanelSlot`] the same way
//! `bv_editor_scene_panel` mounts into `ScenePanelSlot`. On every change to
//! [`Selection`], it walks the selected entity's archetype, keeps only the
//! components that are both reflect-registered (`AppTypeRegistry`) and have
//! `#[reflect(Component)]` data, and for each one asks
//! `bv_editor_reflect_ui::spawn_component_fields` to render its fields —
//! this panel owns component *listing*, `bv_editor_reflect_ui` owns field
//! *editing*, matching the crate split in docs/DESIGN.md section 3.
//!
//! Components that aren't reflect-registered (most render/asset components —
//! `Mesh3d`, `Visibility`, etc. — nothing in this workspace registers them)
//! simply don't appear in the list yet. Phase 3's own doc comment only
//! requires the field types `f32`/`bool`/`String`/`Vec3`/`Color`/`Entity` to
//! be editable, which is a `bevy_reflect`-first framing to begin with;
//! widening component coverage (e.g. a non-reflect fallback row showing just
//! the name) is future work, not a Phase 3 requirement.

use bevy_app::{App, Plugin, Startup, Update};
use bevy_color::Color;
use bevy_ecs::component::ComponentId;
use bevy_ecs::prelude::*;
use bevy_ecs::reflect::{AppTypeRegistry, ReflectComponent};
use bevy_ecs::world::CommandQueue;
use bevy_text::TextColor;
use bevy_transform::components::Transform;
use bevy_ui::prelude::*;

use bv_editor_core::Selection;
use bv_editor_reflect_ui::spawn_component_fields;
use bv_editor_ui::InspectorPanelSlot;

const SECTION_HEADER_COLOR: Color = Color::srgb(0.85, 0.85, 0.85);
const EMPTY_HINT_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);

/// Forces [`rebuild_inspector_ui`] to redraw on its next run. Starts `true`
/// so the (possibly already-nonempty) selection is reflected on the first
/// frame, same convention as `bv_editor_scene_panel::SceneTreeDirty`.
#[derive(Resource)]
struct InspectorDirty(bool);

impl Default for InspectorDirty {
    fn default() -> Self {
        Self(true)
    }
}

/// Where component sections get spawned as children — under
/// [`InspectorPanelSlot`], separate from the slot's own Phase 1 title.
#[derive(Component)]
struct InspectorBody;

/// Marks the "(no selection)" hint row so it can be told apart from a real
/// component section header when the body is despawned and rebuilt.
#[derive(Component)]
struct EmptySelectionHint;

/// Spawns the Inspector's own chrome (an empty body container) into the
/// Phase 1 shell, then rebuilds it whenever [`Selection`] changes.
#[derive(Default)]
pub struct InspectorPanelPlugin;

impl Plugin for InspectorPanelPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bv_editor_reflect_ui::ReflectUiPlugin>() {
            app.add_plugins(bv_editor_reflect_ui::ReflectUiPlugin);
        }
        // Defensive, like every other reflect-touching system here: don't
        // assume some other plugin already turned reflection on for this
        // `App`. `init_resource` is a no-op if it's already present.
        app.init_resource::<AppTypeRegistry>();
        app.register_type::<Transform>();

        app.init_resource::<InspectorDirty>();
        // Same `Startup`-ordering hazard `bv_editor_scene_panel` documents:
        // `InspectorPanelSlot` is spawned by `bv_editor_ui`'s own `Startup`
        // system, and `Startup` systems from different plugins have no
        // guaranteed relative order without this.
        app.add_systems(Startup, spawn_inspector_chrome.after(bv_editor_ui::EditorShellSet));
        app.add_systems(Update, (detect_selection_change, rebuild_inspector_ui).chain());
    }
}

fn spawn_inspector_chrome(mut commands: Commands, slots: Query<Entity, With<InspectorPanelSlot>>) {
    let Ok(slot) = slots.single() else { return };
    commands.entity(slot).with_children(|panel| {
        panel.spawn((InspectorBody, Node { flex_direction: FlexDirection::Column, margin: UiRect::top(Val::Px(4.0)), ..Default::default() }));
    });
}

fn detect_selection_change(selection: Res<Selection>, mut dirty: ResMut<InspectorDirty>) {
    if selection.is_changed() {
        dirty.0 = true;
    }
}

/// The short (last path segment) display name for a reflected type, e.g.
/// `"bevy_transform::components::transform::Transform"` -> `"Transform"`.
fn short_type_name(type_path: &str) -> &str {
    type_path.rsplit("::").next().unwrap_or(type_path)
}

/// Exclusive on purpose: listing "whichever components a dynamically-typed
/// entity happens to have" and reflecting into each one needs `AppTypeRegistry`
/// + arbitrary `ComponentId`/`TypeId` lookups against the live `World`, which
/// has no safe non-exclusive `SystemParam` shape (the same reason
/// `bv_editor_reflect_ui`'s own edit-commit systems are exclusive).
fn rebuild_inspector_ui(world: &mut World) {
    if !world.resource::<InspectorDirty>().0 {
        return;
    }
    world.resource_mut::<InspectorDirty>().0 = false;

    let container = {
        let mut bodies = world.query_filtered::<Entity, With<InspectorBody>>();
        let Ok(container) = bodies.single(world) else { return };
        container
    };
    world.entity_mut(container).despawn_children();

    let Some(entity) = world.resource::<Selection>().primary() else {
        let mut commands = world.commands();
        commands.entity(container).with_children(|panel| {
            panel.spawn((EmptySelectionHint, Text::new("(nothing selected)"), TextColor(EMPTY_HINT_COLOR)));
        });
        world.flush();
        return;
    };

    let Ok(entity_ref) = world.get_entity(entity) else { return };
    let component_ids: Vec<ComponentId> = entity_ref.archetype().components().to_vec();

    let registry_arc = world.resource::<AppTypeRegistry>().0.clone();
    let registry = registry_arc.read();

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);

        for component_id in component_ids {
            let Some(type_id) = world.components().get_info(component_id).and_then(|info| info.type_id()) else { continue };
            let Some(registration) = registry.get(type_id) else { continue };
            let Some(reflect_component) = registration.data::<ReflectComponent>() else { continue };
            let Ok(entity_ref) = world.get_entity(entity) else { continue };
            let Some(value) = reflect_component.reflect(entity_ref) else { continue };

            let header = commands.spawn((Text::new(short_type_name(registration.type_info().type_path())), TextColor(SECTION_HEADER_COLOR))).id();
            commands.entity(container).add_child(header);

            let section = commands.spawn(Node { flex_direction: FlexDirection::Column, margin: UiRect::new(Val::Px(8.0), Val::Px(0.0), Val::Px(2.0), Val::Px(6.0)), ..Default::default() }).id();
            commands.entity(container).add_child(section);

            spawn_component_fields(&mut commands, section, entity, type_id, value.as_partial_reflect());
        }
    }
    queue.apply(world);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::name::Name;
    use bevy_input::keyboard::KeyCode;
    use bevy_math::Vec2;
    use bv_editor_core::EditorCorePlugin;
    use bv_editor_test_utils::{headless_app, simulate_click, simulate_key, step};

    /// A headless app with `EditorCorePlugin` (for `Selection`) and
    /// `InspectorPanelPlugin` already running against a bare
    /// `InspectorPanelSlot` — not the full Phase 1 shell.
    fn setup() -> App {
        let mut app = headless_app();
        app.add_plugins(EditorCorePlugin);
        app.world_mut().spawn(InspectorPanelSlot);
        app.add_plugins(InspectorPanelPlugin);
        step(&mut app, 1); // Startup chrome + first (dirty-by-default) rebuild
        app
    }

    fn field_ui_node(app: &mut App, path_suffix: &str) -> Entity {
        let world = app.world_mut();
        let mut fields = world.query::<(Entity, &bv_editor_reflect_ui::FieldHandle)>();
        fields
            .iter(world)
            .find(|(_, handle)| matches!(&handle.kind, bv_editor_reflect_ui::FieldKind::F32 { path } if path == path_suffix))
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("no F32 field with path {path_suffix:?}"))
    }

    #[test]
    fn selecting_an_entity_lists_its_transform_fields() {
        let mut app = setup();
        let cube = app.world_mut().spawn((Transform::from_xyz(1.0, 2.0, 3.0), Name::new("Cube"))).id();
        app.world_mut().resource_mut::<Selection>().select_only(cube);
        step(&mut app, 1);

        let world = app.world_mut();
        let mut fields = world.query::<&bv_editor_reflect_ui::FieldHandle>();
        let paths: Vec<String> = fields
            .iter(world)
            .filter_map(|h| match &h.kind {
                bv_editor_reflect_ui::FieldKind::F32 { path } => Some(path.clone()),
                _ => None,
            })
            .collect();
        for expected in ["translation.x", "translation.y", "translation.z", "scale.x", "scale.y", "scale.z"] {
            assert!(paths.contains(&expected.to_string()), "expected a field for {expected}, got {paths:?}");
        }
    }

    #[test]
    fn editing_translation_x_through_the_inspector_writes_the_real_component() {
        let mut app = setup();
        let cube = app.world_mut().spawn((Transform::from_xyz(1.0, 2.0, 3.0), Name::new("Cube"))).id();
        app.world_mut().resource_mut::<Selection>().select_only(cube);
        step(&mut app, 1);

        let field = field_ui_node(&mut app, "translation.x");
        app.world_mut().entity_mut(field).insert(Interaction::Pressed);
        simulate_click(&mut app, Vec2::ZERO);
        step(&mut app, 1);

        for key in [KeyCode::Digit7, KeyCode::Period, KeyCode::Digit5] {
            simulate_key(&mut app, key);
            step(&mut app, 1);
        }
        simulate_key(&mut app, KeyCode::Enter);
        step(&mut app, 1);

        assert_eq!(app.world().get::<Transform>(cube).unwrap().translation.x, 7.5);
    }

    #[test]
    fn clearing_selection_shows_the_empty_hint_and_no_fields() {
        let mut app = setup();
        let cube = app.world_mut().spawn((Transform::default(), Name::new("Cube"))).id();
        app.world_mut().resource_mut::<Selection>().select_only(cube);
        step(&mut app, 1);

        app.world_mut().resource_mut::<Selection>().clear();
        step(&mut app, 1);

        let world = app.world_mut();
        let mut fields = world.query::<&bv_editor_reflect_ui::FieldHandle>();
        assert_eq!(fields.iter(world).count(), 0);
        let mut hints = world.query::<&EmptySelectionHint>();
        assert_eq!(hints.iter(world).count(), 1);
    }
}
