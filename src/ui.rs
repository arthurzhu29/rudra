//! Rudra - Bevy UI layer (MVP).
//!
//! Architecture: the `Document` (in `crate::document2`) is the single source of
//! truth. This module is the *view*. It never holds derived state about the
//! document; whenever the document or the selection changes it sets a `Dirty`
//! flag, and `rebuild_view` throws the whole view away and respawns it from the
//! document. That "whole-view rebuild" is the deliberately simple approach - an
//! MVP does not need incremental diffing.
//!
//! ---------------------------------------------------------------------------
//! NOTE ON THE BEVY API. This targets Bevy 0.18, but the exact API surface here
//! is *not* compile-verified - the logic and structure are what matter. The
//! spots most likely to need a name/shape fix for your Bevy version:
//!   * the observer event wrapper - `Trigger<E>` vs `On<E>`;
//!   * `Trigger::target()` (may be `.entity()`) and `Trigger::propagate(bool)`;
//!   * the `with_children` closure parameter type (`ChildBuilder` vs
//!     `ChildSpawnerCommands` / `RelatedSpawnerCommands`);
//!   * `Text` / `TextFont` / `TextColor`, `Camera2d`, `Color::srgb`;
//!   * `KeyboardInput` / `Key` field names in `symbol_typing`;
//!   * whether `DefaultPlugins` under the crate's feature set pulls everything
//!     a windowed UI app needs.
//! Fix those names; the wiring, the data flow, and the recursion below should
//! otherwise hold as-is.
//! ---------------------------------------------------------------------------

use bevy::ecs::relationship::RelatedSpawnerCommands;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::document2::*;

// ===========================================================================
// Entry point
// ===========================================================================

pub fn run() {
    let types = sample_types();

    // The Rim canvas starts as a tree of symbols, so the grid operations
    // (add/delete row/column) have something to act on from the first frame.
    let rim_field = FieldDef {
        name: "canvas".to_string(),
        value: CellValue::Symbol,
        is_tree: true,
    };
    let document = Document::new(&types.0, rim_field);

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(Doc(document))
        .insert_resource(Schema(types))
        .init_resource::<Ui>()
        .insert_resource(Dirty(true)) // forces the first-frame build
        .add_systems(Startup, setup)
        // typing edits run before the rebuild so an edit shows the same frame
        .add_systems(Update, (symbol_typing, rebuild_view).chain())
        .run();
}

/// A small sample schema so the MVP shows something interesting: one struct
/// with a two-symbol variant (exercises variant cycling and field rendering)
/// and one struct with a tree-typed field (exercises nested grids).
fn sample_types() -> Types {
    Types(vec![
        // struct 0: "Pair"
        StructDef {
            name: "Pair".to_string(),
            variants: vec![
                // variant 0: the nameless empty variant
                StructVariant { name: String::new(), fields: vec![] },
                // variant 1: two symbol fields
                StructVariant {
                    name: "xy".to_string(),
                    fields: vec![
                        FieldDef { name: "x".to_string(), value: CellValue::Symbol, is_tree: false },
                        FieldDef { name: "y".to_string(), value: CellValue::Symbol, is_tree: false },
                    ],
                },
            ],
        },
        // struct 1: "Box"
        StructDef {
            name: "Box".to_string(),
            variants: vec![
                StructVariant { name: String::new(), fields: vec![] },
                StructVariant {
                    name: "grid".to_string(),
                    fields: vec![FieldDef {
                        name: "items".to_string(),
                        value: CellValue::Symbol,
                        is_tree: true,
                    }],
                },
            ],
        },
    ])
}

// ===========================================================================
// Resources
// ===========================================================================

/// The document - the source of truth. Wrapped so it can be a Bevy resource.
#[derive(Resource)]
struct Doc(Document);

/// The schema. Needed by `edit_variant` and to render struct headers.
#[derive(Resource)]
struct Schema(Types);

/// Set whenever the document or selection changes; `rebuild_view` consumes it.
#[derive(Resource)]
struct Dirty(bool);

/// View-only selection state. Deliberately *not* part of the document core.
#[derive(Resource, Default)]
struct Ui {
    /// The cell most operations act on - the "focused" / highlighted cell.
    focused: Option<CellLocation>,
    /// Copy is a two-click gesture; this tracks the in-between state.
    mode: Mode,
}

#[derive(Default, Clone, PartialEq)]
enum Mode {
    /// Ordinary selecting: a cell click just moves the focus.
    #[default]
    Normal,
    /// Copy was pressed while `dest` was focused; the next cell click names the
    /// source, and the copy is performed.
    AwaitingCopySource { dest: CellLocation },
}

// ===========================================================================
// Components - markers on the spawned view entities
// ===========================================================================

/// The single top node of the rebuilt view. `rebuild_view` despawns it (and,
/// recursively, the whole view) before respawning.
#[derive(Component)]
struct ViewRoot;

/// A rendered cell carries the document location it stands for.
#[derive(Component)]
struct CellTag(CellLocation);

/// A toolbar button carries the operation it triggers.
#[derive(Component)]
struct ButtonTag(Action);

#[derive(Clone, Copy)]
enum Action {
    Copy,
    AddRowAbove,
    AddRowBelow,
    AddColLeft,
    AddColRight,
    DeleteRowAbove,
    DeleteRowBelow,
    DeleteColLeft,
    DeleteColRight,
    NextVariant,
}

// ===========================================================================
// Startup
// ===========================================================================

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

// ===========================================================================
// Rebuild - drop the old view, build a fresh one from the document
// ===========================================================================

fn rebuild_view(
    mut commands: Commands,
    mut dirty: ResMut<Dirty>,
    old: Query<Entity, With<ViewRoot>>,
    doc: Res<Doc>,
    schema: Res<Schema>,
    ui: Res<Ui>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    // Drop the previous view entirely (despawn is recursive).
    for e in &old {
        commands.entity(e).despawn();
    }

    build_view(&mut commands, &doc.0, &schema.0, &ui);
}

fn build_view(commands: &mut Commands, doc: &Document, types: &Types, ui: &Ui) {
    commands
        .spawn((
            ViewRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.10, 0.10, 0.12)),
        ))
        .with_children(|root| {
            // --- status line ---
            root.spawn((
                Text::new(status_text(ui)),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.85, 0.80, 0.55)),
                Node { margin: UiRect::all(Val::Px(6.0)), ..default() },
            ));

            // --- toolbar ---
            spawn_toolbar(root);

            // --- the three regions, side by side ---
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_grow: 1.0,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|row| {
                for region in [Region::Rom, Region::Ram, Region::Rim] {
                    spawn_region(row, region, doc, types, ui);
                }
            });
        });
}

fn spawn_toolbar(root: &mut RelatedSpawnerCommands<ChildOf>) {
    // Plain ASCII labels - the default Bevy font is not guaranteed to carry
    // arrow glyphs.
    let actions = [
        ("Copy", Action::Copy),
        ("Row+ above", Action::AddRowAbove),
        ("Row+ below", Action::AddRowBelow),
        ("Col+ left", Action::AddColLeft),
        ("Col+ right", Action::AddColRight),
        ("Row- above", Action::DeleteRowAbove),
        ("Row- below", Action::DeleteRowBelow),
        ("Col- left", Action::DeleteColLeft),
        ("Col- right", Action::DeleteColRight),
        ("Variant +", Action::NextVariant),
    ];

    root.spawn(Node {
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(4.0),
        row_gap: Val::Px(4.0),
        padding: UiRect::all(Val::Px(6.0)),
        ..default()
    })
    .with_children(|bar| {
        for (label, action) in actions {
            bar.spawn((
                ButtonTag(action),
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.25, 0.25, 0.30)),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(label),
                    TextFont { font_size: 13.0, ..default() },
                    TextColor(Color::WHITE),
                ));
            })
            .observe(on_button_click);
        }
    });
}

fn spawn_region(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    region: Region,
    doc: &Document,
    types: &Types,
    ui: &Ui,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new(region_name(region)),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.60, 0.70, 1.0)),
            ));
            // each region is one root Cell, rendered recursively
            let root_loc = CellLocation { region, path: vec![] };
            spawn_cell(col, doc.root(region), &root_loc, types, ui);
        });
}

// ===========================================================================
// The recursive cell renderer - the heart of the view
// ===========================================================================

fn spawn_cell(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    cell: &Cell,
    loc: &CellLocation,
    types: &Types,
    ui: &Ui,
) {
    let bg = cell_background(loc, ui);

    match cell {
        // --- a symbol: an editable text box -------------------------------
        Cell::Symbol(s) => {
            let shown = if s.is_empty() { "EMPTY".to_string() } else { s.clone() };
            parent
                .spawn((
                    CellTag(loc.clone()),
                    Node {
                        min_width: Val::Px(44.0),
                        min_height: Val::Px(28.0),
                        padding: UiRect::all(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(bg),
                    BorderColor::all(Color::srgb(0.40, 0.40, 0.42)),
                ))
                .with_children(|c| {
                    c.spawn((
                        Text::new(shown),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                })
                .observe(on_cell_click);
        }

        // --- an empty cell: a bordered placeholder ------------------------
        Cell::Empty => {
            parent
                .spawn((
                    CellTag(loc.clone()),
                    Node {
                        min_width: Val::Px(44.0),
                        min_height: Val::Px(28.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(bg),
                    BorderColor::all(Color::srgb(0.30, 0.30, 0.32)),
                ))
                .observe(on_cell_click);
        }

        // --- a struct: a header plus one child per field ------------------
        Cell::Struct(sv) => {
            let header = struct_header(sv, types);
            parent
                .spawn((
                    CellTag(loc.clone()),
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                    BackgroundColor(bg),
                    BorderColor::all(Color::srgb(0.55, 0.45, 0.30)),
                ))
                .with_children(|c| {
                    c.spawn((
                        Text::new(header),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(Color::srgb(0.90, 0.80, 0.60)),
                    ));
                    for (i, field) in sv.fields.iter().enumerate() {
                        let field_loc = child(loc, PathStep::Struct(i));
                        spawn_cell(c, field, &field_loc, types, ui);
                    }
                })
                .observe(on_cell_click);
        }

        // --- a tree: a width x height arrangement of cells ----------------
        Cell::Tree(t) => {
            parent
                .spawn((
                    CellTag(loc.clone()),
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(3.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        row_gap: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(bg),
                    BorderColor::all(Color::srgb(0.30, 0.50, 0.50)),
                ))
                .with_children(|grid| {
                    // one flex row per grid row; cells are row-major in `contents`
                    for y in 0..t.height {
                        grid.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(2.0),
                            ..default()
                        })
                        .with_children(|row| {
                            for x in 0..t.width {
                                let inner = &t.contents[y * t.width + x];
                                let inner_loc = child(loc, PathStep::Tree(x, y));
                                spawn_cell(row, inner, &inner_loc, types, ui);
                            }
                        });
                    }
                })
                .observe(on_cell_click);
        }
    }
}

// ===========================================================================
// Click handlers (observers)
// ===========================================================================

/// A cell was clicked. In `Normal` mode this moves the focus; in
/// `AwaitingCopySource` mode this click names the copy source and performs it.
fn on_cell_click(
    mut click: On<Pointer<Click>>,
    tags: Query<&CellTag>,
    mut ui: ResMut<Ui>,
    mut doc: ResMut<Doc>,
    mut dirty: ResMut<Dirty>,
) {
    // a click on an inner cell must not also register on its container
    click.propagate(false);

    let Ok(tag) = tags.get(click.entity) else {
        return;
    };
    let clicked = tag.0.clone();

    // take() leaves `Mode::Normal` behind - so a copy is one-shot
    match std::mem::take(&mut ui.mode) {
        Mode::AwaitingCopySource { dest } => {
            // Rom is the read-only palette; it can be a source but not a target
            if writable(&dest) {
                doc.0.copy(&dest, &clicked);
            }
            ui.focused = Some(clicked);
            dirty.0 = true;
        }
        Mode::Normal => {
            ui.focused = Some(clicked);
            dirty.0 = true;
        }
    }
}

/// A toolbar button was clicked. Every action reads the focused cell.
fn on_button_click(
    click: On<Pointer<Click>>,
    buttons: Query<&ButtonTag>,
    mut ui: ResMut<Ui>,
    mut doc: ResMut<Doc>,
    schema: Res<Schema>,
    mut dirty: ResMut<Dirty>,
) {
    let Ok(&ButtonTag(action)) = buttons.get(click.entity) else {
        return;
    };
    // every action needs a focused cell to act on / from
    let Some(loc) = ui.focused.clone() else {
        return;
    };
    let types: &Types = &schema.0;

    match action {
        // Copy: remember the focused cell as the destination, then wait for a
        // source click. Pressing Copy again (or clicking a source) ends it.
        Action::Copy => {
            ui.mode = Mode::AwaitingCopySource { dest: loc };
            dirty.0 = true; // refresh the status line + the dest highlight
        }

        // Variant cycling: only on a struct, only where writable.
        Action::NextVariant => {
            if !writable(&loc) {
                return;
            }
            let next = match doc.0.resolve(&loc) {
                Cell::Struct(sv) => {
                    let count = types.0[sv.id].variants.len();
                    if count == 0 {
                        return;
                    }
                    (sv.variant + 1) % count
                }
                _ => return, // not a struct - nothing to cycle
            };
            doc.0.edit_variant(&loc, next, types);
            dirty.0 = true;
        }

        // Breadth operations: only valid on a cell *inside a grid* (the op pops
        // the trailing `Tree` step to find the enclosing tree) and only where
        // writable. The guard keeps `document2`'s `unwrap()`/`panic!()` paths
        // unreached.
        Action::AddRowAbove
        | Action::AddRowBelow
        | Action::AddColLeft
        | Action::AddColRight
        | Action::DeleteRowAbove
        | Action::DeleteRowBelow
        | Action::DeleteColLeft
        | Action::DeleteColRight => {
            if !writable(&loc) || !is_grid_cell(&loc) {
                return;
            }
            match action {
                Action::AddRowAbove => doc.0.add_row_above(&loc),
                Action::AddRowBelow => doc.0.add_row_below(&loc),
                Action::AddColLeft => doc.0.add_column_left(&loc),
                Action::AddColRight => doc.0.add_column_right(&loc),
                Action::DeleteRowAbove => doc.0.delete_row_above(&loc),
                Action::DeleteRowBelow => doc.0.delete_row_below(&loc),
                Action::DeleteColLeft => doc.0.delete_column_left(&loc),
                Action::DeleteColRight => doc.0.delete_column_right(&loc),
                _ => unreachable!(),
            }
            // a structural change can shift positional paths - drop the focus
            // rather than risk resolving a stale `CellLocation`
            ui.focused = None;
            dirty.0 = true;
        }
    }
}

// ===========================================================================
// Keyboard: typing edits the focused symbol cell
// ===========================================================================

fn symbol_typing(
    mut keys: MessageReader<KeyboardInput>,
    mut doc: ResMut<Doc>,
    ui: Res<Ui>,
    mut dirty: ResMut<Dirty>,
) {
    let Some(loc) = ui.focused.clone() else {
        return;
    };
    // only act when the focused cell actually is a symbol
    let mut text = match doc.0.resolve(&loc) {
        Cell::Symbol(s) => s.clone(),
        _ => return,
    };

    let mut changed = false;
    for ev in keys.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        match &ev.logical_key {
            Key::Character(c) => {
                text.push_str(c.as_str());
                changed = true;
            }
            Key::Space => {
                text.push(' ');
                changed = true;
            }
            Key::Backspace => {
                text.pop();
                changed = true;
            }
            _ => {}
        }
    }

    if changed {
        doc.0.edit_symbol(&loc, &text);
        dirty.0 = true;
    }
}

// ===========================================================================
// Small helpers
// ===========================================================================

/// `loc` with one more path step appended.
fn child(loc: &CellLocation, step: PathStep) -> CellLocation {
    let mut l = loc.clone();
    l.path.push(step);
    l
}

/// Rom is the read-only palette - only Ram and Rim accept mutations.
fn writable(loc: &CellLocation) -> bool {
    loc.region != Region::Rom
}

/// A breadth op needs a cell whose location ends in a grid step.
fn is_grid_cell(loc: &CellLocation) -> bool {
    matches!(loc.path.last(), Some(PathStep::Tree(_, _)))
}

/// The background tint for a rendered cell, given the current selection.
fn cell_background(loc: &CellLocation, ui: &Ui) -> Color {
    if let Mode::AwaitingCopySource { dest } = &ui.mode {
        if dest == loc {
            return Color::srgb(0.45, 0.30, 0.12); // the pending copy destination
        }
    }
    if ui.focused.as_ref() == Some(loc) {
        return Color::srgb(0.20, 0.35, 0.55); // the focused cell
    }
    Color::srgb(0.16, 0.16, 0.18) // default
}

/// e.g. `"Pair [xy]"` - struct name and the current variant's name.
fn struct_header(sv: &StructVal, types: &Types) -> String {
    let def = &types.0[sv.id];
    let variant = &def.variants[sv.variant];
    let vname = if variant.name.is_empty() { "-" } else { variant.name.as_str() };
    format!("{} [{}]", def.name, vname)
}

fn region_name(r: Region) -> &'static str {
    match r {
        Region::Rom => "Rom - palette (read-only)",
        Region::Ram => "Ram - scratch",
        Region::Rim => "Rim - canvas",
    }
}

fn status_text(ui: &Ui) -> String {
    match &ui.mode {
        Mode::Normal => {
            "Click a cell to select it. The toolbar acts on the selection; \
             typing edits a selected symbol."
                .to_string()
        }
        Mode::AwaitingCopySource { .. } => {
            "Copy: now click the SOURCE cell to copy it into the highlighted \
             destination."
                .to_string()
        }
    }
}
