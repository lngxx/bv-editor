//! The shell: toolbar, viewport, status bar (all fixed — docs/DESIGN.md
//! section 9, Phase 1), and three dockable regions (left, right, bottom)
//! whose contents are driven by [`crate::dock::DockLayout`]
//! (docs/UI_FEATURES.md F5). Dragging a panel's tab strip button onto
//! another panel merges them into a tab group; dragging one onto an edge
//! splits it back out into its own resizable box — see [`crate::dock`] for
//! the tree data model itself, which this module only renders.
//!
//! - [`spawn_editor_shell`] builds the fixed chrome plus the *initial* dock
//!   render (`DockLayout::default()`), synchronously, so the entities in
//!   the returned [`EditorShellEntities`] are valid immediately.
//! - [`rebuild_dock_ui_system`] re-renders whenever [`crate::dock::DockLayoutDirty`]
//!   is set (a merge, a split, or a tab click) — see its own doc comment for
//!   how it avoids destroying the persistent `*Slot` content entities each
//!   time.
//! - The `*Slot` marker components are unchanged from Phase 1: panel
//!   plugins (`bv_editor_scene_panel` etc.) still just look one up and
//!   spawn their own content as its children, with no idea docking exists
//!   underneath them.

use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_text::TextColor;
use bevy_ui::prelude::*;

use crate::breakpoint::{bottom_panel_height_px, side_panel_width_px, LayoutBreakpoint};
use crate::dnd::{drag_and_drop_system, DragPayload, DragSource, DragState, DropTarget};
use crate::dock::{dock_drag_drop_system, dock_tab_click_system, DockDropTarget, DockEdge, DockLayout, DockLayoutDirty, DockNode, DockRegion, DockSlots, DockTabButton, DockZone, PanelId};
use crate::splitter::{Splitter, SplitterAxis};

/// Minimum/maximum width a side panel (Scene Tree / Components) can be dragged to.
const SIDE_PANEL_MIN_PX: f32 = 150.0;
const SIDE_PANEL_MAX_PX: f32 = 640.0;
/// Minimum/maximum height the bottom panel row can be dragged to.
const BOTTOM_ROW_MIN_PX: f32 = 80.0;
const BOTTOM_ROW_MAX_PX: f32 = 560.0;
/// Minimum/maximum size any *nested* dock split's resizable (first) side
/// can be dragged to — e.g. Project Files/Console's live splitter, or
/// whatever new split a drag-to-edge creates. One shared pair rather than
/// per-panel bounds: nothing about F5's request asks for per-panel tuning,
/// and a single constant is what F7's own `Node.min_*`/`max_*` floor needs
/// regardless of which panel ends up there.
const DOCK_SPLIT_MIN_PX: f32 = 120.0;
const DOCK_SPLIT_MAX_PX: f32 = 640.0;

const SPLITTER_THICKNESS_PX: f32 = 6.0;
const TOOLBAR_HEIGHT_PX: f32 = 40.0;
const STATUS_BAR_HEIGHT_PX: f32 = 24.0;
const TAB_STRIP_HEIGHT_PX: f32 = 26.0;
/// Thickness of the four invisible-until-hovered edge strips a leaf's
/// content mount grows, each a [`DockDropTarget`] for splitting a dragged
/// tab off to that side.
const DOCK_EDGE_ZONE_PX: f32 = 14.0;

/// docs/UI_FEATURES.md F7: the viewport's own floor, so it can never be
/// squeezed to nothing by the side panels/bottom row growing. Set as real
/// `Node.min_width`/`min_height` below rather than computed by a custom
/// system — `bevy_ui`'s own flexbox layout already enforces `min_*` as a
/// hard floor on every frame regardless of *why* a panel's size changed
/// (drag, window resize, a future saved-layout load, ...), which is exactly
/// "min/max size ที่ยึดอยู่จริง" without reimplementing constraint solving.
const VIEWPORT_MIN_WIDTH_PX: f32 = 200.0;
const VIEWPORT_MIN_HEIGHT_PX: f32 = 150.0;

const PANEL_BACKGROUND: Color = Color::srgb(0.16, 0.16, 0.18);
const VIEWPORT_BACKGROUND: Color = Color::srgb(0.10, 0.10, 0.11);
const SPLITTER_BACKGROUND: Color = Color::srgb(0.08, 0.08, 0.09);
const CHROME_BACKGROUND: Color = Color::srgb(0.13, 0.13, 0.15);
const ROOT_BACKGROUND: Color = Color::srgb(0.20, 0.20, 0.22);
const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.85);
const TAB_STRIP_BACKGROUND: Color = Color::srgb(0.12, 0.12, 0.13);
const TAB_ACTIVE_BACKGROUND: Color = Color::srgb(0.16, 0.16, 0.18);
const TAB_INACTIVE_BACKGROUND: Color = Color::srgb(0.10, 0.10, 0.11);
/// Shown on whichever [`DockDropTarget`] is currently under the pointer
/// while a panel tab is being dragged — the drop-zone indicator
/// docs/UI_FEATURES.md F5 asks for, simplified to "highlight the target
/// itself" rather than a separately-positioned overlay box (no extra
/// geometry to keep in sync, and the target's own shape already
/// communicates "merge into this whole tab strip" vs. "split off this
/// edge-thin strip").
const DOCK_DROP_HOVER_BACKGROUND: Color = Color::srgba(0.35, 0.55, 0.85, 0.55);
const DOCK_EDGE_ZONE_IDLE: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);

/// Marks the shell's single root node, spawned once by [`spawn_editor_shell`].
#[derive(Component)]
pub struct EditorShellRoot;

/// Where the top toolbar mounts its buttons/menu.
#[derive(Component)]
pub struct ToolbarSlot;
/// Where `bv_editor_scene_panel` mounts the Scene Tree (Phase 2). Stays
/// alive across dock rebuilds — see [`rebuild_dock_ui_system`] — so this
/// crate's own plugin never needs to know docking exists.
#[derive(Component)]
pub struct ScenePanelSlot;
/// Where `bv_editor_viewport` mounts the rendered 3D view (Phase 4). Not
/// dockable (docs/UI_FEATURES.md F5 only asked for side panels), so unlike
/// the other four `*Slot`s this one stays put in [`EditorShellEntities::viewport`].
#[derive(Component)]
pub struct ViewportSlot;
/// Where `bv_editor_inspector_panel` mounts Components/Materials/Resources (Phase 3).
#[derive(Component)]
pub struct InspectorPanelSlot;
/// Where `bv_editor_assets_panel` mounts the Project Files browser (Phase 5).
#[derive(Component)]
pub struct AssetsPanelSlot;
/// Where `bv_editor_console_panel` mounts the log view (still unscheduled — reserved from Phase 1 on).
#[derive(Component)]
pub struct ConsolePanelSlot;
/// Where the bottom status bar mounts its content.
#[derive(Component)]
pub struct StatusBarSlot;

/// Marks every entity [`render_dock_node`] spawns (split containers, tab
/// strips, tab buttons, edge drop-zones, content mounts) so
/// [`rebuild_dock_ui_system`] can find and despawn all of it on a rebuild —
/// deliberately *not* applied to the `*Slot` entities themselves, which are
/// reparented, never despawned.
#[derive(Component)]
struct DockChrome;

/// Marks one of the shell's three permanent dock region boxes (left/right/
/// bottom), which [`spawn_editor_shell`] creates once and
/// [`rebuild_dock_ui_system`] only ever re-renders the *children* of. Public
/// so a caller that only has an `App` (not the [`EditorShellEntities`]
/// `spawn_editor_shell` returned, e.g. after going through
/// [`crate::EditorUiPlugin`]'s own `Startup` system) can still find a
/// specific region's resizable box.
#[derive(Component)]
pub struct DockRegionRoot(pub DockRegion);

/// The leaf content container a docked panel's `*Slot` entity is
/// reparented into — see [`render_dock_leaf`].
#[derive(Component)]
struct DockLeafContentMount;

/// Handles to the key entities of a freshly spawned shell, returned by
/// [`spawn_editor_shell`] so callers (tests, and later phases before they
/// switch to querying by `*Slot` marker) don't have to re-walk the tree.
///
/// `scene_panel`/`inspector_panel` are the left/right dock region boxes —
/// stable across rebuilds, still carrying the same `min_width`/`max_width`
/// bounds F7 always gave them. There's no equivalent stable box for
/// Project Files/Console any more: which box (if any) currently holds
/// either of them can change as panels get dragged around, so a caller
/// after this point should look one up via [`DockTabButton`]/[`DockSlots`]
/// rather than a field here.
pub struct EditorShellEntities {
    pub root: Entity,
    pub toolbar: Entity,
    pub scene_panel: Entity,
    pub viewport: Entity,
    pub inspector_panel: Entity,
    pub status_bar: Entity,
}

/// Base flexbox settings shared by every panel box (column layout, a little padding).
fn panel_node() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Px(6.0)),
        ..Default::default()
    }
}

fn title_text(label: &'static str) -> impl Bundle {
    (Text::new(label), TextColor(TEXT_COLOR))
}

/// Human-readable tab strip label for a built-in panel — the same names the
/// Phase 1 shell used to show as each box's static title, now shown on its
/// tab button instead.
fn panel_label(id: PanelId) -> &'static str {
    match id {
        PanelId::Scene => "Scene Tree",
        PanelId::Inspector => "Components",
        PanelId::Assets => "Project Files",
        PanelId::Console => "Console",
    }
}

/// The persistent Node every `*Slot` entity carries — reapplied (not just
/// once at spawn) every time [`render_dock_leaf`] reparents it, since
/// that's also how a slot's visibility is toggled: `Display::Flex` if it's
/// the active tab of its leaf, `Display::None` otherwise. Safe to fully
/// overwrite because `shell.rs` is the sole author of a slot's own `Node`
/// — panel plugins only ever add *children* to it (see the module doc
/// comment), never touch this component.
fn slot_node(visible: bool) -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        padding: UiRect::all(Val::Px(6.0)),
        display: if visible { Display::Flex } else { Display::None },
        ..Default::default()
    }
}

/// Spawn a resizable splitter bar between a dock split's `first`/`second`
/// children. Always `invert: false`: [`render_dock_node`] only ever builds
/// `first`-then-splitter-then-`second` left-to-right/top-to-bottom, so the
/// splitter is always `target`'s *far* edge — the same convention (and the
/// same reason) [`spawn_vertical_splitter`]'s Scene Tree splitter uses.
fn spawn_dock_splitter_bar(commands: &mut Commands, parent: Entity, target: Entity, axis: SplitterAxis) {
    let node = match axis {
        SplitterAxis::Horizontal => Node { width: Val::Px(SPLITTER_THICKNESS_PX), height: Val::Percent(100.0), flex_shrink: 0.0, ..Default::default() },
        SplitterAxis::Vertical => Node { width: Val::Percent(100.0), height: Val::Px(SPLITTER_THICKNESS_PX), flex_shrink: 0.0, ..Default::default() },
    };
    commands.spawn((DockChrome, node, BackgroundColor(SPLITTER_BACKGROUND), Interaction::default(), Splitter { target, axis, min_px: DOCK_SPLIT_MIN_PX, max_px: DOCK_SPLIT_MAX_PX, invert: false }, ChildOf(parent)));
}

fn edge_zone_node(edge: DockEdge) -> Node {
    let base = Node { position_type: PositionType::Absolute, ..Default::default() };
    match edge {
        DockEdge::Left => Node { width: Val::Px(DOCK_EDGE_ZONE_PX), height: Val::Percent(100.0), left: Val::Px(0.0), top: Val::Px(0.0), ..base },
        DockEdge::Right => Node { width: Val::Px(DOCK_EDGE_ZONE_PX), height: Val::Percent(100.0), right: Val::Px(0.0), top: Val::Px(0.0), ..base },
        DockEdge::Top => Node { width: Val::Percent(100.0), height: Val::Px(DOCK_EDGE_ZONE_PX), left: Val::Px(0.0), top: Val::Px(0.0), ..base },
        DockEdge::Bottom => Node { width: Val::Percent(100.0), height: Val::Px(DOCK_EDGE_ZONE_PX), left: Val::Px(0.0), bottom: Val::Px(0.0), ..base },
    }
}

/// Renders one leaf (a tab group): the tab strip (one button per tab, each
/// a [`DragSource`]/[`DockDropTarget`] for merging), a content mount with
/// four edge [`DockDropTarget`]s for splitting, and reparents every tab's
/// `*Slot` entity into that mount — all of them, not just the active one,
/// each with [`slot_node`]'s `display` toggled so exactly one renders.
fn render_dock_leaf(commands: &mut Commands, parent: Entity, tabs: &[PanelId], active: usize, slots: &DockSlots) {
    let leaf = commands.spawn((DockChrome, Node { flex_direction: FlexDirection::Column, width: Val::Percent(100.0), height: Val::Percent(100.0), ..Default::default() }, ChildOf(parent))).id();

    let strip = commands
        .spawn((DockChrome, Node { flex_direction: FlexDirection::Row, width: Val::Percent(100.0), height: Val::Px(TAB_STRIP_HEIGHT_PX), flex_shrink: 0.0, ..Default::default() }, BackgroundColor(TAB_STRIP_BACKGROUND), ChildOf(leaf)))
        .id();
    for (i, &panel) in tabs.iter().enumerate() {
        let is_active = i == active;
        commands
            .spawn((
                DockChrome,
                DockTabButton(panel),
                DragSource { payload: DragPayload::Panel(panel) },
                DropTarget,
                DockDropTarget { anchor: panel, zone: DockZone::Center },
                Interaction::default(),
                Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), align_items: AlignItems::Center, ..Default::default() },
                BackgroundColor(if is_active { TAB_ACTIVE_BACKGROUND } else { TAB_INACTIVE_BACKGROUND }),
                ChildOf(strip),
            ))
            .with_children(|b| {
                b.spawn(title_text(panel_label(panel)));
            });
    }

    let mount = commands.spawn((DockChrome, DockLeafContentMount, Node { flex_grow: 1.0, width: Val::Percent(100.0), position_type: PositionType::Relative, ..Default::default() }, ChildOf(leaf))).id();

    let anchor = tabs[active];
    for edge in [DockEdge::Left, DockEdge::Right, DockEdge::Top, DockEdge::Bottom] {
        commands.spawn((DockChrome, DropTarget, DockDropTarget { anchor, zone: DockZone::Edge(edge) }, Interaction::default(), edge_zone_node(edge), BackgroundColor(DOCK_EDGE_ZONE_IDLE), ChildOf(mount)));
    }

    for &panel in tabs {
        commands.entity(slots.get(panel)).insert((slot_node(panel == anchor), ChildOf(mount)));
    }
}

/// Renders one [`DockNode`] under `parent`: either straight through to
/// [`render_dock_leaf`], or a resizable split into two boxes (`first`
/// fixed-size with a [`Splitter`], `second` flex-filled — the exact same
/// shape [`spawn_editor_shell`]'s own Scene Tree/Viewport/Components row
/// already used, just built recursively instead of by hand) each rendered
/// by recursing into this function again.
fn render_dock_node(commands: &mut Commands, parent: Entity, node: &DockNode, slots: &DockSlots) {
    match node {
        DockNode::Tabs { tabs, active } => render_dock_leaf(commands, parent, tabs, *active, slots),
        DockNode::Split { axis, first, second, first_size_px } => {
            let flex_direction = match axis {
                SplitterAxis::Horizontal => FlexDirection::Row,
                SplitterAxis::Vertical => FlexDirection::Column,
            };
            let container = commands.spawn((DockChrome, Node { flex_direction, width: Val::Percent(100.0), height: Val::Percent(100.0), ..Default::default() }, ChildOf(parent))).id();

            let first_node = match axis {
                SplitterAxis::Horizontal => Node { width: Val::Px(*first_size_px), height: Val::Percent(100.0), min_width: Val::Px(DOCK_SPLIT_MIN_PX), max_width: Val::Px(DOCK_SPLIT_MAX_PX), flex_shrink: 0.0, ..Default::default() },
                SplitterAxis::Vertical => Node { width: Val::Percent(100.0), height: Val::Px(*first_size_px), min_height: Val::Px(DOCK_SPLIT_MIN_PX), max_height: Val::Px(DOCK_SPLIT_MAX_PX), flex_shrink: 0.0, ..Default::default() },
            };
            let first_box = commands.spawn((DockChrome, first_node, ChildOf(container))).id();
            render_dock_node(commands, first_box, first, slots);

            spawn_dock_splitter_bar(commands, container, first_box, *axis);

            // No `Splitter` targets this side directly (it just grows to
            // fill whatever `first` doesn't take), but it still needs its
            // own floor — docs/UI_FEATURES.md F7 gap #3 was exactly this
            // shape (a panel with no `Splitter` pointed at it still needs a
            // real `Node.min_*`) and a nested split shouldn't reopen it.
            let second_node = match axis {
                SplitterAxis::Horizontal => Node { flex_grow: 1.0, height: Val::Percent(100.0), min_width: Val::Px(DOCK_SPLIT_MIN_PX), ..Default::default() },
                SplitterAxis::Vertical => Node { flex_grow: 1.0, width: Val::Percent(100.0), min_height: Val::Px(DOCK_SPLIT_MIN_PX), ..Default::default() },
            };
            let second_box = commands.spawn((DockChrome, second_node, ChildOf(container))).id();
            render_dock_node(commands, second_box, second, slots);
        }
    }
}

/// Spawn a vertical (width-resizing) splitter bar as a child of `parent`, targeting `target`.
/// `invert` must be `true` when the splitter is `target`'s *left* edge (the
/// Components panel, to the right of the viewport) rather than its right
/// edge (the Scene Tree, to the left of the viewport) — see [`Splitter::invert`].
fn spawn_vertical_splitter(commands: &mut Commands, parent: Entity, target: Entity, invert: bool) {
    commands.spawn((
        Node { width: Val::Px(SPLITTER_THICKNESS_PX), height: Val::Percent(100.0), ..Default::default() },
        BackgroundColor(SPLITTER_BACKGROUND),
        Interaction::default(),
        Splitter { target, axis: SplitterAxis::Horizontal, min_px: SIDE_PANEL_MIN_PX, max_px: SIDE_PANEL_MAX_PX, invert },
        ChildOf(parent),
    ));
}

/// Build the fixed shell (toolbar, viewport, status bar, the three dock
/// region boxes and their splitters) under a new root node, then render
/// [`DockLayout::default()`] into those boxes and return the key entities.
/// `breakpoint` only picks the *initial* side-panel/bottom-row sizes (see
/// [`crate::breakpoint`]) — the splitters take over from there.
pub fn spawn_editor_shell(commands: &mut Commands, breakpoint: LayoutBreakpoint) -> EditorShellEntities {
    let side_width = side_panel_width_px(breakpoint);
    let bottom_height = bottom_panel_height_px(breakpoint);

    let root = commands
        .spawn((
            EditorShellRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            BackgroundColor(ROOT_BACKGROUND),
        ))
        .id();

    // --- Toolbar ---
    let toolbar = commands
        .spawn((
            ToolbarSlot,
            Node { width: Val::Percent(100.0), height: Val::Px(TOOLBAR_HEIGHT_PX), ..panel_node() },
            BackgroundColor(CHROME_BACKGROUND),
            ChildOf(root),
        ))
        .with_children(|p| {
            p.spawn(title_text("Toolbar"));
        })
        .id();

    // --- Middle row: left dock region | splitter | Viewport | splitter | right dock region ---
    // `min_height` here is what actually protects the viewport's own floor
    // on the *vertical* axis (F7): it's a direct child of `root`'s column
    // layout, the same level the bottom row's splitter negotiates against,
    // so this is the constraint that stops the bottom row growing tall
    // enough to squeeze the viewport short — the viewport's own
    // `min_height` below matters for its direct row-mates, not this.
    let middle_row = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_grow: 1.0, flex_direction: FlexDirection::Row, min_height: Val::Px(VIEWPORT_MIN_HEIGHT_PX), ..Default::default() },
            ChildOf(root),
        ))
        .id();

    let left_region = commands
        .spawn((
            DockRegionRoot(DockRegion::Left),
            Node { width: Val::Px(side_width), height: Val::Percent(100.0), min_width: Val::Px(SIDE_PANEL_MIN_PX), max_width: Val::Px(SIDE_PANEL_MAX_PX), flex_direction: FlexDirection::Column, ..Default::default() },
            BackgroundColor(PANEL_BACKGROUND),
            ChildOf(middle_row),
        ))
        .id();

    spawn_vertical_splitter(commands, middle_row, left_region, false);

    let viewport = commands
        .spawn((
            ViewportSlot,
            Node { flex_grow: 1.0, height: Val::Percent(100.0), min_width: Val::Px(VIEWPORT_MIN_WIDTH_PX), min_height: Val::Px(VIEWPORT_MIN_HEIGHT_PX), ..panel_node() },
            BackgroundColor(VIEWPORT_BACKGROUND),
            ChildOf(middle_row),
        ))
        .with_children(|p| {
            p.spawn(title_text("Viewport"));
        })
        .id();

    // `right_region`'s id must exist before the splitter that targets it,
    // but it needs to render *after* the viewport in the row — reserve the
    // id now, insert its real components (and `ChildOf`, which fixes its
    // position in the parent's children order) further down.
    let right_region = commands.spawn_empty().id();
    spawn_vertical_splitter(commands, middle_row, right_region, true);
    commands.entity(right_region).insert((
        DockRegionRoot(DockRegion::Right),
        Node { width: Val::Px(side_width), height: Val::Percent(100.0), min_width: Val::Px(SIDE_PANEL_MIN_PX), max_width: Val::Px(SIDE_PANEL_MAX_PX), flex_direction: FlexDirection::Column, ..Default::default() },
        BackgroundColor(PANEL_BACKGROUND),
        ChildOf(middle_row),
    ));

    // --- Bottom splitter + row: the bottom dock region (Project Files | Console by default) ---
    // Same forward-reference trick: the splitter needs `bottom_region`'s id
    // before `bottom_region` is actually built.
    let bottom_region = commands.spawn_empty().id();
    commands.spawn((
        Node { width: Val::Percent(100.0), height: Val::Px(SPLITTER_THICKNESS_PX), ..Default::default() },
        BackgroundColor(SPLITTER_BACKGROUND),
        Interaction::default(),
        Splitter { target: bottom_region, axis: SplitterAxis::Vertical, min_px: BOTTOM_ROW_MIN_PX, max_px: BOTTOM_ROW_MAX_PX, invert: true },
        ChildOf(root),
    ));
    commands.entity(bottom_region).insert((
        DockRegionRoot(DockRegion::Bottom),
        Node { width: Val::Percent(100.0), height: Val::Px(bottom_height), flex_direction: FlexDirection::Row, min_height: Val::Px(BOTTOM_ROW_MIN_PX), max_height: Val::Px(BOTTOM_ROW_MAX_PX), ..Default::default() },
        ChildOf(root),
    ));

    // --- Status bar ---
    let status_bar = commands
        .spawn((
            StatusBarSlot,
            Node { width: Val::Percent(100.0), height: Val::Px(STATUS_BAR_HEIGHT_PX), ..panel_node() },
            BackgroundColor(CHROME_BACKGROUND),
            ChildOf(root),
        ))
        .with_children(|p| {
            p.spawn(title_text("Ready"));
        })
        .id();

    // --- The four dockable panels' persistent content slots, plus the
    // initial dock render that places them (docs/UI_FEATURES.md F5) ---
    let slots = DockSlots { scene: commands.spawn(ScenePanelSlot).id(), inspector: commands.spawn(InspectorPanelSlot).id(), assets: commands.spawn(AssetsPanelSlot).id(), console: commands.spawn(ConsolePanelSlot).id() };
    let layout = DockLayout::default();
    render_dock_node(commands, left_region, &layout.left, &slots);
    render_dock_node(commands, right_region, &layout.right, &slots);
    render_dock_node(commands, bottom_region, &layout.bottom, &slots);
    commands.insert_resource(slots);
    commands.insert_resource(layout);
    commands.insert_resource(DockLayoutDirty::default());

    EditorShellEntities { root, toolbar, scene_panel: left_region, viewport, inspector_panel: right_region, status_bar }
}

/// Re-renders the three dock regions whenever [`DockLayoutDirty`] is set —
/// after a merge, a split, or a plain tab click ([`dock_drag_drop_system`]/
/// [`dock_tab_click_system`], both `Update`-scheduled ahead of this one).
///
/// The tricky part isn't the rendering (that's just [`render_dock_node`]
/// again) — it's that the four `*Slot` entities currently live *somewhere*
/// inside the old chrome this system is about to despawn, and `despawn` is
/// recursive over the hierarchy: despawning old chrome without detaching
/// the slots first would take their real content (the whole Scene Tree,
/// Components, ...) down with it. So every rebuild: detach all four slots
/// (`remove::<ChildOf>`, making them parentless but very much alive) *before*
/// despawning anything, then despawn every [`DockChrome`] entity (safe now
/// that no slot is a descendant of one), then render fresh and let
/// [`render_dock_leaf`] reparent the slots back into their new homes.
fn rebuild_dock_ui_system(mut commands: Commands, mut dirty: ResMut<DockLayoutDirty>, layout: Res<DockLayout>, slots: Res<DockSlots>, region_roots: Query<(Entity, &DockRegionRoot)>, chrome: Query<Entity, With<DockChrome>>) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    for slot in [slots.scene, slots.inspector, slots.assets, slots.console] {
        commands.entity(slot).remove::<ChildOf>();
    }
    for entity in &chrome {
        commands.entity(entity).despawn();
    }

    for (root, DockRegionRoot(region)) in &region_roots {
        let node = match region {
            DockRegion::Left => &layout.left,
            DockRegion::Right => &layout.right,
            DockRegion::Bottom => &layout.bottom,
        };
        render_dock_node(&mut commands, root, node, &slots);
    }
}

/// Highlights whichever [`DockDropTarget`] the pointer is over while
/// dragging a panel tab (docs/UI_FEATURES.md F5's drop-zone indicator —
/// see [`DOCK_DROP_HOVER_BACKGROUND`]'s own doc comment for why this is a
/// plain highlight rather than a separate overlay), and otherwise keeps
/// each target showing its resting color: a tab button's active/inactive
/// tint, or transparent for an edge zone. Runs every frame unconditionally
/// (like [`crate::splitter::splitter_cursor_system`]) rather than behind
/// [`DockLayoutDirty`] — hover state changes every frame a drag is in
/// progress, independent of any actual layout change.
pub fn dock_drop_target_highlight_system(drag: Res<DragState>, layout: Res<DockLayout>, mut targets: Query<(Entity, Option<&DockTabButton>, &mut BackgroundColor), With<DockDropTarget>>) {
    let dragging_a_panel = matches!(drag.payload, Some(DragPayload::Panel(_)));
    for (entity, tab_button, mut background) in &mut targets {
        let hovered = dragging_a_panel && drag.hovered_target == Some(entity);
        background.0 = if hovered {
            DOCK_DROP_HOVER_BACKGROUND
        } else {
            match tab_button {
                Some(DockTabButton(panel)) if layout.is_active(*panel) => TAB_ACTIVE_BACKGROUND,
                Some(_) => TAB_INACTIVE_BACKGROUND,
                None => DOCK_EDGE_ZONE_IDLE,
            }
        };
    }
}

/// Registers the systems that drive docking (docs/UI_FEATURES.md F5):
/// tab clicks and drag-drop both mutate [`DockLayout`] and set
/// [`DockLayoutDirty`]; [`rebuild_dock_ui_system`] re-renders when it's
/// set; the highlight system runs unconditionally. [`crate::EditorUiPlugin`]
/// calls this — pulled out to its own function only so `lib.rs`'s
/// `build` doesn't have to spell out the whole ordering chain inline.
pub(crate) fn add_dock_systems(app: &mut bevy_app::App) {
    use bevy_app::Update;
    app.add_systems(Update, (dock_tab_click_system, dock_drag_drop_system, rebuild_dock_ui_system).chain().after(drag_and_drop_system));
    app.add_systems(Update, dock_drop_target_highlight_system);
}
