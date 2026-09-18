//! `bv_editor_reflect_ui` — generic field editor from `bevy_reflect` +
//! `AppTypeRegistry` (docs/DESIGN.md section 3, section 9 Phase 3).
//!
//! Given a reflected component value (`&dyn PartialReflect`, typically
//! obtained via `ReflectComponent::reflect`), [`spawn_component_fields`]
//! walks its (one level of) named struct fields and spawns one editable row
//! per field it understands: `f32`, `bool`, `String`, `Vec3` (as three `f32`
//! sub-fields), and `Color` (as four `f32` channel sub-fields, round-tripped
//! through `Srgba`). Anything else — `Handle<T>`, `Entity`, nested structs —
//! renders as a read-only `{:?}` placeholder rather than being skipped, so
//! the field is still visible; per docs/DESIGN.md Phase 3 scope, a real
//! asset-reference picker for `Handle<T>` is Phase 5's job.
//!
//! Editing itself is click-to-focus, then type-to-replace: clicking a leaf
//! (other than a `bool`, which toggles immediately) clears [`FieldEditState`]'s
//! buffer and starts capturing key presses; Enter commits the buffer back
//! into the world through the same `ReflectComponent`/`GetPath` route the
//! initial display came from, Escape cancels. There is no existing
//! text-input widget anywhere in bv-editor to build on (`bevy_ui_widgets`/
//! `bevy_feathers` aren't wired up yet), so this hand-rolls the minimum
//! needed rather than pulling those in for one text field — consistent with
//! this codebase's existing pattern of hand-rolling small frameworks
//! (docking, drag-and-drop) where `bevy_ui` has no built-in.

use std::any::TypeId;

use bevy_app::{App, Plugin, Update};
use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_ecs::reflect::{AppTypeRegistry, ReflectComponent};
use bevy_input::keyboard::KeyCode;
use bevy_input::mouse::MouseButton;
use bevy_input::ButtonInput;
use bevy_math::Vec3;
use bevy_reflect::prelude::*;
use bevy_reflect::{PartialReflect, ReflectRef};
use bevy_text::TextColor;
use bevy_ui::prelude::*;

const FIELD_LABEL_COLOR: Color = Color::srgb(0.6, 0.6, 0.65);
const FIELD_VALUE_BACKGROUND: Color = Color::srgb(0.22, 0.22, 0.25);
const FIELD_EDITING_BACKGROUND: Color = Color::srgb(0.30, 0.45, 0.30);
const FIELD_TEXT_COLOR: Color = Color::srgb(0.85, 0.85, 0.85);
const FIELD_READONLY_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);
const TRANSPARENT: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);

/// One RGBA channel of a `Color` field, edited as an independent `f32`
/// sub-field the way `Vec3`'s `x`/`y`/`z` are, since `Color` is an enum and
/// has no dotted reflect path of its own to a single channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorChannel {
    R,
    G,
    B,
    A,
}

/// What a leaf field editor node edits, and how to parse/write its value
/// back. `path` is a `bevy_reflect::GetPath` dotted path rooted at the
/// component value (e.g. `"translation.x"`).
#[derive(Clone, Debug, PartialEq)]
pub enum FieldKind {
    F32 { path: String },
    Bool { path: String },
    Str { path: String },
    ColorChannel { field: String, channel: ColorChannel },
    /// Not editable — clicking it does nothing. Used for field types this
    /// crate doesn't have a dedicated editor for yet.
    ReadOnly,
}

/// Tags a spawned UI node as the editable leaf for one field of one
/// component on one (game) entity. `entity`/`type_id` identify which
/// component to reach back into on commit; `kind` says how.
#[derive(Component, Clone, Debug)]
pub struct FieldHandle {
    pub entity: Entity,
    pub type_id: TypeId,
    pub kind: FieldKind,
}

/// Which field's UI node (if any) is currently capturing keystrokes, and the
/// in-progress text buffer for it. Starts empty on click rather than
/// pre-filled with the current value: the user always types a full
/// replacement, never edits in place.
#[derive(Resource, Default)]
pub struct FieldEditState {
    pub target: Option<Entity>,
    pub buffer: String,
}

/// Registers [`FieldEditState`] and the click/keyboard systems that drive
/// editing. Both are plain `Update` systems operating on whichever entity
/// [`FieldHandle`]-tagged nodes exist in the world — nothing here is specific
/// to the Inspector panel, so any panel spawning fields via
/// [`spawn_component_fields`] gets editing for free by having this plugin
/// (transitively) present.
#[derive(Default)]
pub struct ReflectUiPlugin;

impl Plugin for ReflectUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FieldEditState>();
        app.add_systems(Update, (handle_field_click, handle_field_keyboard).chain());
    }
}

fn format_f32(value: f32) -> String {
    format!("{value:.3}")
}

/// Spawn one row per named struct field of `value` as children of
/// `container`. No-ops (spawns nothing) if `value` isn't a reflect `Struct`
/// — Phase 3's scope only covers plain component structs like `Transform`,
/// not tuple structs/enums/collections.
pub fn spawn_component_fields(commands: &mut Commands, container: Entity, entity: Entity, type_id: TypeId, value: &dyn PartialReflect) {
    let ReflectRef::Struct(s) = value.reflect_ref() else {
        return;
    };
    for i in 0..s.field_len() {
        let (Some(name), Some(field)) = (s.name_at(i), s.field_at(i)) else { continue };
        spawn_field_row(commands, container, entity, type_id, name, field);
    }
}

fn leaf_bundle(display: String, kind: FieldKind, entity: Entity, type_id: TypeId) -> impl Bundle {
    let editable = !matches!(kind, FieldKind::ReadOnly);
    (
        FieldHandle { entity, type_id, kind },
        Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), margin: UiRect::right(Val::Px(6.0)), ..Default::default() },
        BackgroundColor(if editable { FIELD_VALUE_BACKGROUND } else { TRANSPARENT }),
        Interaction::default(),
        Text::new(display),
        TextColor(if editable { FIELD_TEXT_COLOR } else { FIELD_READONLY_COLOR }),
    )
}

fn spawn_field_row(commands: &mut Commands, container: Entity, entity: Entity, type_id: TypeId, field_name: &str, field: &dyn PartialReflect) {
    commands.entity(container).with_children(|parent| {
        parent
            .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, margin: UiRect::vertical(Val::Px(1.0)), ..Default::default() })
            .with_children(|row| {
                row.spawn((Text::new(format!("{field_name}: ")), TextColor(FIELD_LABEL_COLOR)));

                if let Some(v) = field.try_downcast_ref::<f32>() {
                    row.spawn(leaf_bundle(format_f32(*v), FieldKind::F32 { path: field_name.to_string() }, entity, type_id));
                } else if let Some(v) = field.try_downcast_ref::<bool>() {
                    row.spawn(leaf_bundle(v.to_string(), FieldKind::Bool { path: field_name.to_string() }, entity, type_id));
                } else if let Some(v) = field.try_downcast_ref::<String>() {
                    row.spawn(leaf_bundle(v.clone(), FieldKind::Str { path: field_name.to_string() }, entity, type_id));
                } else if let Some(v) = field.try_downcast_ref::<Vec3>() {
                    for (axis, axis_value) in [("x", v.x), ("y", v.y), ("z", v.z)] {
                        row.spawn((Text::new(format!("{axis}=")), TextColor(FIELD_LABEL_COLOR)));
                        row.spawn(leaf_bundle(format_f32(axis_value), FieldKind::F32 { path: format!("{field_name}.{axis}") }, entity, type_id));
                    }
                } else if let Some(v) = field.try_downcast_ref::<Color>() {
                    let srgba = v.to_srgba();
                    for (label, channel, channel_value) in
                        [("r", ColorChannel::R, srgba.red), ("g", ColorChannel::G, srgba.green), ("b", ColorChannel::B, srgba.blue), ("a", ColorChannel::A, srgba.alpha)]
                    {
                        row.spawn((Text::new(format!("{label}=")), TextColor(FIELD_LABEL_COLOR)));
                        row.spawn(leaf_bundle(format_f32(channel_value), FieldKind::ColorChannel { field: field_name.to_string(), channel }, entity, type_id));
                    }
                } else {
                    row.spawn(leaf_bundle(format!("{field:?}"), FieldKind::ReadOnly, entity, type_id));
                }
            });
    });
}

/// Look up `type_id`'s `ReflectComponent` in `AppTypeRegistry`, reach into
/// `entity`'s component through it, resolve `path` inside that component's
/// reflected value, and hand the resulting leaf field to `f`. `None` if the
/// component isn't reflect-registered, the entity doesn't have it, or the
/// path doesn't resolve — any of which just means "don't commit", not a panic.
fn with_reflect_field_mut<R>(world: &mut World, entity: Entity, type_id: TypeId, path: &str, f: impl FnOnce(&mut dyn PartialReflect) -> R) -> Option<R> {
    let registry_arc = world.resource::<AppTypeRegistry>().0.clone();
    let reflect_component = {
        let registry = registry_arc.read();
        registry.get_type_data::<ReflectComponent>(type_id)?.clone()
    };
    let mut entity_mut = world.get_entity_mut(entity).ok()?;
    let mut reflected = reflect_component.reflect_mut(&mut entity_mut)?;
    let field = reflected.reflect_path_mut(path).ok()?;
    Some(f(field))
}

/// Replaces a whole `Color` field with one channel changed, since `Color`'s
/// reflect path doesn't expose `r`/`g`/`b`/`a` directly (it's an enum of
/// color spaces). Reads the current value out as `Srgba`, overwrites the one
/// channel, and writes the whole `Color` back via `PartialReflect::apply`.
fn commit_color_channel(world: &mut World, entity: Entity, type_id: TypeId, field_name: &str, channel: ColorChannel, value: f32) -> bool {
    with_reflect_field_mut(world, entity, type_id, field_name, |field| {
        let Some(color) = field.try_downcast_ref::<Color>() else { return false };
        let mut srgba = color.to_srgba();
        match channel {
            ColorChannel::R => srgba.red = value,
            ColorChannel::G => srgba.green = value,
            ColorChannel::B => srgba.blue = value,
            ColorChannel::A => srgba.alpha = value,
        }
        let new_color = Color::srgba(srgba.red, srgba.green, srgba.blue, srgba.alpha);
        field.apply(&new_color);
        true
    })
    .unwrap_or(false)
}

/// Maps a physical key to the character it types into a field buffer.
/// Digits/`-`/`.` always map (needed for `f32`/color-channel fields);
/// letters/space only map for `text_allowed` (`String` fields) — there's no
/// shift/case handling (see the module doc: no real text-input widget
/// exists yet), so typed strings come out lowercase.
fn char_for_key(key: KeyCode, text_allowed: bool) -> Option<char> {
    match key {
        KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
        KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
        KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
        KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
        KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
        KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
        KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
        KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
        KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
        KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
        KeyCode::Minus | KeyCode::NumpadSubtract => Some('-'),
        KeyCode::Period | KeyCode::NumpadDecimal => Some('.'),
        _ if text_allowed => match key {
            KeyCode::Space => Some(' '),
            KeyCode::KeyA => Some('a'),
            KeyCode::KeyB => Some('b'),
            KeyCode::KeyC => Some('c'),
            KeyCode::KeyD => Some('d'),
            KeyCode::KeyE => Some('e'),
            KeyCode::KeyF => Some('f'),
            KeyCode::KeyG => Some('g'),
            KeyCode::KeyH => Some('h'),
            KeyCode::KeyI => Some('i'),
            KeyCode::KeyJ => Some('j'),
            KeyCode::KeyK => Some('k'),
            KeyCode::KeyL => Some('l'),
            KeyCode::KeyM => Some('m'),
            KeyCode::KeyN => Some('n'),
            KeyCode::KeyO => Some('o'),
            KeyCode::KeyP => Some('p'),
            KeyCode::KeyQ => Some('q'),
            KeyCode::KeyR => Some('r'),
            KeyCode::KeyS => Some('s'),
            KeyCode::KeyT => Some('t'),
            KeyCode::KeyU => Some('u'),
            KeyCode::KeyV => Some('v'),
            KeyCode::KeyW => Some('w'),
            KeyCode::KeyX => Some('x'),
            KeyCode::KeyY => Some('y'),
            KeyCode::KeyZ => Some('z'),
            _ => None,
        },
        _ => None,
    }
}

/// Click a `bool` leaf to toggle it immediately (no buffer/commit step);
/// click any other editable leaf to start capturing keystrokes for it in
/// [`FieldEditState`]. `ReadOnly` leaves do nothing.
fn handle_field_click(world: &mut World) {
    if !world.resource::<ButtonInput<MouseButton>>().just_pressed(MouseButton::Left) {
        return;
    }

    let pressed = {
        let mut query = world.query::<(Entity, &Interaction, &FieldHandle)>();
        query.iter(world).find(|(_, interaction, _)| **interaction == Interaction::Pressed).map(|(e, _, h)| (e, h.clone()))
    };
    let Some((ui_entity, handle)) = pressed else { return };

    match &handle.kind {
        FieldKind::Bool { path } => {
            let new_value = with_reflect_field_mut(world, handle.entity, handle.type_id, path, |field| {
                let b = field.try_downcast_mut::<bool>()?;
                *b = !*b;
                Some(*b)
            })
            .flatten();
            if let Some(v) = new_value {
                if let Some(mut text) = world.get_mut::<Text>(ui_entity) {
                    *text = Text::new(v.to_string());
                }
            }
        }
        FieldKind::ReadOnly => {}
        _ => {
            let mut state = world.resource_mut::<FieldEditState>();
            state.target = Some(ui_entity);
            state.buffer.clear();
            if let Some(mut background) = world.get_mut::<BackgroundColor>(ui_entity) {
                background.0 = FIELD_EDITING_BACKGROUND;
            }
        }
    }
}

fn handle_field_keyboard(world: &mut World) {
    let Some(ui_entity) = world.resource::<FieldEditState>().target else { return };

    let just_pressed: Vec<KeyCode> = world.resource::<ButtonInput<KeyCode>>().get_just_pressed().copied().collect();
    if just_pressed.is_empty() {
        return;
    }

    let Some(handle) = world.get::<FieldHandle>(ui_entity).cloned() else {
        world.resource_mut::<FieldEditState>().target = None;
        return;
    };
    let text_allowed = matches!(handle.kind, FieldKind::Str { .. });

    let mut commit = false;
    let mut cancel = false;
    {
        let mut state = world.resource_mut::<FieldEditState>();
        for key in &just_pressed {
            match key {
                KeyCode::Enter | KeyCode::NumpadEnter => commit = true,
                KeyCode::Escape => cancel = true,
                KeyCode::Backspace => {
                    state.buffer.pop();
                }
                _ => {
                    if let Some(ch) = char_for_key(*key, text_allowed) {
                        state.buffer.push(ch);
                    }
                }
            }
        }
    }

    if cancel {
        finish_editing(world, ui_entity);
        return;
    }
    if !commit {
        return;
    }

    let buffer = world.resource::<FieldEditState>().buffer.clone();
    let new_display = match &handle.kind {
        FieldKind::F32 { path } => buffer.parse::<f32>().ok().and_then(|v| {
            let ok = with_reflect_field_mut(world, handle.entity, handle.type_id, path, |field| {
                let f = field.try_downcast_mut::<f32>()?;
                *f = v;
                Some(())
            })
            .flatten()
            .is_some();
            ok.then(|| format_f32(v))
        }),
        FieldKind::Str { path } => {
            let ok = with_reflect_field_mut(world, handle.entity, handle.type_id, path, |field| {
                let s = field.try_downcast_mut::<String>()?;
                *s = buffer.clone();
                Some(())
            })
            .flatten()
            .is_some();
            ok.then(|| buffer.clone())
        }
        FieldKind::ColorChannel { field, channel } => buffer.parse::<f32>().ok().and_then(|v| {
            let ok = commit_color_channel(world, handle.entity, handle.type_id, field, *channel, v);
            ok.then(|| format_f32(v))
        }),
        FieldKind::Bool { .. } | FieldKind::ReadOnly => None,
    };

    if let Some(display) = new_display {
        if let Some(mut text) = world.get_mut::<Text>(ui_entity) {
            *text = Text::new(display);
        }
    }
    finish_editing(world, ui_entity);
}

fn finish_editing(world: &mut World, ui_entity: Entity) {
    let mut state = world.resource_mut::<FieldEditState>();
    state.target = None;
    state.buffer.clear();
    if let Some(mut background) = world.get_mut::<BackgroundColor>(ui_entity) {
        background.0 = FIELD_VALUE_BACKGROUND;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::reflect::AppTypeRegistry;
    use bevy_transform::components::Transform;
    use bv_editor_test_utils::{headless_app, simulate_click, simulate_key, step};

    fn setup() -> App {
        let mut app = headless_app();
        app.init_resource::<AppTypeRegistry>();
        app.register_type::<Transform>();
        app.register_type::<bevy_math::Vec3>();
        app.add_plugins(ReflectUiPlugin);
        app
    }

    fn spawn_transform_fields(app: &mut App, entity: Entity) -> Entity {
        use bevy_ecs::world::CommandQueue;

        let container = app.world_mut().spawn(Node::default()).id();
        let type_id = TypeId::of::<Transform>();

        let registry_arc = app.world().resource::<AppTypeRegistry>().0.clone();
        let registry = registry_arc.read();
        let reflect_component = registry.get_type_data::<ReflectComponent>(type_id).unwrap().clone();
        drop(registry);

        let world_ref = app.world();
        let entity_ref = world_ref.entity(entity);
        let value = reflect_component.reflect(entity_ref).unwrap();

        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, world_ref);
        spawn_component_fields(&mut commands, container, entity, type_id, value.as_partial_reflect());
        queue.apply(app.world_mut());

        container
    }

    fn field_node(app: &mut App, path_suffix: &str) -> Entity {
        let world = app.world_mut();
        let mut fields = world.query::<(Entity, &FieldHandle)>();
        fields
            .iter(world)
            .find(|(_, handle)| matches!(&handle.kind, FieldKind::F32 { path } if path == path_suffix))
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("no F32 field with path {path_suffix:?}"))
    }

    #[test]
    fn editing_translation_x_writes_back_into_the_real_component() {
        let mut app = setup();
        let entity = app.world_mut().spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();
        spawn_transform_fields(&mut app, entity);
        step(&mut app, 1);

        let field = field_node(&mut app, "translation.x");
        app.world_mut().entity_mut(field).insert(Interaction::Pressed);
        simulate_click(&mut app, bevy_math::Vec2::ZERO);
        step(&mut app, 1); // click: starts editing, clears the buffer

        for key in [KeyCode::Digit4, KeyCode::Period, KeyCode::Digit5] {
            simulate_key(&mut app, key);
            step(&mut app, 1);
        }
        simulate_key(&mut app, KeyCode::Enter);
        step(&mut app, 1);

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation.x, 4.5);
        assert_eq!(transform.translation.y, 2.0);
        assert_eq!(transform.translation.z, 3.0);
    }

    #[test]
    fn escape_cancels_without_writing() {
        let mut app = setup();
        let entity = app.world_mut().spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();
        spawn_transform_fields(&mut app, entity);
        step(&mut app, 1);

        let field = field_node(&mut app, "translation.x");
        app.world_mut().entity_mut(field).insert(Interaction::Pressed);
        simulate_click(&mut app, bevy_math::Vec2::ZERO);
        step(&mut app, 1);
        assert_eq!(app.world().resource::<FieldEditState>().target, Some(field), "click should have started editing");

        simulate_key(&mut app, KeyCode::Digit9);
        step(&mut app, 1);
        simulate_key(&mut app, KeyCode::Escape);
        step(&mut app, 1);

        assert_eq!(app.world().get::<Transform>(entity).unwrap().translation.x, 1.0);
        assert!(app.world().resource::<FieldEditState>().target.is_none());
    }
}
