//! Rudra - Bevy UI layer.
//!
//! The `Document` (in `crate::document2`) is the single source of truth; this
//! module is the *view* and holds no derived document state. On any change a
//! `Dirty` flag is set, and `rebuild_view` discards the whole view and respawns
//! it from the document - the deliberately simple whole-view-rebuild approach.
//!
//! Selection (the focused cell, copy mode) is view-only state in the `Ui`
//! resource and is baked into each cell's colours when the view is rebuilt.
//! Hovering is separate: per-cell `Over`/`Out` observers swap a cell's
//! background to its hover colour and back, with `propagate(false)` keeping a
//! hover confined to the innermost cell under the pointer.

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

/// An entity whose background swaps between two colours on pointer hover.
/// Carried by every cell and every toolbar button. `rest` is the colour the
/// view was built with (it already reflects selection state); `hover` is shown
/// while the pointer is directly over the entity.
#[derive(Component)]
struct Hoverable {
    rest: Color,
    hover: Color,
}

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
            BackgroundColor(APP_BG),
        ))
        .with_children(|root| {
            // top to bottom: toolbar, status line, then the regions fill the rest
            crate::spawn_section::spawn_columns(
                root,
                |cs| spawn_region(cs, Region::Rom, doc, types, ui),
                |cs| spawn_region(cs, Region::Ram, doc, types, ui),
                |cs| spawn_region(cs, Region::Rim, doc, types, ui),
            );
            spawn_toolbar(root);
            spawn_status(root, ui);
        });
}

fn spawn_toolbar(root: &mut ChildSpawnerCommands) {
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

    root.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
    ))
    .with_children(|bar| {
        for (label, action) in actions {
            bar.spawn((
                ButtonTag(action),
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(BUTTON_BG),
                Hoverable { rest: BUTTON_BG, hover: lighten(BUTTON_BG, 0.22) },
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(label),
                    TextFont { font_size: 13.0, ..default() },
                    TextColor(TEXT_FG),
                ));
            })
            .observe(on_button_click)
            .observe(observe_over)
            .observe(observe_out);
        }
    });
}

fn spawn_status(root: &mut ChildSpawnerCommands, ui: &Ui) {
    root.spawn((
        Node { padding: UiRect::all(Val::Px(6.0)), ..default() },
        BackgroundColor(PANEL_BG),
    ))
    .with_children(|s| {
        s.spawn((
            Text::new(status_text(ui)),
            TextFont { font_size: 13.0, ..default() },
            TextColor(STATUS_FG),
        ));
    });
}

fn spawn_region(
    parent: &mut ChildSpawnerCommands,
    region: Region,
    doc: &Document,
    types: &Types,
    ui: &Ui,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(8.0)),
            align_items: AlignItems::Start,
            ..default()
        })
        .with_children(|col| {
            // each region is one root Cell, rendered recursively
            let root_loc = CellLocation { region, path: vec![] };
            spawn_cell(col, doc.root(region), &root_loc, types, ui);
        });
}

// ===========================================================================
// The recursive cell renderer - the heart of the view
// ===========================================================================

fn spawn_cell(
    parent: &mut ChildSpawnerCommands,
    cell: &Cell,
    loc: &CellLocation,
    types: &Types,
    ui: &Ui,
) {
    let v = cell_visual(loc, ui, cell);

    // shared by every cell variant: a tagged, coloured, hoverable box
    let base = (
        CellTag(loc.clone()),
        BackgroundColor(v.rest_bg),
        BorderColor::all(v.border),
        Hoverable { rest: v.rest_bg, hover: v.hover_bg },
    );

    match cell {
        // --- a symbol: an editable text box -------------------------------
        Cell::Symbol(s) => {
            let shown = if s.is_empty() { "EMPTY".to_string() } else { s.clone() };
            parent
                .spawn((
                    base,
                    Node {
                        min_width: Val::Px(44.0),
                        min_height: Val::Px(28.0),
                        border: UiRect::all(Val::Px(CELL_BORDER)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                ))
                .with_children(|c| {
                    c.spawn((
                        Text::new(shown),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(TEXT_FG),
                    ));
                })
                .observe(on_cell_click)
                .observe(observe_over)
                .observe(observe_out);
        }

        // --- an empty cell: a bordered placeholder ------------------------
        Cell::Empty => {
            parent
                .spawn((
                    base,
                    Node {
                        min_width: Val::Px(44.0),
                        min_height: Val::Px(28.0),
                        border: UiRect::all(Val::Px(CELL_BORDER)),
                        ..default()
                    },
                ))
                .observe(on_cell_click)
                .observe(observe_over)
                .observe(observe_out);
        }

        // --- a struct: a header plus one child per field ------------------
        Cell::Struct(sv) => {
            let header = struct_header(sv, types);
            parent
                .spawn((
                    base,
                    Node {
                        flex_direction: FlexDirection::Column,
                        border: UiRect::all(Val::Px(CELL_BORDER)),
                        ..default()
                    },
                ))
                .with_children(|c| {
                    c.spawn((
                        Text::new(header),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(HEADER_FG),
                    ));
                    for (i, field) in sv.fields.iter().enumerate() {
                        let field_loc = child(loc, PathStep::Struct(i));
                        spawn_cell(c, field, &field_loc, types, ui);
                    }
                })
                .observe(on_cell_click)
                .observe(observe_over)
                .observe(observe_out);
        }

        // --- a tree: a width x height arrangement of cells ----------------
        Cell::Tree(t) => {
            parent
                .spawn((
                    base,
                    Node {
                        flex_direction: FlexDirection::Column,
                        border: UiRect::all(Val::Px(CELL_BORDER)),
                        ..default()
                    },
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
                .observe(on_cell_click)
                .observe(observe_over)
                .observe(observe_out);
        }
    }
}

// ===========================================================================
// Hover (observers)
// ===========================================================================
//
// `propagate(false)` confines the hover to the innermost cell under the
// pointer: without it the same `Over` would bubble up the parent chain and
// every enclosing cell would light up at once.

fn observe_over(mut over: On<Pointer<Over>>, mut q: Query<(&Hoverable, &mut BackgroundColor)>) {
    over.propagate(false);
    if let Ok((h, mut bg)) = q.get_mut(over.entity) {
        *bg = BackgroundColor(h.hover);
    }
}

fn observe_out(mut out: On<Pointer<Out>>, mut q: Query<(&Hoverable, &mut BackgroundColor)>) {
    out.propagate(false);
    if let Ok((h, mut bg)) = q.get_mut(out.entity) {
        // back to the resting colour - which already reflects selection state
        *bg = BackgroundColor(h.rest);
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
// Palette
// ===========================================================================

// app chrome
const APP_BG: Color = Color::srgb(0.09, 0.09, 0.11);
const PANEL_BG: Color = Color::srgb(0.13, 0.13, 0.16);
const BUTTON_BG: Color = Color::srgb(0.22, 0.22, 0.28);

// text
const TEXT_FG: Color = Color::srgb(0.92, 0.92, 0.94);
const HEADER_FG: Color = Color::srgb(0.90, 0.80, 0.60);
const STATUS_FG: Color = Color::srgb(0.70, 0.72, 0.78);

// cell backgrounds, by cell type (subtle, dark, distinguishable)
const BG_SYMBOL: Color = Color::srgb(0.17, 0.19, 0.25);
const BG_EMPTY: Color = Color::srgb(0.12, 0.12, 0.14);
const BG_STRUCT: Color = Color::srgb(0.23, 0.19, 0.15);
const BG_TREE: Color = Color::srgb(0.14, 0.21, 0.21);

// cell borders, by cell type
const BD_SYMBOL: Color = Color::srgb(0.36, 0.40, 0.52);
const BD_EMPTY: Color = Color::srgb(0.26, 0.26, 0.30);
const BD_STRUCT: Color = Color::srgb(0.52, 0.42, 0.30);
const BD_TREE: Color = Color::srgb(0.30, 0.52, 0.52);

// selection - shown on top of the per-type colours
const FOCUS_BG: Color = Color::srgb(0.18, 0.30, 0.46);
const FOCUS_BORDER: Color = Color::srgb(0.40, 0.66, 1.00);
const COPY_DEST_BG: Color = Color::srgb(0.34, 0.24, 0.10);
const COPY_DEST_BORDER: Color = Color::srgb(1.00, 0.66, 0.26);

/// Cell border thickness, in px.
const CELL_BORDER: f32 = 5.0;

// ===========================================================================
// Visuals
// ===========================================================================

/// The three colours a cell is drawn with.
struct CellVisual {
    rest_bg: Color,
    hover_bg: Color,
    border: Color,
}

/// Resolve a cell's colours from its type and the current selection state.
/// Selection wins over the per-type colour; the hover colour is just the
/// resting background, lightened.
fn cell_visual(loc: &CellLocation, ui: &Ui, cell: &Cell) -> CellVisual {
    let is_copy_dest = matches!(&ui.mode, Mode::AwaitingCopySource { dest } if dest == loc);
    let is_focused = ui.focused.as_ref() == Some(loc);

    let (rest_bg, border) = if is_copy_dest {
        (COPY_DEST_BG, COPY_DEST_BORDER)
    } else if is_focused {
        (FOCUS_BG, FOCUS_BORDER)
    } else {
        (type_bg(cell), type_border(cell))
    };

    CellVisual { rest_bg, hover_bg: lighten(rest_bg, 0.18), border }
}

fn type_bg(cell: &Cell) -> Color {
    match cell {
        Cell::Symbol(_) => BG_SYMBOL,
        Cell::Empty => BG_EMPTY,
        Cell::Struct(_) => BG_STRUCT,
        Cell::Tree(_) => BG_TREE,
    }
}

fn type_border(cell: &Cell) -> Color {
    match cell {
        Cell::Symbol(_) => BD_SYMBOL,
        Cell::Empty => BD_EMPTY,
        Cell::Struct(_) => BD_STRUCT,
        Cell::Tree(_) => BD_TREE,
    }
}

/// Move a colour fraction `t` of the way towards white.
fn lighten(c: Color, t: f32) -> Color {
    let s = c.to_srgba();
    Color::srgb(
        s.red + (1.0 - s.red) * t,
        s.green + (1.0 - s.green) * t,
        s.blue + (1.0 - s.blue) * t,
    )
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

fn region_color(r: Region) -> Color {
    match r {
        Region::Rom => Color::srgb(0.85, 0.55, 0.45),
        Region::Ram => Color::srgb(0.55, 0.80, 0.55),
        Region::Rim => Color::srgb(0.55, 0.70, 1.00),
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
