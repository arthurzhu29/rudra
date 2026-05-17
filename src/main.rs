use std::ops::{Index, IndexMut};
use std::mem;

use bevy::prelude::*;

fn main() {
    println!("Hello, world!");
}

struct Schema {
    structs: Vec<StructDef>,          // indexed by StructId
}

struct StructDef {
    name: String,
    variants: Vec<VariantDef>,        // 0 variants is permitted (design §2)
}

struct VariantDef {
    name: String,
    fields: Vec<FieldDef>,
}

struct FieldDef {
    name: String,
    elem: TypeRef,                    // the element type T
    is_tree: bool,                    // the depth-1 toggle (design §3.2)
}

#[derive(Clone, Copy)]
enum TypeRef {
    Symbol,
    Struct(StructId),
}

#[derive(Default, Clone)]
struct Cell {
    content: CellContent,
}
impl Cell {
    fn new(content: CellContent) -> Self {
        Self { content }
    }
}


#[derive(Default, Clone)]
enum CellContent {
    Value(LeafValue),                 // the T case
    Grid(Grid),                       // the Grid case; may not be empty
    #[default]
    Empty,
}

#[derive(Clone)]
struct Grid {
    cells: Vec<Cell>,                 // row-major; len() == width * height
    width: usize,
    height: usize,
}
impl Grid {
    fn new(cells: Vec<Cell>, width: usize, height: usize) -> Self {
        Self { cells, width, height }
    }
}

#[derive(Clone)]
enum LeafValue {
    Symbol(String),
    Struct(StructInstance),
}

#[derive(Clone)]
struct StructInstance {
    struct_id: StructId,
    variant: Option<usize>,           // index into StructDef::variants; None = no variant picked yet
    fields: Vec<Cell>,                // one Cell per field of the chosen variant
}

type StructId = u32;

struct CellLocation {
    region: Region,                   // Rom | Ram | Rim
    path: Path,
}

impl CellLocation {
    fn new(region: Region, path: Path) -> Self {
        Self { region, path }
    }
}

#[derive(Clone)]
enum PathStep {
    Grid { row: usize, col: usize },  // descend the grid axis
    Field { index: usize },           // descend the field axis
}

#[derive(Copy, Clone)]
enum Region { Rom, Ram, Rim }

fn _flatten<'a>(cell: &'a Cell, out: &mut Vec<&'a LeafValue>) {
    match &cell.content {
        CellContent::Value(v) => out.push(v),
        CellContent::Grid(g) => {
            for c in &g.cells { _flatten(c, out); }   // cells are row-major
        }
        CellContent::Empty => {},
    }
}

#[derive(Resource)]
struct Document {
    schema: Schema,
    rom: Root,                        // content is always Grid
    ram: Root,                        // content is always Grid
    rim: Root,                        // content is always Grid
    selection: Selection,
    dirty: RegionMask,                // which regions need a view rebuild
}

impl Document {
    fn root(&self, region: Region) -> &Root {
        match region {
            Region::Ram => &self.ram,
            Region::Rim => &self.rim,
            Region::Rom => &self.rom,
        }
    }
    fn root_mut(&mut self, region: Region) -> &mut Root {
        match region {
            Region::Ram => &mut self.ram,
            Region::Rim => &mut self.rim,
            Region::Rom => &mut self.rom,
        }
    }
}

/// Per-region "needs a view rebuild" flags (impl §2.3).
#[derive(Default, Clone, Copy)]
struct RegionMask {
    rom: bool,
    ram: bool,
    rim: bool,
}

impl RegionMask {
    /// Every region dirty — used when restoring a snapshot (impl §6.1).
    fn all() -> Self {
        Self { rom: true, ram: true, rim: true }
    }
    fn mark(&mut self, region: Region) {
        match region {
            Region::Rom => self.rom = true,
            Region::Ram => self.ram = true,
            Region::Rim => self.rim = true,
        }
    }
}

#[derive(Component)]
struct CellView {
    loc: CellLocation,                // back-reference into the document
}

struct Selection {
    rom: Path,                // always present (design §5.4)
    rim: Path,                // always present (design §5.4)
    ram: Vec<Path>,           // may be empty (design §5.2, §8.2)
}

#[derive(Clone)]
struct Path(Vec<PathStep>);

fn apply(doc: &mut Document, op: Operation) -> Result<(), MoveError> {
    let (from, to_region, to_paths) = match op {
        Operation::ToRam { from, to_paths } => (from, Region::Ram, to_paths),
        Operation::RamToRim { from, to } => {
            // Ram → Rim is the only fallible move: validate before touching anything.
            check_move_ram_to_rim(doc, &from, &to)?;
            (CellLocation::new(Region::Ram, from), Region::Rim, vec![to])
        }
        Operation::Invalid => return Err(MoveError::Forbidden),
    };

    // Rom is read-only (design §8, impl §7): moving *out* of Rom is a copy and
    // Rom itself is never mutated. Every other source region is moved from.
    let source_is_rom = matches!(from.region, Region::Rom);
    let cell = if source_is_rom {
        doc.root(from.region).resolve_cell(&from.path).clone()
    } else {
        mem::take(doc.root_mut(from.region).resolve_cell_mut(&from.path))
    };

    // The first target receives the moved cell; any extra targets (Ram
    // multi-select duplication) receive clones. Zero targets => the cell is
    // dropped, which is exactly deletion (design §8.2).
    let mut targets = to_paths.into_iter();
    let first = targets.next();
    let root = doc.root_mut(to_region);
    for path in targets {
        *root.resolve_cell_mut(&path) = cell.clone();
    }
    if let Some(path) = first {
        *root.resolve_cell_mut(&path) = cell;
    }

    doc.dirty.mark(to_region);
    if !source_is_rom {
        doc.dirty.mark(from.region);
    }
    Ok(())
}


enum Operation {
    ToRam {
        from: CellLocation,
        to_paths: Vec<Path>,
    },
    RamToRim {
        from: Path,
        to: Path,
    },
    Invalid,
}

#[derive(Resource, Default)]
struct History {
    undo: Vec<DocumentSnapshot>,
    redo: Vec<DocumentSnapshot>,
}

struct DocumentSnapshot {
    schema: Schema,
    ram: Root,
    rim: Root,
    selection: Selection,
}

impl From<Document> for DocumentSnapshot {
    fn from(value: Document) -> Self {
        let Document {
            schema,
            ram,
            rim,
            selection,
            ..
        } = value;
        Self {
            schema,
            ram,
            rim,
            selection
        }
    }
}
impl From<DocumentSnapshot> for Document {
    fn from(value: DocumentSnapshot) -> Self {
        let DocumentSnapshot {
            schema,
            ram,
            rim,
            selection,
        } = value;
        let rom = rom_from_schema(&schema);
        Self {
            schema,
            rom,
            ram,
            rim,
            selection,
            // Restoring a snapshot rebuilds every region's view (impl §6.1).
            dirty: RegionMask::all(),
        }
    }
}

fn rom_from_schema(schema: &Schema) -> Root {
    let cells = schema.structs.iter().enumerate().map(|(id, _)|
        Cell::new(CellContent::Value(LeafValue::Struct(
            // A Rom palette struct has no variant chosen yet; the user picks
            // one once it has been copied into Ram.
            StructInstance { struct_id: id as u32, variant: None, fields: vec![] }
        )
    )))
        .chain([
            Cell::new(CellContent::Value(LeafValue::Symbol(String::new()))),
            Cell::new(CellContent::Empty),   // the bare Tree
        ])
        .collect::<Vec<_>>();
    let len = cells.len();
    let grid = Grid::new(cells, 1, len);
    let cell = Cell::new(CellContent::Grid(grid));
    Root(cell)
}

struct Root(Cell);
impl Root {
    fn resolve_cell(&self, path: &Path) -> &Cell {
        let mut current = &self.0;
        for step in &path.0 {
            current = match (&current.content, step) {
                (
                    CellContent::Value(LeafValue::Struct(
                        StructInstance { fields, .. }
                    )),
                    &PathStep::Field { index },
                ) => &fields[index],
                (CellContent::Grid(grid), &PathStep::Grid { row, col }) => {
                    &grid[(col, row)]
                },
                _ => unreachable!(),
            };
        }
        current
    }
    fn resolve_cell_mut(&mut self, path: &Path) -> &mut Cell {
        let mut current = &mut self.0;
        for step in &path.0 {
            current = match (&mut current.content, step) {
                (
                    CellContent::Value(LeafValue::Struct(
                        StructInstance { fields, .. }
                    )),
                    &PathStep::Field { index },
                ) => &mut fields[index],
                (CellContent::Grid(grid), &PathStep::Grid { row, col }) => {
                    &mut grid[(col, row)]
                },
                _ => unreachable!(),
            };
        }
        current
    }

    fn resolve_field_def<'a>(&self, schema: &'a Schema, path: &Path) -> Option<&'a FieldDef> {
        let mut def = None;
        let mut current = &self.0;
        for step in &path.0 {
            let (new_def, new_current) = match (&current.content, step) {
                (
                    &CellContent::Value(LeafValue::Struct(
                        StructInstance { struct_id, variant, ref fields, .. }
                    )),
                    &PathStep::Field { index },
                ) => {
                    let variant = variant
                        .expect("path descends a Field step into a struct with no variant selected");
                    let struct_def = &schema.structs[struct_id as usize];
                    let variant_def = &struct_def.variants[variant];
                    (Some(&variant_def.fields[index]), &fields[index])
                },
                (CellContent::Grid(grid), &PathStep::Grid { row, col }) => {
                    (def, &grid[(col, row)])
                },
                _ => unreachable!(),
            };
            def = new_def;
            current = new_current;
        }
        def
    }
}

fn check_move_ram_to_rim(doc: &Document, from: &Path, to: &Path)
    -> Result<(), MoveError>
{
    let dest_field = doc.rim.resolve_field_def(&doc.schema, to);
    if let Some(dest_field) = dest_field {
        let candidate = doc.ram.resolve_cell(from);
        let root = CellLocation::new(Region::Ram, from.clone());
        let mut descent = Vec::new();
        validate(candidate, dest_field, &doc.schema, &root, &mut descent)
            .map_err(MoveError::Validation)?;
    }
    Ok(())
}



impl Index<(usize, usize)> for Grid {
    type Output = Cell;
    fn index(&self, (x, y): (usize, usize)) -> &Self::Output {
        let idx = self.width * y + x;
        &self.cells[idx]
    }
}
impl IndexMut<(usize, usize)> for Grid {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        let idx = self.width * y + x;
        &mut self.cells[idx]
    }
}

enum MoveError {
    Forbidden,                        // Rom→Rim, within-Rom
    Validation(ValidationError),
}

struct ValidationError {
    offending: CellLocation,          // the leaf that failed
    expected: TypeRef,
}

impl ValidationError {
    fn new(offending: CellLocation, expected: TypeRef) -> Self {
        Self { offending, expected }
    }
}

/// Location of the cell currently being validated: the candidate root plus the
/// grid/field steps walked to reach it. Built so a rejected move can point the
/// UI at the exact offending leaf (design §8.3).
fn offending_location(root: &CellLocation, descent: &[PathStep]) -> CellLocation {
    let mut path = root.path.clone();
    path.0.extend(descent.iter().cloned());
    CellLocation::new(root.region, path)
}

/// Structural validation of a candidate against a destination field (design
/// §6.4). Walks the entire grid axis; `descent` accumulates the steps taken
/// from the candidate root so any failure can be located precisely.
fn validate(
    cell: &Cell,
    field: &FieldDef,
    schema: &Schema,
    root: &CellLocation,
    descent: &mut Vec<PathStep>,
) -> Result<(), ValidationError> {
    match &cell.content {
        CellContent::Grid(g) => {
            if !field.is_tree {
                // A non-tree field wants a single value, not a grid.
                return Err(ValidationError::new(
                    offending_location(root, descent.as_slice()),
                    field.elem,
                ));
            }
            for row in 0..g.height {
                for col in 0..g.width {
                    descent.push(PathStep::Grid { row, col });
                    validate(&g[(col, row)], field, schema, root, descent)?;
                    descent.pop();
                }
            }
            Ok(())
        },
        CellContent::Value(val) => {
            validate_as_elem(val, &field.elem, schema, root, descent)
        },
        // non-tree field will be cleared
        CellContent::Empty => Ok(()),
    }
}

/// Checks one leaf value against an element type. For a struct this recurses
/// the *field* axis: the variant must be chosen and in range, the field count
/// must match, and every field cell must be valid against its own FieldDef
/// (impl §8.2).
fn validate_as_elem(
    val: &LeafValue,
    elem: &TypeRef,
    schema: &Schema,
    root: &CellLocation,
    descent: &mut Vec<PathStep>,
) -> Result<(), ValidationError> {
    match (val, elem) {
        (LeafValue::Symbol(_), TypeRef::Symbol) => Ok(()),
        (LeafValue::Struct(instance), TypeRef::Struct(id))
            if instance.struct_id == *id =>
        {
            let struct_def = &schema.structs[*id as usize];

            // A struct value must have a variant chosen, and it must be in range.
            let Some(variant) = instance.variant else {
                return Err(ValidationError::new(
                    offending_location(root, descent.as_slice()),
                    *elem,
                ));
            };
            // the following is not necessary because invalid variants cannot be constructed
            // via the ui.
            // let Some(variant_def) = struct_def.variants.get(variant) else {
            //     return Err(ValidationError::new(
            //         offending_location(root, descent.as_slice()),
            //         *elem,
            //     ));
            // };
            let variant_def = &struct_def.variants[variant];

            // The instance must carry exactly one cell per declared field.
            if instance.fields.len() != variant_def.fields.len() {
                return Err(ValidationError::new(
                    offending_location(root, descent.as_slice()),
                    *elem,
                ));
            }

            // Recurse the field axis: each field cell against its FieldDef.
            for (index, (field_cell, field_def)) in
                instance.fields.iter().zip(&variant_def.fields).enumerate()
            {
                descent.push(PathStep::Field { index });
                validate(field_cell, field_def, schema, root, descent)?;
                descent.pop();
            }
            Ok(())
        }
        _ => Err(ValidationError::new(
            offending_location(root, descent.as_slice()),
            *elem,
        )),
    }
}
