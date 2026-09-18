//! `bv_editor_scene_panel` — built-in Scene Tree panel (docs/DESIGN.md
//! section 9, Phase 2).
//!
//! - Walks the world's real `Transform`/`ChildOf`/`Children`/`Name` hierarchy
//!   into rows under the Scene Tree slot ([`bv_editor_ui::ScenePanelSlot`]).
//!   `EditorOnly` entities (section 8.3) never appear.
//! - Clicking a row sets [`bv_editor_core::Selection`] and highlights it.
//! - "Add Entity" spawns a child of the current selection (or a root, if
//!   nothing's selected). "Delete Selected" and the Delete key (registered
//!   through `HotkeyRegistry`, section 8.2) remove the selected entity.
//! - Dragging a row onto another reparents it, via the generic
//!   drag-and-drop framework in `bv_editor_ui` (section 8.5).
//!
//! The design doc's original mock shows a right-click "Delete" context menu;
//! there is no context-menu framework yet (that's not scoped to any phase
//! so far), so Phase 2 exposes the same action as a toolbar button plus the
//! Delete hotkey instead. Revisit once a real context-menu framework exists.
//!
//! The tree is rebuilt only when something actually changes ([`SceneTreeDirty`]),
//! not every frame: rebuilding unconditionally would despawn and respawn
//! every row each frame, which would destroy the `Interaction` state that
//! click and drag detection both depend on before anything could ever read
//! it. [`detect_world_changes`] sets the flag when the *host game* changes
//! the hierarchy (spawns, despawns, reparents, renames) outside the editor's
//! own Add/Delete/drag actions, which already set it themselves.

mod tree;

pub use tree::{build_scene_tree, flatten_for_display, SceneEntityRow, SceneTreeNode};

use bevy_app::{App, Plugin, Update};
use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_input::ButtonInput;
use bevy_input::keyboard::KeyCode;
use bevy_input::mouse::MouseButton;
use bevy_text::TextColor;
use bevy_transform::components::Transform;
use bevy_ui::prelude::*;

use bv_editor_core::{EditorOnly, EditorState, HotkeyAppExt, HotkeyDescriptor, Selection};
use bv_editor_ui::{DragDropped, DragPayload, DragSource, DropTarget, ScenePanelSlot};

const ROW_INDENT_PX: f32 = 16.0;
const ROW_HEIGHT_PX: f32 = 22.0;
const ROW_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);
const ROW_SELECTED_BACKGROUND: Color = Color::srgb(0.24, 0.35, 0.55);
const TOOLBAR_BUTTON_BACKGROUND: Color = Color::srgb(0.22, 0.22, 0.25);
const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.85);

/// Forces a rebuild of the tree's UI rows on the next [`rebuild_scene_tree_ui`]
/// run. Starts `true` so the initial (possibly already-populated) hierarchy
/// gets displayed on the first frame.
#[derive(Resource)]
pub struct SceneTreeDirty(pub bool);

impl Default for SceneTreeDirty {
    fn default() -> Self {
        Self(true)
    }
}

/// Where tree rows get spawned as children — the container under
/// [`ScenePanelSlot`], separate from the toolbar row above it.
#[derive(Component)]
struct SceneTreeRowsContainer;

/// A single rendered row; `represents` is the *game* entity it stands for,
/// not to be confused with this UI node's own `Entity`.
#[derive(Component, Clone, Copy)]
struct SceneTreeRow {
    represents: Entity,
}

#[derive(Component)]
struct AddEntityButton;
#[derive(Component)]
struct DeleteSelectedButton;

/// Spawns the Scene Tree's own chrome (toolbar + rows container) into the
/// Phase 1 shell, walks the world into rows, drives selection/add/delete/
/// reparent, and rebuilds the displayed rows when something changes.
#[derive(Default)]
pub struct ScenePanelPlugin;

impl Plugin for ScenePanelPlugin {
    fn build(&self, app: &mut App) {
        // Depends on `bv_editor_ui`'s drag-and-drop framework being active
        // (for `handle_drag_reparent`'s `MessageReader<DragDropped>`), so it
        // ensures that itself rather than assuming `EditorUiPlugin` already
        // added it — this is what lets `ScenePanelPlugin` work on its own
        // (e.g. in its own tests) without the full shell.
        if !app.is_plugin_added::<bv_editor_ui::DragAndDropPlugin>() {
            app.add_plugins(bv_editor_ui::DragAndDropPlugin);
        }
        app.init_resource::<SceneTreeDirty>();
        app.register_hotkey(HotkeyDescriptor {
            id: "scene_panel.delete_entity",
            key: KeyCode::Delete,
            when: EditorState::Editing,
        });
        // `ScenePanelSlot` is spawned by `bv_editor_ui`'s own `Startup`
        // system; `Startup` systems from different plugins have no
        // guaranteed relative order without this, so this can otherwise run
        // (and silently no-op, finding no slot) before the slot exists.
        app.add_systems(bevy_app::Startup, spawn_scene_panel_chrome.after(bv_editor_ui::EditorShellSet));
        app.add_systems(
            Update,
            (
                detect_world_changes,
                handle_row_clicks,
                handle_add_entity_button,
                handle_delete_selected,
                handle_drag_reparent,
                rebuild_scene_tree_ui,
            )
                .chain()
                // `handle_drag_reparent` reads `DragDropped`, which
                // `bv_editor_ui::drag_and_drop_system` only writes this same
                // frame if it runs first; nothing else orders the two
                // relative to each other otherwise.
                .after(bv_editor_ui::drag_and_drop_system),
        );
    }
}

fn button(label: &'static str) -> impl Bundle {
    (
        Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), margin: UiRect::right(Val::Px(4.0)), ..Default::default() },
        BackgroundColor(TOOLBAR_BUTTON_BACKGROUND),
        Interaction::default(),
        children![(Text::new(label), TextColor(TEXT_COLOR))],
    )
}

fn spawn_scene_panel_chrome(mut commands: Commands, slots: Query<Entity, With<ScenePanelSlot>>) {
    let Ok(slot) = slots.single() else { return };

    commands.entity(slot).with_children(|panel| {
        panel
            .spawn(Node { flex_direction: FlexDirection::Row, margin: UiRect::top(Val::Px(4.0)), ..Default::default() })
            .with_children(|toolbar| {
                toolbar.spawn((AddEntityButton, button("+ Add Entity")));
                toolbar.spawn((DeleteSelectedButton, button("Delete")));
            });

        panel.spawn((
            SceneTreeRowsContainer,
            Node { flex_direction: FlexDirection::Column, margin: UiRect::top(Val::Px(4.0)), ..Default::default() },
        ));
    });
}

/// Marks the tree dirty when the *host game* changes the hierarchy — a
/// spawn, despawn, reparent, or rename that didn't come from this panel's
/// own Add/Delete/drag systems (those already set [`SceneTreeDirty`]
/// themselves). Cheap existence checks, not a rebuild, so this costs nothing
/// on the common frame where nothing changed.
///
/// Scoped to `With<Transform>` deliberately: `bevy_ui` rows/buttons/containers
/// all gain a fresh `ChildOf` every time they're (re)spawned too, and without
/// this filter their own churn would itself look like a hierarchy change —
/// every rebuild would mark itself dirty again on the very next frame,
/// perpetually despawning and respawning rows (and invalidating any entity a
/// consumer, e.g. a drag in progress, was holding onto).
fn detect_world_changes(
    changed: Query<Entity, (With<Transform>, Or<(Added<Transform>, Changed<ChildOf>, Changed<bevy_ecs::name::Name>, Added<EditorOnly>)>)>,
    mut removed_transforms: RemovedComponents<Transform>,
    mut dirty: ResMut<SceneTreeDirty>,
) {
    let removed_any = removed_transforms.read().next().is_some();
    if !changed.is_empty() || removed_any {
        dirty.0 = true;
    }
}

fn handle_row_clicks(mouse: Res<ButtonInput<MouseButton>>, rows: Query<(&Interaction, &SceneTreeRow)>, mut selection: ResMut<Selection>) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if let Some((_, row)) = rows.iter().find(|(interaction, _)| **interaction == Interaction::Pressed) {
        selection.select_only(row.represents);
    }
}

fn handle_add_entity_button(
    mouse: Res<ButtonInput<MouseButton>>,
    buttons: Query<&Interaction, With<AddEntityButton>>,
    selection: Res<Selection>,
    mut commands: Commands,
    mut dirty: ResMut<SceneTreeDirty>,
) {
    if !mouse.just_pressed(MouseButton::Left) || !buttons.iter().any(|interaction| *interaction == Interaction::Pressed) {
        return;
    }

    let mut new_entity = commands.spawn((Transform::default(), bevy_ecs::name::Name::new("Entity")));
    if let Some(parent) = selection.primary() {
        new_entity.insert(ChildOf(parent));
    }
    dirty.0 = true;
}

fn handle_delete_selected(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<&Interaction, With<DeleteSelectedButton>>,
    mut selection: ResMut<Selection>,
    mut commands: Commands,
    mut dirty: ResMut<SceneTreeDirty>,
) {
    let button_clicked = mouse.just_pressed(MouseButton::Left) && buttons.iter().any(|interaction| *interaction == Interaction::Pressed);
    let hotkey_pressed = keys.just_pressed(KeyCode::Delete);
    if !button_clicked && !hotkey_pressed {
        return;
    }
    let Some(entity) = selection.primary() else { return };

    commands.entity(entity).despawn();
    selection.clear();
    dirty.0 = true;
}

/// `dragged` becomes `target`'s child unless that would create a cycle
/// (dropping an entity onto itself or onto one of its own descendants).
fn is_ancestor_or_self(parents: &Query<&ChildOf>, ancestor_candidate: Entity, entity: Entity) -> bool {
    let mut current = entity;
    loop {
        if current == ancestor_candidate {
            return true;
        }
        match parents.get(current) {
            Ok(child_of) => current = child_of.parent(),
            Err(_) => return false,
        }
    }
}

fn handle_drag_reparent(
    mut drops: MessageReader<DragDropped>,
    rows: Query<&SceneTreeRow>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
    mut dirty: ResMut<SceneTreeDirty>,
) {
    for drop in drops.read() {
        let DragPayload::Entity(dragged) = drop.payload;
        let Ok(target_row) = rows.get(drop.target) else { continue };
        let target = target_row.represents;

        if is_ancestor_or_self(&parents, dragged, target) {
            continue;
        }

        commands.entity(dragged).insert(ChildOf(target));
        dirty.0 = true;
    }
}

fn rebuild_scene_tree_ui(
    mut commands: Commands,
    mut dirty: ResMut<SceneTreeDirty>,
    containers: Query<Entity, With<SceneTreeRowsContainer>>,
    existing_rows: Query<Entity, With<SceneTreeRow>>,
    scene_rows: Query<(Entity, Option<&bevy_ecs::name::Name>, Option<&ChildOf>, Has<EditorOnly>), (With<Transform>, Without<Node>)>,
    selection: Res<Selection>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    let Ok(container) = containers.single() else { return };

    for row in &existing_rows {
        commands.entity(row).despawn();
    }

    let flat_rows: Vec<SceneEntityRow> = scene_rows
        .iter()
        .map(|(entity, name, child_of, editor_only)| SceneEntityRow {
            entity,
            name: name.map(|n| n.as_str().to_string()),
            parent: child_of.map(ChildOf::parent),
            editor_only,
        })
        .collect();
    let tree = build_scene_tree(&flat_rows);
    let display_rows = flatten_for_display(&tree);

    commands.entity(container).with_children(|rows_container| {
        for (entity, label, depth) in display_rows {
            let background = if selection.contains(entity) { ROW_SELECTED_BACKGROUND } else { ROW_BACKGROUND };
            rows_container
                .spawn((
                    SceneTreeRow { represents: entity },
                    DragSource { payload: DragPayload::Entity(entity) },
                    DropTarget,
                    Interaction::default(),
                    Node {
                        height: Val::Px(ROW_HEIGHT_PX),
                        padding: UiRect::left(Val::Px(4.0 + depth as f32 * ROW_INDENT_PX)),
                        flex_direction: FlexDirection::Column,
                        ..Default::default()
                    },
                    BackgroundColor(background),
                ))
                .with_children(|row| {
                    row.spawn((Text::new(label), TextColor(TEXT_COLOR)));
                });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::name::Name;
    use bevy_math::Vec2;
    use bv_editor_core::EditorCorePlugin;
    use bv_editor_test_utils::{headless_app, simulate_click, simulate_key, simulate_release, step};
    use std::collections::HashSet;

    /// A headless app with `EditorCorePlugin` (for `Selection`/hotkeys) and
    /// `ScenePanelPlugin` already running against a bare `ScenePanelSlot` —
    /// not the full Phase 1 shell, since these tests only need the slot
    /// contract, not the surrounding layout.
    fn setup() -> App {
        let mut app = headless_app();
        app.add_plugins(EditorCorePlugin);
        app.world_mut().spawn(ScenePanelSlot);
        app.add_plugins(ScenePanelPlugin);
        step(&mut app, 1); // Startup chrome + first (dirty-by-default) rebuild
        app
    }

    fn represented_entities(app: &mut App) -> HashSet<Entity> {
        let world = app.world_mut();
        let mut rows = world.query::<&SceneTreeRow>();
        rows.iter(world).map(|row| row.represents).collect()
    }

    fn row_for(app: &mut App, target: Entity) -> Entity {
        let world = app.world_mut();
        let mut rows = world.query::<(Entity, &SceneTreeRow)>();
        rows.iter(world).find(|(_, row)| row.represents == target).map(|(entity, _)| entity).expect("row should exist for entity")
    }

    #[test]
    fn tree_reflects_hierarchy_and_hides_editor_only_subtree() {
        let mut app = setup();
        let grandparent = app.world_mut().spawn((Transform::default(), Name::new("Grandparent"))).id();
        let parent = app.world_mut().spawn((Transform::default(), Name::new("Parent"), ChildOf(grandparent))).id();
        let child = app.world_mut().spawn((Transform::default(), Name::new("Child"), ChildOf(parent))).id();
        let camera = app.world_mut().spawn((Transform::default(), Name::new("EditorCamera"), EditorOnly)).id();
        let gizmo_mesh = app.world_mut().spawn((Transform::default(), Name::new("GizmoMesh"), ChildOf(camera))).id();

        step(&mut app, 1);

        let represented = represented_entities(&mut app);
        assert!(represented.contains(&grandparent));
        assert!(represented.contains(&parent));
        assert!(represented.contains(&child));
        assert!(!represented.contains(&camera));
        assert!(!represented.contains(&gizmo_mesh));
        assert_eq!(represented.len(), 3);
    }

    #[test]
    fn clicking_a_row_sets_selection() {
        let mut app = setup();
        let cube = app.world_mut().spawn((Transform::default(), Name::new("Cube"))).id();
        step(&mut app, 1);

        let row = row_for(&mut app, cube);
        app.world_mut().entity_mut(row).insert(Interaction::Pressed);
        simulate_click(&mut app, Vec2::ZERO);
        step(&mut app, 1);

        assert_eq!(app.world().resource::<Selection>().primary(), Some(cube));
    }

    #[test]
    fn add_entity_button_spawns_a_child_of_the_current_selection() {
        let mut app = setup();
        let parent = app.world_mut().spawn((Transform::default(), Name::new("Parent"))).id();
        app.world_mut().resource_mut::<Selection>().select_only(parent);
        step(&mut app, 1);

        let button = {
            let world = app.world_mut();
            let mut buttons = world.query_filtered::<Entity, With<AddEntityButton>>();
            buttons.single(world).expect("Add Entity button should exist")
        };
        app.world_mut().entity_mut(button).insert(Interaction::Pressed);
        simulate_click(&mut app, Vec2::ZERO);
        step(&mut app, 1);

        let world = app.world_mut();
        let mut new_children = world.query_filtered::<&ChildOf, (With<Transform>, Without<Node>, Without<SceneTreeRow>)>();
        let children_of_parent: Vec<_> = new_children.iter(world).filter(|child_of| child_of.parent() == parent).collect();
        assert_eq!(children_of_parent.len(), 1, "exactly one new entity should have been spawned as a child of the selection");
    }

    #[test]
    fn add_entity_button_spawns_a_root_when_nothing_is_selected() {
        let mut app = setup();
        step(&mut app, 1);

        let button = {
            let world = app.world_mut();
            let mut buttons = world.query_filtered::<Entity, With<AddEntityButton>>();
            buttons.single(world).expect("Add Entity button should exist")
        };
        app.world_mut().entity_mut(button).insert(Interaction::Pressed);
        simulate_click(&mut app, Vec2::ZERO);
        step(&mut app, 1);

        assert_eq!(represented_entities(&mut app).len(), 1);
    }

    #[test]
    fn delete_hotkey_removes_the_selected_entity() {
        let mut app = setup();
        let cube = app.world_mut().spawn((Transform::default(), Name::new("Cube"))).id();
        app.world_mut().resource_mut::<Selection>().select_only(cube);
        step(&mut app, 1);

        simulate_key(&mut app, KeyCode::Delete);
        step(&mut app, 1);

        assert!(app.world().get_entity(cube).is_err(), "entity should have been despawned");
        assert!(app.world().resource::<Selection>().is_empty());
    }

    #[test]
    fn delete_button_also_removes_the_selected_entity() {
        let mut app = setup();
        let cube = app.world_mut().spawn((Transform::default(), Name::new("Cube"))).id();
        app.world_mut().resource_mut::<Selection>().select_only(cube);
        step(&mut app, 1);

        let button = {
            let world = app.world_mut();
            let mut buttons = world.query_filtered::<Entity, With<DeleteSelectedButton>>();
            buttons.single(world).expect("Delete button should exist")
        };
        app.world_mut().entity_mut(button).insert(Interaction::Pressed);
        simulate_click(&mut app, Vec2::ZERO);
        step(&mut app, 1);

        assert!(app.world().get_entity(cube).is_err());
    }

    #[test]
    fn dragging_a_row_onto_another_reparents_it() {
        let mut app = setup();
        let a = app.world_mut().spawn((Transform::default(), Name::new("A"))).id();
        let b = app.world_mut().spawn((Transform::default(), Name::new("B"))).id();
        step(&mut app, 1);

        let row_a = row_for(&mut app, a);
        let row_b = row_for(&mut app, b);

        app.world_mut().entity_mut(row_a).insert(Interaction::Pressed);
        simulate_click(&mut app, Vec2::ZERO);
        step(&mut app, 1); // bv_editor_ui's drag_and_drop_system picks up the press as the drag source

        // Simulate the cursor moving from row_a to row_b: a real picking
        // backend clears row_a's `Interaction` once the cursor is no longer
        // over it (the button stays logically held via `ButtonInput`, but
        // `Interaction` itself only reflects "is the cursor here right now").
        // Leaving row_a `Pressed` here would make both rows valid hover
        // targets and the drop would land on whichever the query happens to
        // iterate first.
        app.world_mut().entity_mut(row_a).insert(Interaction::None);
        app.world_mut().entity_mut(row_b).insert(Interaction::Hovered);
        step(&mut app, 1); // tracked as the hovered drop target

        simulate_release(&mut app);
        step(&mut app, 1); // DragDropped fires; scene_panel reparents `a` under `b`

        assert_eq!(app.world().get::<ChildOf>(a).map(ChildOf::parent), Some(b));
    }

    #[test]
    fn dropping_an_entity_onto_its_own_descendant_is_ignored() {
        let mut app = setup();
        let parent = app.world_mut().spawn((Transform::default(), Name::new("Parent"))).id();
        let child = app.world_mut().spawn((Transform::default(), Name::new("Child"), ChildOf(parent))).id();
        step(&mut app, 1);

        let row_parent = row_for(&mut app, parent);
        let row_child = row_for(&mut app, child);

        app.world_mut().entity_mut(row_parent).insert(Interaction::Pressed);
        simulate_click(&mut app, Vec2::ZERO);
        step(&mut app, 1);

        // See the comment in `dragging_a_row_onto_another_reparents_it`:
        // clear the source row's `Interaction` to realistically simulate the
        // cursor moving onto `row_child`, so the drop actually targets the
        // descendant this test means to exercise, not whichever row a query
        // happens to iterate first.
        app.world_mut().entity_mut(row_parent).insert(Interaction::None);
        app.world_mut().entity_mut(row_child).insert(Interaction::Hovered);
        step(&mut app, 1);

        simulate_release(&mut app);
        step(&mut app, 1);

        // `parent` must not become a child of its own descendant — the world
        // must not gain a hierarchy cycle.
        assert!(app.world().get::<ChildOf>(parent).is_none());
        assert_eq!(app.world().get::<ChildOf>(child).map(ChildOf::parent), Some(parent));
    }
}
