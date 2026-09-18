//! Pure tree-building logic, decoupled from both `bevy_ecs` queries and
//! `bevy_ui` rendering, so "does the tree match the real hierarchy" (the
//! Phase 2 test bullet in docs/DESIGN.md section 9) is a plain data
//! comparison rather than something that needs a running `App`.
//!
//! `lib.rs`'s `rebuild_scene_tree_ui` turns the live world into a flat
//! `Vec<SceneEntityRow>` via an ordinary query; [`build_scene_tree`] turns
//! that flat list into the nested [`SceneTreeNode`] tree the UI renders and
//! tests assert against.

use std::collections::{HashMap, HashSet};

use bevy_ecs::entity::Entity;

/// One flat row of scene data, as read straight off a `Transform`-bearing
/// entity: its name (if any), its parent (if any), and whether it's marked
/// `EditorOnly` (docs/DESIGN.md section 8.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneEntityRow {
    pub entity: Entity,
    pub name: Option<String>,
    pub parent: Option<Entity>,
    pub editor_only: bool,
}

/// One row of the Scene Tree the panel renders, and its descendants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneTreeNode {
    pub entity: Entity,
    pub label: String,
    pub children: Vec<SceneTreeNode>,
}

fn label_for(row: &SceneEntityRow) -> String {
    match &row.name {
        Some(name) => name.clone(),
        None => format!("Entity ({})", row.entity),
    }
}

/// Build the nested tree from a flat list of rows, preserving the order
/// `rows` was given in (both for roots and for children under the same
/// parent) — not `Entity`'s own `Ord`, which reflects internal allocator
/// bit layout, not spawn or hierarchy order. A real query's iteration order
/// isn't a hard guarantee either, but in practice tracks spawn order closely
/// enough for a Scene Tree; revisit with `Children`'s authored order
/// directly if that ever proves not good enough.
///
/// `EditorOnly` entities, and everything parented under one (directly or
/// transitively), never appear (section 8.3 — this is not optional). An
/// entity whose parent isn't itself a scene row at all (e.g. accidentally
/// parented under `bevy_ui` chrome) is promoted to a root instead of
/// silently disappearing.
pub fn build_scene_tree(rows: &[SceneEntityRow]) -> Vec<SceneTreeNode> {
    let row_entities: HashSet<Entity> = rows.iter().map(|row| row.entity).collect();
    let editor_only: HashSet<Entity> = rows.iter().filter(|row| row.editor_only).map(|row| row.entity).collect();

    let mut children_of: HashMap<Entity, Vec<&SceneEntityRow>> = HashMap::new();
    let mut roots: Vec<&SceneEntityRow> = Vec::new();

    for row in rows {
        if editor_only.contains(&row.entity) {
            continue;
        }
        match row.parent {
            Some(parent) if editor_only.contains(&parent) => continue, // hidden with its EditorOnly parent
            Some(parent) if row_entities.contains(&parent) => children_of.entry(parent).or_default().push(row),
            _ => roots.push(row), // no parent, or parent isn't a scene entity at all
        }
    }

    fn build(row: &SceneEntityRow, children_of: &HashMap<Entity, Vec<&SceneEntityRow>>) -> SceneTreeNode {
        let children: Vec<SceneTreeNode> = children_of
            .get(&row.entity)
            .into_iter()
            .flatten()
            .map(|child| build(child, children_of))
            .collect();
        SceneTreeNode { entity: row.entity, label: label_for(row), children }
    }

    roots.iter().map(|row| build(row, &children_of)).collect()
}

/// Flatten the tree into a `(entity, label, depth)` list in display order.
/// The panel renders rows as a flat list of UI nodes indented by `depth`
/// (rather than nesting them to match the tree, which would need to name
/// `bevy_ui`'s recursive child-spawner type) — this is what makes that
/// possible while keeping the actual nesting logic in [`build_scene_tree`]
/// testable on its own.
pub fn flatten_for_display(nodes: &[SceneTreeNode]) -> Vec<(Entity, String, u32)> {
    fn walk(nodes: &[SceneTreeNode], depth: u32, out: &mut Vec<(Entity, String, u32)>) {
        for node in nodes {
            out.push((node.entity, node.label.clone(), depth));
            walk(&node.children, depth + 1, out);
        }
    }

    let mut out = Vec::new();
    walk(nodes, 0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entity: Entity, name: Option<&str>, parent: Option<Entity>) -> SceneEntityRow {
        SceneEntityRow { entity, name: name.map(str::to_string), parent, editor_only: false }
    }

    #[test]
    fn flat_roots_with_no_hierarchy() {
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        let rows = vec![row(a, Some("A"), None), row(b, Some("B"), None)];

        let tree = build_scene_tree(&rows);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].entity, a);
        assert_eq!(tree[0].label, "A");
        assert_eq!(tree[1].entity, b);
    }

    #[test]
    fn three_level_hierarchy_matches_real_parenting() {
        let grandparent = Entity::from_raw_u32(1).unwrap();
        let parent = Entity::from_raw_u32(2).unwrap();
        let child = Entity::from_raw_u32(3).unwrap();
        let rows = vec![
            row(grandparent, Some("Grandparent"), None),
            row(parent, Some("Parent"), Some(grandparent)),
            row(child, Some("Child"), Some(parent)),
        ];

        let tree = build_scene_tree(&rows);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].label, "Grandparent");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].label, "Parent");
        assert_eq!(tree[0].children[0].children.len(), 1);
        assert_eq!(tree[0].children[0].children[0].label, "Child");
    }

    #[test]
    fn unnamed_entity_gets_a_fallback_label() {
        let entity = Entity::from_raw_u32(42).unwrap();
        let tree = build_scene_tree(&[row(entity, None, None)]);
        assert_eq!(tree[0].label, format!("Entity ({entity})"));
    }

    #[test]
    fn editor_only_entity_and_its_subtree_are_hidden() {
        let camera = Entity::from_raw_u32(1).unwrap();
        let gizmo_mesh = Entity::from_raw_u32(2).unwrap();
        let real_entity = Entity::from_raw_u32(3).unwrap();
        let rows = vec![
            SceneEntityRow { entity: camera, name: Some("EditorCamera".into()), parent: None, editor_only: true },
            row(gizmo_mesh, Some("GizmoMesh"), Some(camera)),
            row(real_entity, Some("Cube"), None),
        ];

        let tree = build_scene_tree(&rows);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].label, "Cube");
    }

    #[test]
    fn entity_parented_outside_the_scene_is_promoted_to_root() {
        // e.g. accidentally parented under a `bevy_ui` node, which never
        // shows up as a `SceneEntityRow` at all.
        let outside_parent = Entity::from_raw_u32(99).unwrap();
        let entity = Entity::from_raw_u32(1).unwrap();
        let tree = build_scene_tree(&[row(entity, Some("Orphan"), Some(outside_parent))]);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].label, "Orphan");
    }

    #[test]
    fn flatten_preserves_depth_first_order_and_depth() {
        let parent = Entity::from_raw_u32(1).unwrap();
        let child = Entity::from_raw_u32(2).unwrap();
        let sibling = Entity::from_raw_u32(3).unwrap();
        let tree = build_scene_tree(&[row(parent, Some("Parent"), None), row(child, Some("Child"), Some(parent)), row(sibling, Some("Sibling"), None)]);

        let flat = flatten_for_display(&tree);
        assert_eq!(
            flat,
            vec![(parent, "Parent".to_string(), 0), (child, "Child".to_string(), 1), (sibling, "Sibling".to_string(), 0)]
        );
    }

    #[test]
    fn siblings_preserve_input_order_not_entity_id_order() {
        // `Entity`'s own `Ord` reflects allocator bit layout, not spawn or
        // hierarchy order, so a higher entity id spawned/listed *first* must
        // still come first — this is the opposite of sorting by entity id.
        let parent = Entity::from_raw_u32(1).unwrap();
        let higher_id_but_listed_first = Entity::from_raw_u32(3).unwrap();
        let lower_id_but_listed_second = Entity::from_raw_u32(2).unwrap();
        let rows = vec![
            row(parent, Some("Parent"), None),
            row(higher_id_but_listed_first, Some("First"), Some(parent)),
            row(lower_id_but_listed_second, Some("Second"), Some(parent)),
        ];

        let tree = build_scene_tree(&rows);
        assert_eq!(tree[0].children[0].label, "First");
        assert_eq!(tree[0].children[1].label, "Second");
    }
}
