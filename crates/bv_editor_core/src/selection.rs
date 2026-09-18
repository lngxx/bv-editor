//! The central `Selection` resource (docs/DESIGN.md section 6): two entry
//! points — clicking a mesh in the viewport (Phase 4) and clicking a row in
//! the Scene Tree (Phase 2) — write to this one resource, so every panel and
//! gizmo reading "what's selected" sees the same answer regardless of which
//! one set it.

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;

/// What's currently selected. Stored as a list (not `Option<Entity>`) so
/// multi-select can be added later (docs/DESIGN.md section 7.2's
/// `GizmoDrawContext::selection: &'w [Entity]` already assumes a slice)
/// without changing this type's shape again.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    entities: Vec<Entity>,
}

impl Selection {
    /// Replace the selection with exactly one entity.
    pub fn select_only(&mut self, entity: Entity) {
        self.entities.clear();
        self.entities.push(entity);
    }

    /// Deselect everything.
    pub fn clear(&mut self) {
        self.entities.clear();
    }

    /// The first selected entity, if any. For single-select interactions
    /// (Phase 2's tree click, Phase 4's viewport click) this is the whole
    /// selection.
    pub fn primary(&self) -> Option<Entity> {
        self.entities.first().copied()
    }

    /// Whether `entity` is part of the current selection.
    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.contains(&entity)
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Number of selected entities.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// The current selection as a slice, matching the shape gizmo code
    /// (Phase 4) will read it in.
    pub fn as_slice(&self) -> &[Entity] {
        &self.entities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    #[test]
    fn select_only_replaces_previous_selection() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();

        let mut selection = Selection::default();
        selection.select_only(a);
        assert!(selection.contains(a));
        assert_eq!(selection.primary(), Some(a));

        selection.select_only(b);
        assert!(!selection.contains(a));
        assert!(selection.contains(b));
        assert_eq!(selection.len(), 1);
    }

    #[test]
    fn clear_empties_the_selection() {
        let mut world = World::new();
        let a = world.spawn_empty().id();

        let mut selection = Selection::default();
        selection.select_only(a);
        selection.clear();

        assert!(selection.is_empty());
        assert_eq!(selection.primary(), None);
    }
}
