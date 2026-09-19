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
use bv_editor_ui::{InspectorPanelSlot, ScrollbarAxis, ScrollbarThumb};

const SECTION_HEADER_COLOR: Color = Color::srgb(0.85, 0.85, 0.85);
const EMPTY_HINT_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);
const SCROLLBAR_TRACK_WIDTH_PX: f32 = 12.0;
const SCROLLBAR_TRACK_BACKGROUND: Color = Color::srgb(0.12, 0.12, 0.13);
const SCROLLBAR_THUMB_BACKGROUND: Color = Color::srgb(0.35, 0.35, 0.4);

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

        // Same reasoning as `bv_editor_scene_panel::ScenePanelPlugin`'s own
        // guard: this panel's tests spawn a bare `InspectorPanelSlot`
        // without the full `EditorUiPlugin`, so the Components panel
        // scrollbar (docs/UI_FEATURES.md F3) needs its driving systems here
        // too.
        if !app.is_plugin_added::<bv_editor_ui::ScrollbarPlugin>() {
            app.add_plugins(bv_editor_ui::ScrollbarPlugin);
        }

        app.init_resource::<InspectorDirty>();
        // Same `Startup`-ordering hazard `bv_editor_scene_panel` documents:
        // `InspectorPanelSlot` is spawned by `bv_editor_ui`'s own `Startup`
        // system, and `Startup` systems from different plugins have no
        // guaranteed relative order without this.
        app.add_systems(Startup, spawn_inspector_chrome.after(bv_editor_ui::EditorShellSet));
        app.add_systems(Update, (detect_selection_change, rebuild_inspector_ui).chain());
    }
}

/// Builds the Inspector's chrome: [`InspectorBody`] (clipped + scrollable on
/// both axes, docs/UI_FEATURES.md F3) inside an "L-shaped" scroll pane —
/// `content_row` (body | vertical track) sits above a horizontal track that
/// spans the full width. Content can overflow horizontally too (e.g. a
/// `Vec3`/`Color` field's inline boxes on a narrow panel), and a vertical-only
/// scrollbar left that simply spilling past the panel edge with no way to
/// reach it. See `bv_editor_scene_panel::spawn_scene_panel_chrome`'s doc
/// comment for why `flex_grow`/`min_height: Val::Px(0.0)` are needed for
/// `Overflow::scroll_y()` to actually have something to clip against —
/// `min_width: Val::Px(0.0)` is the same idea for the horizontal axis, so
/// `body` can shrink narrower than its content's natural (min-content) width.
fn spawn_inspector_chrome(mut commands: Commands, slots: Query<Entity, With<InspectorPanelSlot>>) {
    let Ok(slot) = slots.single() else { return };

    let scroll_area = commands
        .spawn((
            Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_height: Val::Px(0.0), margin: UiRect::top(Val::Px(4.0)), ..Default::default() },
            ChildOf(slot),
        ))
        .id();

    let content_row = commands
        .spawn((Node { flex_direction: FlexDirection::Row, flex_grow: 1.0, min_height: Val::Px(0.0), ..Default::default() }, ChildOf(scroll_area)))
        .id();

    let body = commands
        .spawn((
            InspectorBody,
            Interaction::default(),
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                overflow: Overflow::scroll(),
                ..Default::default()
            },
            ChildOf(content_row),
        ))
        .id();

    let v_track = commands
        .spawn((
            // `flex_shrink: 0.0` because a `Node`'s default is `1.0` — without
            // it, `content_row`'s flexbox would shrink this fixed-width track
            // to make room for `body`'s (potentially very wide, e.g. a long
            // debug-formatted field value) content, the same way it's
            // supposed to shrink `body` itself. A sidebar-style fixed-size
            // element must opt out of shrinking explicitly.
            Node { width: Val::Px(SCROLLBAR_TRACK_WIDTH_PX), height: Val::Percent(100.0), flex_shrink: 0.0, margin: UiRect::left(Val::Px(2.0)), ..Default::default() },
            BackgroundColor(SCROLLBAR_TRACK_BACKGROUND),
            ChildOf(content_row),
        ))
        .id();

    commands.spawn((
        ScrollbarThumb { target: body, axis: ScrollbarAxis::Vertical },
        Interaction::default(),
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), top: Val::Px(0.0), ..Default::default() },
        BackgroundColor(SCROLLBAR_THUMB_BACKGROUND),
        ChildOf(v_track),
    ));

    let h_track = commands
        .spawn((
            // Same `flex_shrink: 0.0` reasoning as `v_track` above, but on
            // `scroll_area`'s axis (a Column, so its main axis — the one
            // flex-shrink acts on — is height here, not width).
            Node { width: Val::Percent(100.0), height: Val::Px(SCROLLBAR_TRACK_WIDTH_PX), flex_shrink: 0.0, margin: UiRect::top(Val::Px(2.0)), ..Default::default() },
            BackgroundColor(SCROLLBAR_TRACK_BACKGROUND),
            ChildOf(scroll_area),
        ))
        .id();

    commands.spawn((
        ScrollbarThumb { target: body, axis: ScrollbarAxis::Horizontal },
        Interaction::default(),
        Node { position_type: PositionType::Absolute, height: Val::Percent(100.0), left: Val::Px(0.0), ..Default::default() },
        BackgroundColor(SCROLLBAR_THUMB_BACKGROUND),
        ChildOf(h_track),
    ));
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
    // docs/UI_FEATURES.md F3: unlike the Scene Tree's scroll position
    // (which should survive a tree rebuild), this one should *not* survive
    // a rebuild — every rebuild here is a selection change (the only thing
    // that sets `InspectorDirty`), and switching entities should always
    // show its first component, not wherever the previous entity's list
    // happened to be scrolled to.
    if let Some(mut scroll) = world.get_mut::<ScrollPosition>(container) {
        scroll.0.y = 0.0;
    }

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
    fn scrollbar_thumb_targets_the_inspector_body_and_wheel_scrolls_it() {
        use bevy_input::mouse::{MouseScrollUnit, MouseWheel};
        use bevy_input::touch::TouchPhase;
        use bevy_ui::ComputedNode;

        let mut app = setup();
        let body = {
            let world = app.world_mut();
            let mut bodies = world.query_filtered::<Entity, With<InspectorBody>>();
            bodies.single(world).expect("inspector body should exist")
        };
        {
            let world = app.world_mut();
            let mut thumbs = world.query::<&ScrollbarThumb>();
            assert!(thumbs.iter(world).any(|thumb| thumb.target == body), "a scrollbar thumb targeting the inspector body should exist");
        }

        // Same reasoning as `bv_editor_scene_panel`'s equivalent test:
        // headless tests never run `ui_layout_system`, so simulate a real
        // post-layout state directly on `ComputedNode`.
        app.world_mut().entity_mut(body).insert((
            Interaction::Hovered,
            ComputedNode { size: Vec2::new(200.0, 100.0), content_size: Vec2::new(200.0, 400.0), inverse_scale_factor: 1.0, ..Default::default() },
        ));
        step(&mut app, 1);

        app.world_mut().write_message(MouseWheel { unit: MouseScrollUnit::Pixel, x: 0.0, y: -40.0, window: Entity::PLACEHOLDER, phase: TouchPhase::Moved });
        step(&mut app, 1);

        assert_eq!(app.world().get::<bevy_ui::ScrollPosition>(body).unwrap().0.y, 40.0);
    }

    #[test]
    fn switching_selection_resets_the_scroll_position() {
        let mut app = setup();
        let cube = app.world_mut().spawn((Transform::default(), Name::new("Cube"))).id();
        let sphere = app.world_mut().spawn((Transform::default(), Name::new("Sphere"))).id();
        app.world_mut().resource_mut::<Selection>().select_only(cube);
        step(&mut app, 1);

        let body = {
            let world = app.world_mut();
            let mut bodies = world.query_filtered::<Entity, With<InspectorBody>>();
            bodies.single(world).expect("inspector body should exist")
        };
        app.world_mut().get_mut::<bevy_ui::ScrollPosition>(body).unwrap().0.y = 123.0;

        app.world_mut().resource_mut::<Selection>().select_only(sphere);
        step(&mut app, 1);

        assert_eq!(app.world().get::<bevy_ui::ScrollPosition>(body).unwrap().0.y, 0.0);
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
