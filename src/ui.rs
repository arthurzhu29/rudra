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
//!
//! Each region has an independent pan and zoom held in the `Views` resource
//! (so they survive a rebuild). Dragging inside a region pans it, the mouse
//! wheel zooms it, and a per-region "Center" button fits the root cell to the
//! viewport. Pan/zoom is a `UiTransform` on the region's content node and never
//! triggers a rebuild.
//!
//! ---------------------------------------------------------------------------
//! API NOTE. A few names below are best-effort against Bevy 0.18 and may need a
//! local fix: `UiTransform` / `Val2::px`, the `Pointer<Drag>` `delta` accessor,
//! `RelativeCursorPosition::mouse_over()`, and `ComputedNode::size()`.
//! ---------------------------------------------------------------------------

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::document3::*;

// ===========================================================================
// Entry point
// ===========================================================================

pub fn run() {
    let types = crate::custom::sample_types();

    // The Rim canvas starts as a tree of symbols, so the grid operations
    // (add/delete row/column) have something to act on from the first frame.
    // let rim_field = FieldDef {
    //     name: "canvas".to_string(),
    //     value: CellValue::Symbol,
    //     is_tree: true,
    // };
    let document = crate::save_load::load(&types);

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(Doc(document))
        .insert_resource(Schema(types))
        .init_resource::<Ui>()
        .init_resource::<Views>()
        .insert_resource(Dirty(true)) // forces the first-frame build
        .add_systems(Startup, setup)
        // zoom feeds Views; rebuild respawns the view; apply_views then pushes
        // the current pan/zoom onto the (possibly just-spawned) content nodes.
        .add_systems(
            Update,
            (zoom_on_scroll, symbol_typing, rebuild_view, apply_views).chain(),
        )
        .add_systems(Update, (save, crate::save_load::save_query).run_if(
    bevy::time::common_conditions::on_timer(std::time::Duration::from_secs_f32(0.5))))
        .run();
}


pub struct StaticBuilder {
    pub root: &'static str,
    pub data: &'static[&'static[(&'static[(&'static str, Option<&'static str>)], &'static str)]],
}

pub fn save(document: Res<Doc>) {
    crate::serialization::test::test_serialize(&document.0);
}

// ===========================================================================
// Resources
// ===========================================================================

/// The document - the source of truth. Wrapped so it can be a Bevy resource.
#[derive(Resource)]
pub struct Doc(pub Document);

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
    focused: Option<CellPath>,
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
    AwaitingCopySource { dest: CellPath },
}

/// Independent pan/zoom for each of the three regions. Lives in a resource so
/// it survives the whole-view rebuild (the spawned entities do not).
#[derive(Resource, Default)]
struct Views {
    rom: ViewState,
    ram: ViewState,
    rim: ViewState,
}

/// One region's camera-like state: `pan` is a free screen-space offset of the
/// content, `zoom` a scale factor. Panning is unbounded - the content may be
/// dragged entirely outside the viewport, just like a pan camera.
#[derive(Clone, Copy)]
struct ViewState {
    pan: Vec2,
    zoom: f32,
}

impl Default for ViewState {
    fn default() -> Self {
        Self { pan: Vec2::ZERO, zoom: 1.0 }
    }
}

impl Views {
    fn get(&self, r: Region) -> ViewState {
        match r {
            Region::Rom => self.rom,
            Region::Ram => self.ram,
            Region::Rim => self.rim,
        }
    }
    fn get_mut(&mut self, r: Region) -> &mut ViewState {
        match r {
            Region::Rom => &mut self.rom,
            Region::Ram => &mut self.ram,
            Region::Rim => &mut self.rim,
        }
    }
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
struct CellTag(CellPath);

/// A toolbar button carries the operation it triggers.
#[derive(Component)]
struct ButtonTag(Action);

/// A region's "Center" button carries the region it re-centres.
#[derive(Component)]
struct CenterButton(Region);

/// The clipped viewport node of a region - catches drag (pan) and is the rect
/// the mouse wheel (zoom) and "Center" are measured against.
#[derive(Component)]
struct RegionViewport(Region);

/// The content node of a region - holds the cell tree and carries the
/// `UiTransform` that `apply_views` drives from `Views`.
#[derive(Component)]
struct RegionContent(Region);

/// An entity whose background swaps between two colours on pointer hover.
/// Carried by every cell and every button. `rest` is the colour the view was
/// built with (it already reflects selection state); `hover` is shown while
/// the pointer is directly over the entity.
#[derive(Component)]
struct Hoverable {
    rest: Color,
    hover: Color,
}

/// Carried alongside `Hoverable` by cells (which have a border): the border
/// colours to swap between on hover. Buttons omit it - they have no border to
/// change - so the observers simply leave their borders alone.
#[derive(Component)]
struct HoverBorder {
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
    views: Res<Views>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    // Drop the previous view entirely (despawn is recursive).
    for e in &old {
        commands.entity(e).despawn();
    }

    build_view(&mut commands, &doc.0, &schema.0, &ui, &views);
}

fn build_view(commands: &mut Commands, doc: &Document, types: &Types, ui: &Ui, views: &Views) {
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
            // the three region columns fill the space above the toolbar/status
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                ..default()
            })
            .with_children(|row| {
                for (i, region) in [Region::Rom, Region::Ram, Region::Rim].into_iter().enumerate()
                {
                    if i > 0 {
                        spawn_divider(row);
                    }
                    spawn_region(row, region, doc, types, ui, views.get(region));
                }
            });

            spawn_toolbar(root);
            spawn_status(root, ui);
        });
}

/// A thin fixed-width divider between two region columns.
fn spawn_divider(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            width: Val::Px(DIVIDER_WIDTH),
            height: Val::Percent(100.0),
            flex_shrink: 0.0, // always exactly DIVIDER_WIDTH wide
            ..default()
        },
        BackgroundColor(DIVIDER_COLOR),
    ));
}

/// One region column: a fixed header (name + Center button) above a clipped,
/// pannable/zoomable viewport. The three columns are kept exactly equal width
/// by `flex_basis: 0` + equal `flex_grow` + `min_width: 0`.
fn spawn_region(
    parent: &mut ChildSpawnerCommands,
    region: Region,
    doc: &Document,
    types: &Types,
    ui: &Ui,
    view: ViewState,
) {
    parent
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_basis: Val::Px(0.0), // ignore content size when sharing width
                flex_grow: 1.0,           // ...so the three columns split evenly
                min_width: Val::Px(0.0),  // allow shrinking below content width
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(SECTION_BG),
        ))
        .with_children(|section| {
            spawn_region_header(section, region);

            // the viewport: clips the content, and catches drags to pan
            section
                .spawn((
                    RegionViewport(region),
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    // lets `zoom_on_scroll` ask "is the cursor over this region?"
                    RelativeCursorPosition::default(),
                ))
                .observe(on_viewport_drag)
                .with_children(|viewport| {
                    // the content node: holds the cell tree and is transformed
                    // (pan + zoom) by `apply_views`. Absolutely positioned so
                    // its size shrink-wraps the tree and the transform origin
                    // is the viewport's top-left.
                    viewport
                        .spawn((
                            RegionContent(region),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                ..default()
                            },
                            UiTransform {
                                translation: Val2::px(view.pan.x, view.pan.y),
                                scale: Vec2::splat(view.zoom),
                                ..default()
                            },
                        ))
                        .with_children(|content| {
                            let root_loc = CellPath { region, path: vec![] };
                            spawn_cell(content, &doc[region], &root_loc, types, ui);
                        });
                });
        });
}

fn spawn_region_header(section: &mut ChildSpawnerCommands, region: Region) {
    section
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|header| {
            header.spawn((
                Text::new(region_name(region)),
                TextFont { font_size: 14.0, ..default() },
                TextColor(region_color(region)),
            ));
            header
                .spawn((
                    CenterButton(region),
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(BUTTON_BG),
                    Hoverable { rest: BUTTON_BG, hover: lighten(BUTTON_BG, 0.22) },
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("Center"),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(TEXT_FG),
                    ));
                })
                .observe(on_center_click)
                .observe(observe_over)
                .observe(observe_out);
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

// ===========================================================================
// The recursive cell renderer - the heart of the view
// ===========================================================================

fn spawn_cell(
    parent: &mut ChildSpawnerCommands,
    cell: &Cell,
    loc: &CellPath,
    types: &Types,
    ui: &Ui,
) {
    let v = cell_visual(loc, ui, cell);

    // shared by every cell variant: a tagged, coloured, hoverable box
    let base = (
        CellTag(loc.clone()),
        BackgroundColor(v.rest_bg),
        BorderColor::all(v.rest_border),
        Hoverable { rest: v.rest_bg, hover: v.hover_bg },
        HoverBorder { rest: v.rest_border, hover: v.hover_border },
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
                        border_radius: BorderRadius::all(px(CELL_BORDER)),
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
                        border_radius: BorderRadius::all(px(CELL_BORDER)),
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
                        border_radius: BorderRadius::all(px(CELL_BORDER)),
                        ..default()
                    },
                ))
                .with_children(|c| {
                    c.spawn((
                        Text::new(header),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(HEADER_FG),
                    ));
                    // for (i, field) in sv.fields.iter().enumerate() {
                    //     let field_loc = child(loc, PathStep::Struct(i));
                    //     spawn_cell(c, field, &field_loc, types, ui);
                    // }
                    if let Some(cell) = sv.grid.as_ref() {
                        let grid_loc = child(loc, PathStep::IntoStruct);
                        spawn_cell(c, cell, &grid_loc, types, ui);
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
                        // A real CSS grid, not nested flex rows: every cell
                        // shares the same column and row tracks, so a cell that
                        // grows grows its whole column/row and the grid stays
                        // aligned - instead of each row flexing on its own.
                        display: Display::Grid,
                        grid_template_columns: vec![RepeatedGridTrack::auto(t.width as u16)],
                        grid_template_rows: vec![RepeatedGridTrack::auto(t.height as u16)],
                        // each track sizes to its largest cell; every cell then
                        // stretches to fill its track, so the grid stays uniform
                        justify_items: JustifyItems::Stretch,
                        align_items: AlignItems::Stretch,
                        border: UiRect::all(Val::Px(CELL_BORDER)),
                        border_radius: BorderRadius::all(px(CELL_BORDER)),
                        ..default()
                    },
                ))
                .with_children(|grid| {
                    // cells are row-major in `contents`; the grid auto-flows
                    // them left-to-right, top-to-bottom into its tracks
                    for y in 0..t.height {
                        for x in 0..t.width {
                            let inner = &t.contents[y * t.width + x];
                            let inner_loc = child(loc, PathStep::Tree(x, y));
                            spawn_cell(grid, inner, &inner_loc, types, ui);
                        }
                    }
                })
                .observe(on_cell_click)
                .observe(observe_over)
                .observe(observe_out);
        }
        Cell::Field(field_val) => {
            let struct_def = &types.types[field_val.struct_id];
            let struct_name = &struct_def.name;
            let variant_def = &struct_def.variants[field_val.variant_id];
            let variant_name = &variant_def.name;
            let field_name = &variant_def.fields[field_val.field_id].name;
            parent
                .spawn((
                    base,
                    Node {
                        flex_direction: FlexDirection::Row,
                        border: UiRect::all(Val::Px(CELL_BORDER)),
                        border_radius: BorderRadius::all(px(CELL_BORDER)),
                        // lalala
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|hii| {
                    hii.spawn((
                        Text::new(format!("{}::{}::{}", struct_name, variant_name, field_name)),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(HEADER_FG),
                    ));
                    let value_loc = child(loc, PathStep::IntoField);
                    spawn_cell(hii, &field_val.value, &value_loc, types, ui);
                })
                .observe(on_cell_click)
                .observe(observe_over)
                .observe(observe_out);
        },
    }
}

// ===========================================================================
// Hover (observers)
// ===========================================================================
//
// `propagate(false)` confines the hover to the innermost cell under the
// pointer: without it the same `Over` would bubble up the parent chain and
// every enclosing cell would light up at once.

fn observe_over(
    mut over: On<Pointer<Over>>,
    mut q: Query<(
        &Hoverable,
        &mut BackgroundColor,
        Option<&HoverBorder>,
        Option<&mut BorderColor>,
    )>,
) {
    over.propagate(false);
    if let Ok((h, mut bg, hover_border, border)) = q.get_mut(over.entity) {
        *bg = BackgroundColor(h.hover);
        // cells carry both HoverBorder and BorderColor; buttons carry neither
        if let (Some(hb), Some(mut border)) = (hover_border, border) {
            *border = BorderColor::all(hb.hover);
        }
    }
}

fn observe_out(
    mut out: On<Pointer<Out>>,
    mut q: Query<(
        &Hoverable,
        &mut BackgroundColor,
        Option<&HoverBorder>,
        Option<&mut BorderColor>,
    )>,
) {
    out.propagate(false);
    if let Ok((h, mut bg, hover_border, border)) = q.get_mut(out.entity) {
        // back to the resting colours - which already reflect selection state
        *bg = BackgroundColor(h.rest);
        if let (Some(hb), Some(mut border)) = (hover_border, border) {
            *border = BorderColor::all(hb.rest);
        }
    }
}

// ===========================================================================
// Pan / zoom
// ===========================================================================
//
// Each region's content node carries a `UiTransform`. `Views` is the source of
// truth for pan/zoom; `apply_views` copies it onto the transforms every frame
// (and so also re-applies it to the content node a rebuild just respawned).
// None of this sets `Dirty` - pan/zoom never rebuilds the view.

/// Dragging anywhere inside a region's viewport pans that region. The drag
/// bubbles up from whatever cell it started on to the viewport, which owns this
/// observer - so a drag-on-a-cell pans, while a plain click still selects.
fn on_viewport_drag(
    mut drag: On<Pointer<Drag>>,
    viewports: Query<&RegionViewport>,
    mut views: ResMut<Views>,
) {
    drag.propagate(false);
    let Ok(&RegionViewport(region)) = viewports.get(drag.entity) else {
        return;
    };
    // `delta` is screen-space movement since the last drag event; pan is also
    // screen-space, so this is a direct add. Unbounded - pan as far as you like.
    views.get_mut(region).pan += drag.delta;
}

/// The mouse wheel zooms whichever region the cursor is over.
fn zoom_on_scroll(
    mut wheel: MessageReader<MouseWheel>,
    viewports: Query<(&RegionViewport, &RelativeCursorPosition)>,
    mut views: ResMut<Views>,
) {
    let dy: f32 = wheel.read().map(|e| e.y).sum();
    if dy == 0.0 {
        return;
    }
    for (&RegionViewport(region), cursor) in &viewports {
        if cursor.cursor_over() {
            let v = views.get_mut(region);
            v.zoom = (v.zoom * ZOOM_STEP.powf(dy)).clamp(ZOOM_MIN, ZOOM_MAX);
        }
    }
}

/// Push the current pan/zoom from `Views` onto each region's content node.
/// Cheap (three entities) and idempotent; runs every frame so drag/scroll show
/// immediately and a just-rebuilt content node is corrected the same frame.
fn apply_views(views: Res<Views>, mut contents: Query<(&RegionContent, &mut UiTransform)>) {
    for (&RegionContent(region), mut transform) in &mut contents {
        let v = views.get(region);
        transform.translation = Val2::px(v.pan.x, v.pan.y);
        transform.scale = Vec2::splat(v.zoom);
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
            let next = match &doc.0[&loc] {
                Cell::Struct(sv) => {
                    let count = types.types[sv.struct_id].variants.len();
                    if count == 0 {
                        return;
                    }
                    (sv.variant_id + 1) % count
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
            // rather than risk resolving a stale `CellPath`
            ui.focused = None;
            dirty.0 = true;
        }
    }
}

/// A region's "Center" button: fit the root cell to the viewport - centred,
/// fully visible, zoomed as large as possible with at least one dimension
/// touching the viewport edge.
fn on_center_click(
    click: On<Pointer<Click>>,
    buttons: Query<&CenterButton>,
    viewports: Query<(&RegionViewport, &ComputedNode)>,
    contents: Query<(&RegionContent, &ComputedNode)>,
    mut views: ResMut<Views>,
) {
    let Ok(&CenterButton(region)) = buttons.get(click.entity) else {
        return;
    };

    // the viewport's size, and the content's *natural* (unscaled) size -
    // `ComputedNode` is the layout result and is unaffected by `UiTransform`
    let Some((_, vp)) = viewports.iter().find(|(v, _)| v.0 == region) else {
        return;
    };
    let Some((_, ct)) = contents.iter().find(|(c, _)| c.0 == region) else {
        return;
    };
    let viewport = vp.size();
    let content = ct.size();
    if content.x <= 0.0 || content.y <= 0.0 {
        return; // nothing laid out yet
    }

    // largest zoom that still fits the whole root cell
    let zoom = (viewport.x / content.x).min(viewport.y / content.y);
    // ...then offset so the scaled content sits centred in the viewport.
    // (Assumes `UiTransform` scales about the node centre - if it scales about
    // the top-left instead, use `(viewport - content * zoom) * 0.5`.)
    let pan = (viewport - content) * 0.5;

    *views.get_mut(region) = ViewState { pan, zoom };
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
    let mut text = match &doc.0[&loc] {
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
const SECTION_BG: Color = Color::srgb(0.12, 0.12, 0.15);
const BUTTON_BG: Color = Color::srgb(0.22, 0.22, 0.28);
const DIVIDER_COLOR: Color = Color::srgb(0.22, 0.22, 0.26);

// text
const TEXT_FG: Color = Color::srgb(0.92, 0.92, 0.94);
const HEADER_FG: Color = Color::srgb(0.90, 0.80, 0.60);
const STATUS_FG: Color = Color::srgb(0.70, 0.72, 0.78);

// cell backgrounds, by cell type (subtle, dark, distinguishable)
const BG_SYMBOL: Color = Color::srgb(0.17, 0.19, 0.25);
const BG_EMPTY: Color = Color::srgb(0.12, 0.12, 0.14);
const BG_STRUCT: Color = Color::srgb(0.23, 0.19, 0.15);
const BG_TREE: Color = Color::srgb(0.14, 0.21, 0.21);
const BG_FIELD: Color = Color::srgb(0.11, 0.28, 0.08);

// cell borders, by cell type
const BD_SYMBOL: Color = Color::srgb(0.36, 0.40, 0.52);
const BD_EMPTY: Color = Color::srgb(0.26, 0.26, 0.30);
const BD_STRUCT: Color = Color::srgb(0.52, 0.42, 0.30);
const BD_TREE: Color = Color::srgb(0.30, 0.52, 0.52);
const BD_FIELD: Color = Color::srgb(0.22, 0.56, 0.16);

// selection - shown on top of the per-type colours
const FOCUS_BG: Color = Color::srgb(0.18, 0.30, 0.46);
const FOCUS_BORDER: Color = Color::srgb(0.40, 0.66, 1.00);
const COPY_DEST_BG: Color = Color::srgb(0.34, 0.24, 0.10);
const COPY_DEST_BORDER: Color = Color::srgb(1.00, 0.66, 0.26);

/// Cell border thickness, in px.
const CELL_BORDER: f32 = 5.0;
/// Width of the divider between region columns, in px.
const DIVIDER_WIDTH: f32 = 2.0;

// zoom limits and per-wheel-notch step
const ZOOM_STEP: f32 = 1.12;
const ZOOM_MIN: f32 = 0.1;
const ZOOM_MAX: f32 = 8.0;

// ===========================================================================
// Visuals
// ===========================================================================

/// The colours a cell is drawn with, at rest and on hover.
struct CellVisual {
    rest_bg: Color,
    hover_bg: Color,
    rest_border: Color,
    hover_border: Color,
}

/// Resolve a cell's colours from its type and the current selection state.
/// Selection wins over the per-type colour; the hover colour is just the
/// resting background, lightened.
fn cell_visual(loc: &CellPath, ui: &Ui, cell: &Cell) -> CellVisual {
    let is_copy_dest = matches!(&ui.mode, Mode::AwaitingCopySource { dest } if dest == loc);
    let is_focused = ui.focused.as_ref() == Some(loc);

    let (rest_bg, rest_border) = if is_copy_dest {
        (COPY_DEST_BG, COPY_DEST_BORDER)
    } else if is_focused {
        (FOCUS_BG, FOCUS_BORDER)
    } else {
        (type_bg(cell), type_border(cell))
    };

    CellVisual {
        rest_bg,
        hover_bg: lighten(rest_bg, 0.18),
        rest_border,
        // a brighter lift on the border than the fill, so the edge reads clearly
        hover_border: lighten(rest_border, 0.30),
    }
}

fn type_bg(cell: &Cell) -> Color {
    match cell {
        Cell::Symbol(_) => BG_SYMBOL,
        Cell::Empty => BG_EMPTY,
        Cell::Struct(_) => BG_STRUCT,
        Cell::Tree(_) => BG_TREE,
        Cell::Field(_) => BG_FIELD,
    }
}

fn type_border(cell: &Cell) -> Color {
    match cell {
        Cell::Symbol(_) => BD_SYMBOL,
        Cell::Empty => BD_EMPTY,
        Cell::Struct(_) => BD_STRUCT,
        Cell::Tree(_) => BD_TREE,
        Cell::Field(_) => BD_FIELD,
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
fn child(loc: &CellPath, step: PathStep) -> CellPath {
    let mut l = loc.clone();
    l.path.push(step);
    l
}

/// Rom is the read-only palette - only Ram and Rim accept mutations.
fn writable(loc: &CellPath) -> bool {
    loc.region != Region::Rom
}

/// A breadth op needs a cell whose location ends in a grid step.
fn is_grid_cell(loc: &CellPath) -> bool {
    matches!(loc.path.last(), Some(PathStep::Tree(_, _)))
}

/// e.g. `"Pair [xy]"` - struct name and the current variant's name.
fn struct_header(sv: &StructVal, types: &Types) -> String {
    let def = &types.types[sv.struct_id];
    let variant = &def.variants[sv.variant_id];
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
            "Click a cell to select it. Drag a region to pan, scroll to zoom. \
             The toolbar acts on the selection; typing edits a selected symbol."
                .to_string()
        }
        Mode::AwaitingCopySource { .. } => {
            "Copy: now click the SOURCE cell to copy it into the highlighted \
             destination."
                .to_string()
        }
    }
}