//! The Phase 1 shell: a fixed layout of empty, titled panel boxes plus the
//! splitters between them (docs/DESIGN.md section 9, Phase 1). No tabs, no
//! drag-to-dock yet — those are Phase 10. Content lands panel-by-panel in
//! later phases; this module only builds the boxes and exposes where that
//! content should mount (the `*Slot` marker components below).

use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_text::TextColor;
use bevy_ui::prelude::*;

use crate::breakpoint::{bottom_panel_height_px, side_panel_width_px, LayoutBreakpoint};
use crate::splitter::{Splitter, SplitterAxis};

/// Minimum/maximum width a side panel (Scene Tree / Components) can be dragged to.
const SIDE_PANEL_MIN_PX: f32 = 150.0;
const SIDE_PANEL_MAX_PX: f32 = 640.0;
/// Minimum/maximum height the bottom panel row can be dragged to.
const BOTTOM_ROW_MIN_PX: f32 = 80.0;
const BOTTOM_ROW_MAX_PX: f32 = 560.0;

const SPLITTER_THICKNESS_PX: f32 = 6.0;
const TOOLBAR_HEIGHT_PX: f32 = 40.0;
const STATUS_BAR_HEIGHT_PX: f32 = 24.0;

/// docs/UI_FEATURES.md F7: the viewport's own floor, so it can never be
/// squeezed to nothing by the side panels/bottom row growing. Set as real
/// `Node.min_width`/`min_height` below rather than computed by a custom
/// system — `bevy_ui`'s own flexbox layout already enforces `min_*` as a
/// hard floor on every frame regardless of *why* a panel's size changed
/// (drag, window resize, a future saved-layout load, ...), which is exactly
/// "min/max size ที่ยึดอยู่จริง" without reimplementing constraint solving.
const VIEWPORT_MIN_WIDTH_PX: f32 = 200.0;
const VIEWPORT_MIN_HEIGHT_PX: f32 = 150.0;
/// Same idea for Project Files/Console (F7's third gap): there's no
/// interactive splitter between them yet (just the static divider bar
/// below), but a floor is worth having regardless so they can't be crushed
/// to nothing if the bottom row itself ends up short.
const BOTTOM_PANEL_MIN_WIDTH_PX: f32 = 120.0;

const PANEL_BACKGROUND: Color = Color::srgb(0.16, 0.16, 0.18);
const VIEWPORT_BACKGROUND: Color = Color::srgb(0.10, 0.10, 0.11);
const SPLITTER_BACKGROUND: Color = Color::srgb(0.08, 0.08, 0.09);
const CHROME_BACKGROUND: Color = Color::srgb(0.13, 0.13, 0.15);
const ROOT_BACKGROUND: Color = Color::srgb(0.20, 0.20, 0.22);
const TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.85);

/// Marks the shell's single root node, spawned once by [`spawn_editor_shell`].
#[derive(Component)]
pub struct EditorShellRoot;

/// Where the top toolbar mounts its buttons/menu.
#[derive(Component)]
pub struct ToolbarSlot;
/// Where `bv_editor_scene_panel` mounts the Scene Tree (Phase 2).
#[derive(Component)]
pub struct ScenePanelSlot;
/// Where `bv_editor_viewport` mounts the rendered 3D view (Phase 4).
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

/// Handles to the key entities of a freshly spawned shell, returned by
/// [`spawn_editor_shell`] so callers (tests, and later phases before they
/// switch to querying by `*Slot` marker) don't have to re-walk the tree.
pub struct EditorShellEntities {
    pub root: Entity,
    pub toolbar: Entity,
    pub scene_panel: Entity,
    pub viewport: Entity,
    pub inspector_panel: Entity,
    pub assets_panel: Entity,
    pub console_panel: Entity,
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

/// Build the fixed Phase 1 shell under a new root node and return the key
/// entities. `breakpoint` only picks the *initial* side-panel/bottom-row
/// sizes (see [`crate::breakpoint`]) — the splitters take over from there.
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

    // --- Middle row: Scene Tree | splitter | Viewport | splitter | Components ---
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

    let scene_panel = commands
        .spawn((
            ScenePanelSlot,
            Node { width: Val::Px(side_width), height: Val::Percent(100.0), min_width: Val::Px(SIDE_PANEL_MIN_PX), max_width: Val::Px(SIDE_PANEL_MAX_PX), ..panel_node() },
            BackgroundColor(PANEL_BACKGROUND),
            ChildOf(middle_row),
        ))
        .with_children(|p| {
            p.spawn(title_text("Scene Tree"));
        })
        .id();

    spawn_vertical_splitter(commands, middle_row, scene_panel, false);

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

    // `inspector_panel`'s id must exist before the splitter that targets it,
    // but it needs to render *after* the viewport in the row — reserve the id
    // now, insert its real components (and `ChildOf`, which fixes its
    // position in the parent's children order) further down.
    let inspector_panel = commands.spawn_empty().id();
    spawn_vertical_splitter(commands, middle_row, inspector_panel, true);
    commands
        .entity(inspector_panel)
        .insert((
            InspectorPanelSlot,
            Node { width: Val::Px(side_width), height: Val::Percent(100.0), min_width: Val::Px(SIDE_PANEL_MIN_PX), max_width: Val::Px(SIDE_PANEL_MAX_PX), ..panel_node() },
            BackgroundColor(PANEL_BACKGROUND),
            ChildOf(middle_row),
        ))
        .with_children(|p| {
            p.spawn(title_text("Components"));
        });

    // --- Bottom splitter + row: Project Files | Console ---
    // Same forward-reference trick: the splitter needs `bottom_row`'s id
    // before `bottom_row` is actually built.
    let bottom_row = commands.spawn_empty().id();
    commands.spawn((
        Node { width: Val::Percent(100.0), height: Val::Px(SPLITTER_THICKNESS_PX), ..Default::default() },
        BackgroundColor(SPLITTER_BACKGROUND),
        Interaction::default(),
        Splitter { target: bottom_row, axis: SplitterAxis::Vertical, min_px: BOTTOM_ROW_MIN_PX, max_px: BOTTOM_ROW_MAX_PX, invert: true },
        ChildOf(root),
    ));
    commands.entity(bottom_row).insert((
        Node { width: Val::Percent(100.0), height: Val::Px(bottom_height), flex_direction: FlexDirection::Row, min_height: Val::Px(BOTTOM_ROW_MIN_PX), max_height: Val::Px(BOTTOM_ROW_MAX_PX), ..Default::default() },
        ChildOf(root),
    ));

    let assets_panel = commands
        .spawn((
            AssetsPanelSlot,
            Node { flex_grow: 2.0, height: Val::Percent(100.0), min_width: Val::Px(BOTTOM_PANEL_MIN_WIDTH_PX), ..panel_node() },
            BackgroundColor(PANEL_BACKGROUND),
            ChildOf(bottom_row),
        ))
        .with_children(|p| {
            p.spawn(title_text("Project Files"));
        })
        .id();

    commands.spawn((
        Node { width: Val::Px(SPLITTER_THICKNESS_PX), height: Val::Percent(100.0), ..Default::default() },
        BackgroundColor(SPLITTER_BACKGROUND),
        ChildOf(bottom_row),
    ));

    let console_panel = commands
        .spawn((
            ConsolePanelSlot,
            Node { flex_grow: 1.0, height: Val::Percent(100.0), min_width: Val::Px(BOTTOM_PANEL_MIN_WIDTH_PX), ..panel_node() },
            BackgroundColor(PANEL_BACKGROUND),
            ChildOf(bottom_row),
        ))
        .with_children(|p| {
            p.spawn(title_text("Console"));
        })
        .id();

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

    EditorShellEntities {
        root,
        toolbar,
        scene_panel,
        viewport,
        inspector_panel,
        assets_panel,
        console_panel,
        status_bar,
    }
}
