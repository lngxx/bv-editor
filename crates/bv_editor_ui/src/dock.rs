//! docs/UI_FEATURES.md F5: drag one panel onto another to merge them into a
//! tab group, or drag a tab to an edge to split it back out into its own
//! panel. This module is the tree data model + its pure mutations
//! ([`DockLayout`]/[`DockNode`]) — no ECS, no rendering — the same "pure
//! function first" split [`crate::splitter::resize_value`] and
//! [`crate::scrollbar::thumb_geometry`] already use. [`crate::shell`] wires
//! this into real `bevy_ui` nodes.
//!
//! **Scope of this pass:** only the four built-in side/bottom panels (Scene
//! Tree, Components, Project Files, Console) are dockable — the toolbar,
//! viewport, and status bar stay fixed, matching what docs/UI_FEATURES.md's
//! original request actually asked for ("side panel ต้องลาก-วาง"). Floating
//! windows and saving/loading the arrangement to `editor_layout.ron` are
//! still out of scope (the former for v1 generally, per docs/DESIGN.md
//! section 2; the latter tied to Phase 10/F4's own "persist across
//! sessions" gap, not yet started).
//!
//! **Invariant this module enforces:** a region (`left`/`right`/`bottom`)
//! can never become fully empty. Dragging a panel to a different region is
//! refused (a no-op) if it's the last panel left in its current region —
//! otherwise that region's whole box would have nothing to show and nothing
//! left to drop future panels onto (docs/UI_FEATURES.md doesn't specify
//! this case; refusing it sidesteps needing to collapse/hide/reveal a
//! region's box entirely, which is real added complexity for a case no one
//! asked for).

use bevy_ecs::prelude::*;
use bevy_input::mouse::MouseButton;
use bevy_input::ButtonInput;
use bevy_ui::Interaction;

use crate::dnd::{DragDropped, DragPayload};
use crate::splitter::SplitterAxis;

/// One of the four built-in panels this pass's docking covers. Not an
/// open-ended extension point yet — that needs `bv_editor_extension`'s
/// `PanelDescriptor` (Phase 7, still an empty crate), which this predates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelId {
    Scene,
    Inspector,
    Assets,
    Console,
}

/// Which of the shell's three dockable regions a panel currently lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockRegion {
    Left,
    Right,
    Bottom,
}

/// Which edge of a target panel a tab was dropped on, requesting a new
/// split there. Maps to a [`SplitterAxis`] plus which side the new leaf
/// lands on — see [`DockEdge::axis_and_side`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl DockEdge {
    /// Which dimension this edge splits along, and whether the *new* leaf
    /// (the dragged-in tab) becomes the split's `first` (resizable) child or
    /// its `second` (flex-filled) child — `first` renders left/top,
    /// `second` renders right/bottom, matching [`crate::splitter::Splitter`]'s
    /// own left-to-right/top-to-bottom convention.
    fn axis_and_new_leaf_is_first(self) -> (SplitterAxis, bool) {
        match self {
            DockEdge::Left => (SplitterAxis::Horizontal, true),
            DockEdge::Right => (SplitterAxis::Horizontal, false),
            DockEdge::Top => (SplitterAxis::Vertical, true),
            DockEdge::Bottom => (SplitterAxis::Vertical, false),
        }
    }
}

/// One node of a region's dock tree: either a tab group showing one of its
/// `tabs` at a time, or a resizable split into two subtrees.
#[derive(Debug, Clone, PartialEq)]
pub enum DockNode {
    Tabs { tabs: Vec<PanelId>, active: usize },
    Split { axis: SplitterAxis, first: Box<DockNode>, second: Box<DockNode>, first_size_px: f32 },
}

impl DockNode {
    /// A fresh region containing just `id`, the shape every region starts
    /// in before anything's been dragged.
    pub fn single(id: PanelId) -> Self {
        DockNode::Tabs { tabs: vec![id], active: 0 }
    }

    fn is_empty(&self) -> bool {
        match self {
            DockNode::Tabs { tabs, .. } => tabs.is_empty(),
            DockNode::Split { .. } => false,
        }
    }

    fn contains(&self, id: PanelId) -> bool {
        match self {
            DockNode::Tabs { tabs, .. } => tabs.contains(&id),
            DockNode::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }

    /// Total tab count across the whole subtree — used to guard "don't
    /// empty a region" before letting a cross-region move remove one.
    fn tab_count(&self) -> usize {
        match self {
            DockNode::Tabs { tabs, .. } => tabs.len(),
            DockNode::Split { first, second, .. } => first.tab_count() + second.tab_count(),
        }
    }

    /// Whether `id` is the currently-shown tab of whichever leaf contains
    /// it. `false` if `id` isn't in this subtree at all.
    fn is_active(&self, id: PanelId) -> bool {
        match self {
            DockNode::Tabs { tabs, active } => tabs.get(*active) == Some(&id),
            DockNode::Split { first, second, .. } => first.is_active(id) || second.is_active(id),
        }
    }

    /// Sets whichever leaf contains `id` as showing `id` (its active tab).
    /// A no-op (not found) if `id` isn't in this subtree.
    fn activate(&mut self, id: PanelId) -> bool {
        match self {
            DockNode::Tabs { tabs, active } => match tabs.iter().position(|&p| p == id) {
                Some(pos) => {
                    *active = pos;
                    true
                }
                None => false,
            },
            DockNode::Split { first, second, .. } => first.activate(id) || second.activate(id),
        }
    }

    /// Removes `id` from wherever it is in this subtree. A `Split` child
    /// that empties out collapses into its surviving sibling. Returns
    /// `(new_node, was_removed)` — consumes `self` by value rather than
    /// mutating in place because collapsing a `Split` means *replacing* it
    /// with one of its own children, which Rust's borrow checker won't
    /// allow through a `&mut` reference obtained by matching into the same
    /// node (the classic self-replacement problem; taking ownership
    /// sidesteps it entirely rather than reaching for `unsafe` or a second
    /// indirection).
    fn remove_owned(self, id: PanelId) -> (DockNode, bool) {
        match self {
            DockNode::Tabs { mut tabs, mut active } => match tabs.iter().position(|&p| p == id) {
                Some(pos) => {
                    tabs.remove(pos);
                    if tabs.is_empty() {
                        active = 0;
                    } else if active >= tabs.len() {
                        active = tabs.len() - 1;
                    }
                    (DockNode::Tabs { tabs, active }, true)
                }
                None => (DockNode::Tabs { tabs, active }, false),
            },
            DockNode::Split { axis, first, second, first_size_px } => {
                let (new_first, removed) = first.remove_owned(id);
                if removed {
                    return if new_first.is_empty() { (*second, true) } else { (DockNode::Split { axis, first: Box::new(new_first), second, first_size_px }, true) };
                }
                let (new_second, removed) = second.remove_owned(id);
                if removed {
                    return if new_second.is_empty() { (new_first, true) } else { (DockNode::Split { axis, first: Box::new(new_first), second: Box::new(new_second), first_size_px }, true) };
                }
                (DockNode::Split { axis, first: Box::new(new_first), second: Box::new(new_second), first_size_px }, false)
            }
        }
    }

    /// Appends `new_id` to the tabs of whichever leaf currently contains
    /// `existing`, and makes it the active one. `new_id` must not already be
    /// present anywhere in this subtree (callers remove it from its old
    /// home first — see [`DockLayout::merge_onto`]).
    fn insert_into_leaf_of(&mut self, existing: PanelId, new_id: PanelId) -> bool {
        match self {
            DockNode::Tabs { tabs, active } => {
                if tabs.contains(&existing) {
                    tabs.push(new_id);
                    *active = tabs.len() - 1;
                    true
                } else {
                    false
                }
            }
            DockNode::Split { first, second, .. } => first.insert_into_leaf_of(existing, new_id) || second.insert_into_leaf_of(existing, new_id),
        }
    }

    /// Splits the leaf containing `existing` into two: a fresh single-tab
    /// leaf holding `new_id` alongside the original leaf (with all its
    /// other tabs intact), ordered by `edge`. `new_id` must not already be
    /// present anywhere in this subtree, same caller obligation as
    /// [`Self::insert_into_leaf_of`].
    fn split_leaf_of(&mut self, existing: PanelId, edge: DockEdge, new_id: PanelId, first_size_px: f32) -> bool {
        match self {
            DockNode::Tabs { tabs, .. } if tabs.contains(&existing) => {
                let original = std::mem::replace(self, DockNode::Tabs { tabs: Vec::new(), active: 0 });
                let new_leaf = DockNode::single(new_id);
                let (axis, new_leaf_is_first) = edge.axis_and_new_leaf_is_first();
                let (first, second) = if new_leaf_is_first { (new_leaf, original) } else { (original, new_leaf) };
                *self = DockNode::Split { axis, first: Box::new(first), second: Box::new(second), first_size_px };
                true
            }
            DockNode::Tabs { .. } => false,
            DockNode::Split { first, second, .. } => first.split_leaf_of(existing, edge, new_id, first_size_px) || second.split_leaf_of(existing, edge, new_id, first_size_px),
        }
    }
}

/// Default resizable-side width/height (px) a freshly split-off leaf starts
/// at — same idea as the shell's own initial panel sizes, just a fixed
/// starting point the splitter then takes over from.
pub const DEFAULT_SPLIT_SIZE_PX: f32 = 220.0;

/// The whole shell's dockable arrangement: one tree per region. A
/// `Resource` so drag-drop and tab-click systems (docs/UI_FEATURES.md F5)
/// can mutate it directly; [`crate::shell`]'s rebuild system reads it to
/// re-render.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct DockLayout {
    pub left: DockNode,
    pub right: DockNode,
    pub bottom: DockNode,
}

impl Default for DockLayout {
    /// The shell's Phase 1 arrangement, unchanged until something drags a
    /// panel: Scene Tree alone on the left, Components alone on the right,
    /// Project Files/Console split evenly along the bottom row.
    fn default() -> Self {
        Self {
            left: DockNode::single(PanelId::Scene),
            right: DockNode::single(PanelId::Inspector),
            bottom: DockNode::Split { axis: SplitterAxis::Horizontal, first: Box::new(DockNode::single(PanelId::Assets)), second: Box::new(DockNode::single(PanelId::Console)), first_size_px: DEFAULT_SPLIT_SIZE_PX },
        }
    }
}

impl DockLayout {
    fn region_ref(&self, region: DockRegion) -> &DockNode {
        match region {
            DockRegion::Left => &self.left,
            DockRegion::Right => &self.right,
            DockRegion::Bottom => &self.bottom,
        }
    }

    fn region_mut(&mut self, region: DockRegion) -> &mut DockNode {
        match region {
            DockRegion::Left => &mut self.left,
            DockRegion::Right => &mut self.right,
            DockRegion::Bottom => &mut self.bottom,
        }
    }

    /// Which region currently holds `id`, if any (it always should — every
    /// `PanelId` lives in exactly one region at a time).
    pub fn region_of(&self, id: PanelId) -> Option<DockRegion> {
        [DockRegion::Left, DockRegion::Right, DockRegion::Bottom].into_iter().find(|&region| self.region_ref(region).contains(id))
    }

    /// Makes `id` the active (visible) tab of whichever leaf it's in —
    /// what clicking a tab strip button does. No-op if `id` isn't placed
    /// anywhere (shouldn't happen).
    pub fn activate(&mut self, id: PanelId) {
        let _ = self.left.activate(id) || self.right.activate(id) || self.bottom.activate(id);
    }

    /// Whether `id` is the currently-shown tab of its leaf — what a dock
    /// tab strip button's active/inactive styling reads.
    pub fn is_active(&self, id: PanelId) -> bool {
        self.left.is_active(id) || self.right.is_active(id) || self.bottom.is_active(id)
    }

    /// Refuses a cross-region move that would empty `dragged`'s current
    /// region entirely (this module's "a region is never fully empty"
    /// invariant — see the module doc comment). A same-region move never
    /// needs this: the panel count within that region doesn't change.
    fn would_empty_source_region(&self, dragged: PanelId, dst_region: DockRegion) -> bool {
        match self.region_of(dragged) {
            Some(src_region) if src_region != dst_region => self.region_ref(src_region).tab_count() <= 1,
            _ => false,
        }
    }

    /// Drag `dragged`'s tab onto `target_anchor`'s tab strip: merges
    /// `dragged` into `target_anchor`'s leaf as an additional tab, dropping
    /// wherever `dragged` used to be. Returns whether anything changed —
    /// `false` for dragging onto itself, an unknown panel, or a move that
    /// would empty `dragged`'s source region (see the module doc comment).
    pub fn merge_onto(&mut self, dragged: PanelId, target_anchor: PanelId) -> bool {
        if dragged == target_anchor {
            return false;
        }
        let Some(dst_region) = self.region_of(target_anchor) else { return false };
        if self.would_empty_source_region(dragged, dst_region) {
            return false;
        }
        let Some(src_region) = self.region_of(dragged) else { return false };

        let removed_tree = std::mem::replace(self.region_mut(src_region), DockNode::Tabs { tabs: Vec::new(), active: 0 });
        let (new_src_tree, removed) = removed_tree.remove_owned(dragged);
        *self.region_mut(src_region) = new_src_tree;
        if !removed {
            return false;
        }

        self.region_mut(dst_region).insert_into_leaf_of(target_anchor, dragged)
    }

    /// Drag `dragged`'s tab onto one of `target_anchor`'s four edges:
    /// splits `target_anchor`'s leaf, giving `dragged` a fresh leaf of its
    /// own on that side. Same refusal rules as [`Self::merge_onto`].
    pub fn split_onto(&mut self, dragged: PanelId, target_anchor: PanelId, edge: DockEdge) -> bool {
        if dragged == target_anchor {
            return false;
        }
        let Some(dst_region) = self.region_of(target_anchor) else { return false };
        if self.would_empty_source_region(dragged, dst_region) {
            return false;
        }
        let Some(src_region) = self.region_of(dragged) else { return false };

        let removed_tree = std::mem::replace(self.region_mut(src_region), DockNode::Tabs { tabs: Vec::new(), active: 0 });
        let (new_src_tree, removed) = removed_tree.remove_owned(dragged);
        *self.region_mut(src_region) = new_src_tree;
        if !removed {
            return false;
        }

        self.region_mut(dst_region).split_leaf_of(target_anchor, edge, dragged, DEFAULT_SPLIT_SIZE_PX)
    }
}

/// Where on a dock drop target a drag was released: the middle (merge into
/// a tab group) or one of the four edges (split off a new leaf there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockZone {
    Center,
    Edge(DockEdge),
}

/// Marks a UI node as a place a dragged panel tab can be dropped — paired
/// with the generic [`crate::DropTarget`] marker so
/// [`crate::drag_and_drop_system`] finds it. `anchor` only needs to be
/// *some* panel already in the leaf/region this target acts on:
/// [`DockLayout::merge_onto`]/[`DockLayout::split_onto`] use it purely to
/// locate the right spot in the tree, not as "the" panel being dropped on.
#[derive(Component, Clone, Copy, Debug)]
pub struct DockDropTarget {
    pub anchor: PanelId,
    pub zone: DockZone,
}

/// Marks a dock tab strip button for `panel` — clicking it makes `panel`
/// the active tab of its leaf ([`dock_tab_click_system`]). Also carries
/// [`crate::DragSource`] with `DragPayload::Panel(panel)` so the same
/// button can be dragged onto another [`DockDropTarget`].
#[derive(Component, Clone, Copy, Debug)]
pub struct DockTabButton(pub PanelId);

/// Set whenever [`DockLayout`] changes in a way that needs re-rendering — a
/// merge, a split, or just switching which tab is active.
/// [`crate::shell`]'s rebuild system clears it after acting.
#[derive(Resource, Default)]
pub struct DockLayoutDirty(pub bool);

/// Applies a completed drag onto a [`DockDropTarget`] to [`DockLayout`]:
/// merges into tabs ([`DockZone::Center`]) or splits off a new leaf
/// ([`DockZone::Edge`]). Ignores drops whose payload isn't
/// `DragPayload::Panel` (e.g. a Scene Tree row dragged over a dock target
/// by accident) or that land outside any [`DockDropTarget`] — the same
/// "wrong drop-target type" caution `bv_editor_scene_panel`'s own
/// `handle_drag_reparent` uses via its row-lookup query failing silently.
pub fn dock_drag_drop_system(mut dropped: MessageReader<DragDropped>, targets: Query<&DockDropTarget>, mut layout: ResMut<DockLayout>, mut dirty: ResMut<DockLayoutDirty>) {
    for drop in dropped.read() {
        let DragPayload::Panel(dragged) = drop.payload else { continue };
        let Ok(target) = targets.get(drop.target) else { continue };

        let changed = match target.zone {
            DockZone::Center => layout.merge_onto(dragged, target.anchor),
            DockZone::Edge(edge) => layout.split_onto(dragged, target.anchor, edge),
        };
        if changed {
            dirty.0 = true;
        }
    }
}

/// Clicking a dock tab strip button (press-and-release without becoming a
/// drag) makes it the active tab of its leaf. Gated on `just_released`,
/// *not* `just_pressed` (unlike `bv_editor_scene_panel`'s own toolbar
/// buttons) — this was a real bug: firing on press set
/// [`DockLayoutDirty`] on the exact frame a drag starts, and
/// [`crate::shell::rebuild_dock_ui_system`] reacting to that immediately
/// despawns and respawns the whole dock chrome, including the tab button
/// entity the pointer is still pressed on mid-gesture. That silently broke
/// every drag before it could reach a drop target — the tab strip button
/// the drag "started from" no longer existed by the next frame, and a
/// freshly-spawned replacement has no memory of ever being pressed. Firing
/// on release instead (using the same `Interaction::Hovered | Pressed`
/// check [`drag_and_drop_system`] itself uses to find the drop target, since
/// by release time `Interaction` may already have moved on from `Pressed`)
/// means the rebuild never happens until the drag gesture is already over,
/// which is also *when* [`dock_drag_drop_system`] resolves a drop — a plain
/// click (no movement) and a same-target drag both still land here as a
/// self-targeted merge (harmless no-op per [`DockLayout::merge_onto`]'s
/// `dragged == target_anchor` guard) with this system actually switching
/// the tab; a real drag onto a *different* target gets its final active-tab
/// state from [`DockLayout::merge_onto`]/[`DockLayout::split_onto`] instead,
/// since this system's `.chain()` order runs before
/// [`dock_drag_drop_system`] and its `activate` gets overridden by
/// whichever one of those actually fires.
pub fn dock_tab_click_system(mouse_buttons: Res<ButtonInput<MouseButton>>, buttons: Query<(&Interaction, &DockTabButton)>, mut layout: ResMut<DockLayout>, mut dirty: ResMut<DockLayoutDirty>) {
    if !mouse_buttons.just_released(MouseButton::Left) {
        return;
    }
    if let Some((_, button)) = buttons.iter().find(|(interaction, _)| matches!(interaction, Interaction::Hovered | Interaction::Pressed)) {
        layout.activate(button.0);
        dirty.0 = true;
    }
}

/// The four built-in panels' stable content-mount entities (the same
/// `*Slot` marker entities `bv_editor_scene_panel`/etc. already look up and
/// mount their content into — see docs/UI_FEATURES.md F5's shell.rs notes).
/// `bv_editor_ui::shell` populates this once at startup; [`crate::shell`]'s
/// dock rendering reparents whichever of these four sits in each rebuild
/// rather than destroying and recreating them, so panel plugins never need
/// to know docking exists.
#[derive(Resource, Clone, Copy, Debug)]
pub struct DockSlots {
    pub scene: Entity,
    pub inspector: Entity,
    pub assets: Entity,
    pub console: Entity,
}

impl DockSlots {
    pub fn get(&self, id: PanelId) -> Entity {
        match id {
            PanelId::Scene => self.scene,
            PanelId::Inspector => self.inspector,
            PanelId::Assets => self.assets,
            PanelId::Console => self.console,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PanelId::*;

    #[test]
    fn default_layout_matches_phase_1_shell() {
        let layout = DockLayout::default();
        assert_eq!(layout.region_of(Scene), Some(DockRegion::Left));
        assert_eq!(layout.region_of(Inspector), Some(DockRegion::Right));
        assert_eq!(layout.region_of(Assets), Some(DockRegion::Bottom));
        assert_eq!(layout.region_of(Console), Some(DockRegion::Bottom));
    }

    #[test]
    fn merge_moves_the_dragged_panel_into_the_targets_leaf() {
        let mut layout = DockLayout::default();
        assert!(layout.merge_onto(Console, Scene));

        assert_eq!(layout.region_of(Console), Some(DockRegion::Left));
        assert_eq!(layout.left, DockNode::Tabs { tabs: vec![Scene, Console], active: 1 });
        // Bottom lost Console but still has Assets, so it must not have
        // collapsed into some other shape — it's just the one leaf now.
        assert_eq!(layout.bottom, DockNode::single(Assets));
    }

    #[test]
    fn merge_onto_self_is_a_no_op() {
        let mut layout = DockLayout::default();
        assert!(!layout.merge_onto(Scene, Scene));
        assert_eq!(layout, DockLayout::default());
    }

    #[test]
    fn merge_refuses_to_empty_a_region() {
        // Left only has Scene; dragging it away to merge with Inspector
        // would leave the left region with nothing to show, which this
        // module's invariant forbids.
        let mut layout = DockLayout::default();
        assert!(!layout.merge_onto(Scene, Inspector));
        assert_eq!(layout, DockLayout::default());
    }

    #[test]
    fn merge_within_the_same_region_reorders_without_touching_the_would_empty_guard() {
        // Assets and Console already share the bottom region (across two
        // leaves) — merging them into one leaf must be allowed even though
        // it's "the last tab" of each leaf, because the *region* never
        // empties.
        let mut layout = DockLayout::default();
        assert!(layout.merge_onto(Console, Assets));
        assert_eq!(layout.bottom, DockNode::Tabs { tabs: vec![Assets, Console], active: 1 });
    }

    #[test]
    fn split_onto_an_edge_creates_a_sibling_leaf_on_that_side() {
        let mut layout = DockLayout::default();
        assert!(layout.merge_onto(Console, Scene)); // left is now Tabs[Scene, Console]
        assert!(layout.split_onto(Console, Scene, DockEdge::Bottom));

        match &layout.left {
            DockNode::Split { axis, first, second, .. } => {
                assert_eq!(*axis, SplitterAxis::Vertical);
                assert_eq!(**first, DockNode::single(Scene));
                assert_eq!(**second, DockNode::single(Console));
            }
            other => panic!("expected a Split, got {other:?}"),
        }
    }

    #[test]
    fn split_onto_left_edge_puts_the_new_leaf_first() {
        let mut layout = DockLayout::default();
        assert!(layout.split_onto(Console, Assets, DockEdge::Left));

        match &layout.bottom {
            DockNode::Split { axis, first, second, first_size_px } => {
                assert_eq!(*axis, SplitterAxis::Horizontal);
                assert_eq!(**first, DockNode::single(Console));
                assert_eq!(**second, DockNode::single(Assets));
                assert_eq!(*first_size_px, DEFAULT_SPLIT_SIZE_PX);
            }
            other => panic!("expected a Split, got {other:?}"),
        }
    }

    #[test]
    fn split_refuses_to_empty_a_region() {
        let mut layout = DockLayout::default();
        assert!(!layout.split_onto(Scene, Inspector, DockEdge::Left));
        assert_eq!(layout, DockLayout::default());
    }

    #[test]
    fn split_onto_self_is_a_no_op() {
        let mut layout = DockLayout::default();
        assert!(!layout.split_onto(Assets, Assets, DockEdge::Left));
        assert_eq!(layout, DockLayout::default());
    }

    #[test]
    fn activate_switches_which_tab_a_leaf_shows() {
        let mut layout = DockLayout::default();
        assert!(layout.merge_onto(Console, Assets)); // bottom is now Tabs[Assets, Console], active Console
        assert_eq!(layout.bottom, DockNode::Tabs { tabs: vec![Assets, Console], active: 1 });
        assert!(layout.is_active(Console));
        assert!(!layout.is_active(Assets));

        layout.activate(Assets);
        assert_eq!(layout.bottom, DockNode::Tabs { tabs: vec![Assets, Console], active: 0 });
        assert!(layout.is_active(Assets));
        assert!(!layout.is_active(Console));
    }

    #[test]
    fn removing_the_last_tab_from_a_split_child_collapses_it_into_its_sibling() {
        let mut layout = DockLayout::default();
        // Split Assets off Console's leaf, then drag Assets right back onto
        // Console: the now-empty split-off leaf must vanish, leaving a
        // plain Tabs leaf again rather than a Split with an empty child.
        assert!(layout.split_onto(Assets, Console, DockEdge::Left));
        assert!(matches!(layout.bottom, DockNode::Split { .. }));

        assert!(layout.merge_onto(Assets, Console));
        assert_eq!(layout.bottom, DockNode::Tabs { tabs: vec![Console, Assets], active: 1 });
    }

    #[test]
    fn every_panel_can_end_up_in_a_different_region_than_it_started_in() {
        // Left/right start with only one tab each, so neither can lose its
        // only occupant directly (the empty-region guard blocks that) —
        // this walks a sequence that first grows a region past one tab
        // before draining it, the only way a solo panel ever moves at all.
        let mut layout = DockLayout::default();

        assert!(layout.merge_onto(Console, Scene)); // left: [Scene, Console]; bottom: [Assets]
        assert!(layout.merge_onto(Scene, Inspector)); // left: [Console]; right: [Inspector, Scene]
        assert!(layout.merge_onto(Scene, Assets)); // right: [Inspector]; bottom: [Assets, Scene]
        assert!(layout.split_onto(Assets, Console, DockEdge::Top)); // bottom: [Scene]; left: Assets split above Console

        assert_eq!(layout.region_of(Assets), Some(DockRegion::Left));
        assert_eq!(layout.region_of(Console), Some(DockRegion::Left));
        assert_eq!(layout.region_of(Inspector), Some(DockRegion::Right));
        assert_eq!(layout.region_of(Scene), Some(DockRegion::Bottom));
        assert_eq!(layout.bottom, DockNode::single(Scene));
        match &layout.left {
            DockNode::Split { axis, first, second, .. } => {
                assert_eq!(*axis, SplitterAxis::Vertical);
                assert_eq!(**first, DockNode::single(Assets));
                assert_eq!(**second, DockNode::single(Console));
            }
            other => panic!("expected a Split, got {other:?}"),
        }
    }

    // ECS-level tests for `dock_drag_drop_system`/`dock_tab_click_system` —
    // bare `World` + `Schedule`, no `App`/`MinimalPlugins`, matching
    // `crate::dnd`'s own test style for the same reason: these systems only
    // touch a couple of resources/queries, not anything that needs a real
    // input pipeline.
    use bevy_ecs::message::Messages;

    #[test]
    fn drag_drop_center_zone_merges_the_dragged_panel() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(DockLayout::default());
        world.insert_resource(DockLayoutDirty::default());
        world.init_resource::<Messages<DragDropped>>();

        let target = world.spawn(DockDropTarget { anchor: Scene, zone: DockZone::Center }).id();
        world.resource_mut::<Messages<DragDropped>>().write(DragDropped { payload: DragPayload::Panel(Console), target });

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(dock_drag_drop_system);
        schedule.run(&mut world);

        assert_eq!(world.resource::<DockLayout>().region_of(Console), Some(DockRegion::Left));
        assert!(world.resource::<DockLayoutDirty>().0);
    }

    #[test]
    fn drag_drop_edge_zone_splits_off_the_dragged_panel() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(DockLayout::default());
        world.insert_resource(DockLayoutDirty::default());
        world.init_resource::<Messages<DragDropped>>();

        let target = world.spawn(DockDropTarget { anchor: Assets, zone: DockZone::Edge(DockEdge::Left) }).id();
        world.resource_mut::<Messages<DragDropped>>().write(DragDropped { payload: DragPayload::Panel(Console), target });

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(dock_drag_drop_system);
        schedule.run(&mut world);

        assert!(matches!(world.resource::<DockLayout>().bottom, DockNode::Split { .. }));
        assert!(world.resource::<DockLayoutDirty>().0);
    }

    #[test]
    fn drag_drop_ignores_a_non_panel_payload() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(DockLayout::default());
        world.insert_resource(DockLayoutDirty::default());
        world.init_resource::<Messages<DragDropped>>();

        let target = world.spawn(DockDropTarget { anchor: Scene, zone: DockZone::Center }).id();
        world.resource_mut::<Messages<DragDropped>>().write(DragDropped { payload: DragPayload::Entity(Entity::PLACEHOLDER), target });

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(dock_drag_drop_system);
        schedule.run(&mut world);

        assert_eq!(world.resource::<DockLayout>(), &DockLayout::default());
        assert!(!world.resource::<DockLayoutDirty>().0);
    }

    #[test]
    fn drag_drop_ignores_a_target_with_no_dock_drop_target() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(DockLayout::default());
        world.insert_resource(DockLayoutDirty::default());
        world.init_resource::<Messages<DragDropped>>();

        let target = world.spawn_empty().id();
        world.resource_mut::<Messages<DragDropped>>().write(DragDropped { payload: DragPayload::Panel(Console), target });

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(dock_drag_drop_system);
        schedule.run(&mut world);

        assert_eq!(world.resource::<DockLayout>(), &DockLayout::default());
        assert!(!world.resource::<DockLayoutDirty>().0);
    }

    #[test]
    fn clicking_a_tab_button_activates_it_only_on_release() {
        // Regression test: this used to fire on `just_pressed`, which set
        // `DockLayoutDirty` on the very frame a drag starts — triggering a
        // chrome rebuild mid-drag that silently broke every drag before it
        // could reach a drop target (see this system's own doc comment).
        let mut world = bevy_ecs::world::World::new();
        let mut layout = DockLayout::default();
        assert!(layout.merge_onto(Console, Assets)); // bottom: Tabs[Assets, Console], active Console
        world.insert_resource(layout);
        world.insert_resource(DockLayoutDirty::default());
        world.init_resource::<ButtonInput<MouseButton>>();

        world.spawn((Interaction::Pressed, DockTabButton(Assets)));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(dock_tab_click_system);

        world.resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        schedule.run(&mut world);
        assert_eq!(world.resource::<DockLayout>().bottom, DockNode::Tabs { tabs: vec![Assets, Console], active: 1 }, "pressing (not yet releasing) must not activate — that's the exact bug this test guards against");
        assert!(!world.resource::<DockLayoutDirty>().0);

        world.resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
        schedule.run(&mut world);

        assert_eq!(world.resource::<DockLayout>().bottom, DockNode::Tabs { tabs: vec![Assets, Console], active: 0 });
        assert!(world.resource::<DockLayoutDirty>().0);
    }
}
