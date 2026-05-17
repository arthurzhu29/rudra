use std::ops::{Index, IndexMut};

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

enum TypeRef {
    Symbol,
    Struct(StructId),
}

struct Cell {
    content: CellContent,
}

enum CellContent {
    Value(LeafValue),                 // the T case
    Grid(Grid),                       // the Grid case; may be empty
}

struct Grid {
    cells: Vec<Cell>,                 // row-major; len() == width * height
    width: usize,
    height: usize,
}

enum LeafValue {
    Symbol(String),
    Struct(StructInstance),
}

struct StructInstance {
    struct_id: StructId,
    variant: usize,                   // index into StructDef::variants
    fields: Vec<Cell>,                // one Cell per field of that variant
}

type StructId = u32;

struct CellLocation {
    region: Region,                   // Rom | Ram | Rim
    path: Vec<PathStep>,
}

enum PathStep {
    Grid { row: usize, col: usize },  // descend the grid axis
    Field { index: usize },           // descend the field axis
}

enum Region { Rom, Ram, Rim }

fn _flatten<'a>(cell: &'a Cell, out: &mut Vec<&'a LeafValue>) {
    match &cell.content {
        CellContent::Value(v) => out.push(v),
        CellContent::Grid(g) => {
            for c in &g.cells { _flatten(c, out); }   // cells are row-major
        }
    }
}

#[derive(Resource)]
struct Document {
    schema: Schema,
    rom: Cell,                        // content is always Grid
    ram: Cell,                        // content is always Grid
    rim: Cell,                        // content is always Grid
    selection: Selection,
    dirty: RegionMask,                // which regions need a view rebuild
}

struct RegionMask; // TODO

#[derive(Component)]
struct CellView {
    loc: CellLocation,                // back-reference into the document
}

struct Selection {
    rom: CellLocation,                // always present (design §5.4)
    rim: CellLocation,                // always present (design §5.4)
    ram: Vec<CellLocation>,           // may be empty (design §5.2, §8.2)
}

fn _apply(doc: &mut Document, op: Operation) -> Result<(), MoveError> {
    todo!();
}

enum OperationType {
    RomToRam,
    RamToRam,
    RimToRam,
    RamToRim,
}

struct Operation {
    ty: OperationType,
    from: CellLocation,
    to: CellLocation,
}

#[derive(Resource, Default)]
struct History {
    undo: Vec<DocumentSnapshot>,
    redo: Vec<DocumentSnapshot>,
}

struct DocumentSnapshot {
    schema: Schema,
    ram: Cell,
    rim: Cell,
    selection: Selection,
}

fn move_ram_to_rim(doc: &mut Document, from: &[PathStep], to: &[PathStep])
    -> Result<(), MoveError>
{
    let dest_field = resolve_field_def(&doc.schema, &doc.rim, to);
    let candidate  = resolve_cell(&doc.ram, from);
    if let Some(dest_field) = dest_field {
        validate(candidate, dest_field, &doc.schema)
            .map_err(MoveError::Validation)?;
    }
    // only on success: detach from Ram, attach at `to`
    todo!()
}

fn resolve_cell<'a>(root: &'a Cell, path: &[PathStep]) -> &'a Cell {
    let mut current = root;
    for step in path {
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

fn resolve_cell_mut<'a>(root: &'a mut Cell, path: &[PathStep]) -> &'a mut Cell {
    let mut current = root;
    for step in path {
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

fn resolve_field_def<'a>(schema: &'a Schema, root: &Cell, path: &[PathStep]) -> Option<&'a FieldDef> {
    let mut def = None;
    let mut current = root;
    for step in path {
        let (new_def, new_current) = match (&current.content, step) {
            (
                &CellContent::Value(LeafValue::Struct(
                    StructInstance { struct_id, variant, ref fields, .. }
                )),
                &PathStep::Field { index },
            ) => {
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

fn validate(cell: &Cell, field: &FieldDef, schema: &Schema)
    -> Result<(), ValidationError>
{
    match &cell.content {
        CellContent::Grid(g) => {
            if !field.is_tree { return Err(/* non-tree field, got a grid */ todo!()); }
            for c in &g.cells { validate_as_elem(c, &field.elem, schema)?; }
            Ok(())
        }
        CellContent::Value(_) => validate_as_elem(cell, &field.elem, schema),
    }
}

fn validate_as_elem(cell: &Cell, elem: &TypeRef, schema: &Schema) -> Result<(), ValidationError> {
    todo!()
}