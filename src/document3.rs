use std::{mem, ops::{Index, IndexMut}};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Document {
    pub rim: Cell,
    pub ram: Cell,
    pub rom: Cell,
}


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum Cell {
    Symbol(String),
    Struct(StructVal),
    Tree(Tree),
    #[default]
    Empty,
    Field(FieldVal),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FieldVal {
    pub struct_id: usize,
    pub variant_id: usize,
    pub field_id: usize,
    pub value: Box<Cell>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StructVal {
    pub struct_id: usize,
    pub variant_id: usize,
    pub grid: Option<Box<Cell>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Tree {
    pub contents: Vec<Cell>,
    pub width: usize,
    pub height: usize,
}


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CellPath {
    pub region: Region,
    pub path: Vec<PathStep>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PathStep {
    IntoStruct,
    IntoField,
    Tree(usize, usize),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Region {
    Rim,
    Ram,
    Rom,
}


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Types {
    pub types: Vec<StructDef>,
    pub rim: FieldDef,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StructDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VariantDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub value: CellValue,
    pub is_tree: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CellValue {
    Symbol,
    Struct(usize),
}

impl FieldDef {
    pub fn default(&self, types: &Types) -> Cell {
        if self.is_tree {
            Tree::any_default()
        } else {
            self.value.default(types)
        }
    }
}

impl Cell {
    pub fn symbol() -> Self {
        Self::Symbol(String::new())
    }
    pub fn is_empty(&self) -> bool {
        matches!(self, Cell::Empty)
    }
}

impl Types {
    pub fn default_struct_variant(&self, struct_id: usize, variant_id: usize) -> Cell {
        Cell::Struct(StructVal {
            struct_id,
            variant_id,
            grid: {
                let contents = self.types[struct_id].variants[variant_id].fields.iter().enumerate().map(|(field_id, fd)| Cell::Field(FieldVal { struct_id, variant_id, field_id, value: Box::new(fd.default(self)) })).collect::<Vec<_>>();
                (!contents.is_empty()).then_some(
                    Box::new(Cell::Tree(Tree {
                        width: 1,
                        height: contents.len(),
                        contents,
                    }))
                )
            },
        })
    }
    pub fn default_struct(&self, n: usize) -> Cell {
        self.default_struct_variant(n, 0)
    }
}


impl CellValue {
    pub fn default(&self, types: &Types) -> Cell {
        match self {
            Self::Symbol => Cell::symbol(),
            Self::Struct(n) => types.default_struct(*n),
        }
    }
}

impl Tree {
    pub fn any_default() -> Cell {
        Cell::Tree(Tree { contents: vec![Cell::Empty], width: 1, height: 1 })
    }
}

impl Document {
    pub fn new(types: &Types) -> Self {
        Document {
            rim: types.rim.default(types),
            ram: Tree::any_default(),
            rom: Cell::Tree({
                let width = types.types.iter().map(|sd| sd.variants.len()).max().unwrap_or(1);
                let height = types.types.len() + 2;
                let mut contents = Vec::with_capacity(width * height);
                for (struct_id, sd) in types.types.iter().enumerate() {
                    let variants = sd.variants.len();
                    for variant_id in 0 .. variants {
                        contents.push(types.default_struct_variant(struct_id, variant_id));
                    }
                    for _ in 0 .. width - variants {
                        contents.push(Cell::Empty);
                    }
                }
                for item in [Tree::any_default(), Cell::symbol()] {
                    contents.push(item);
                    for _ in 0 .. width - 1 {
                        contents.push(Cell::Empty);
                    }
                }
                Tree {
                    contents,
                    width,
                    height,
                }
            }),
        }
    }
}
impl Index<&CellPath> for Document {
    type Output = Cell;
    fn index(&self, index: &CellPath) -> &Self::Output {
        index.path.iter().fold(
            &self[index.region],
            |current, pathstep| &current[pathstep],
        )
    }
}
impl IndexMut<&CellPath> for Document {
    fn index_mut(&mut self, index: &CellPath) -> &mut Self::Output {
        index.path.iter().fold(
            &mut self[index.region],
            |current, pathstep| &mut current[pathstep],
        )
    }
}
// ops
impl Document {
    /// Copy `src` onto `dest`. Applied only if the result is a well-formed
    /// document; on rejection the document is left unchanged and the offending
    /// cell (here, the destination) is returned so the UI can flag it red.
    pub fn copy(&mut self, dest: &CellPath, src: &CellPath, types: &Types) -> Result<(), CellPath> {
        if dest.region == Region::Rom {
            return Err(dest.clone()); // the palette is read-only
        }
        let mut trial = self.clone();
        trial[dest] = trial[src].clone();
        if is_valid(&trial, types) {
            *self = trial;
            Ok(())
        } else {
            Err(dest.clone())
        }
    }
    /// Move `src` onto `dest`, leaving `Empty` behind at `src`. Applied only if
    /// the result is well-formed; on rejection the document is unchanged and
    /// the offending cell is returned. Only a tree cell may be moved - only it
    /// may legally become `Empty` - and the palette is read-only.
    pub fn mova(&mut self, src: &CellPath, dest: &CellPath, types: &Types) -> Result<(), CellPath> {
        if src.region == Region::Rom || !ends_in_tree(src) {
            return Err(src.clone());
        }
        if dest.region == Region::Rom || !ends_in_tree(dest) {
            return Err(dest.clone());
        }
        let mut trial = self.clone();
        trial[dest] = mem::take(&mut trial[src]);
        if is_valid(&trial, types) {
            *self = trial;
            Ok(())
        } else {
            Err(dest.clone())
        }
    }
    pub fn add_column_right(&mut self, cell: &CellPath) {
        self.add_column(cell, true);
    }
    pub fn add_column_left(&mut self, cell: &CellPath) {
        self.add_column(cell, false);
    }
    pub fn add_column(&mut self, cell: &CellPath, is_right: bool) {
        let mut parent = cell.clone();
        let PathStep::Tree(x, _) = parent.path.pop().unwrap() else {
            panic!();
        };
        let parent = &mut self[&parent];
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            for i in (0 .. *height).map(|i| i * *width + x + is_right as usize).rev() {
                contents.insert(i, Cell::Empty);
            }
            *width += 1;
        }
    }
    pub fn add_row_above(&mut self, cell: &CellPath) {
        self.add_row(cell, false);
    }
    pub fn add_row_below(&mut self, cell: &CellPath) {
        self.add_row(cell, true);
    }
    pub fn add_row(&mut self, cell: &CellPath, is_down: bool) {
        let mut parent = cell.clone();
        let PathStep::Tree(_, y) = parent.path.pop().unwrap() else {
            panic!();
        };
        let parent = &mut self[&parent];
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            for _ in 0 .. *width {
                contents.insert((y + is_down as usize) * *width, Cell::Empty);
                // contents.insert(y * *width, Cell::Empty);
            }
            *height += 1;
        }
    }
    pub fn edit_symbol(&mut self, cell: &CellPath, new: &str) {
        let Cell::Symbol(s) = &mut self[cell] else {
            panic!();
        };
        *s = new.to_owned();
    }
    pub fn edit_variant(&mut self, cell: &CellPath, variant_to: usize, types: &Types) {
        let edit = &mut self[cell];
        let &mut Cell::Struct(StructVal { struct_id, .. }) = edit else {
            panic!();
        };
        *edit = types.default_struct_variant(struct_id, variant_to);
    }
    pub fn delete_row_above(&mut self, cell: &CellPath) {
        let mut parent = cell.clone();
        let PathStep::Tree(_, y) = parent.path.pop().unwrap() else {
            panic!();
        };
        if y == 0 { return; }
        let parent = &mut self[&parent];
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            if row_has_field(contents, *width, y - 1) {
                return; // a struct grid must keep all its field cells
            }
            for _ in 0 .. *width {
                contents.remove((y - 1) * *width);
            }
            *height -= 1;
        }
    }
    pub fn delete_row_below(&mut self, cell: &CellPath) {
        let mut parent = cell.clone();
        let PathStep::Tree(_, y) = parent.path.pop().unwrap() else {
            panic!();
        };
        let parent = &mut self[&parent];
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            if y == *height - 1 {
                return;
            }
            if row_has_field(contents, *width, y + 1) {
                return; // a struct grid must keep all its field cells
            }
            for _ in 0 .. *width {
                contents.remove((y + 1) * *width);
            }
            *height -= 1;
        }
    }
    pub fn delete_column_left(&mut self, cell: &CellPath) {
        let mut parent = cell.clone();
        let PathStep::Tree(x, _) = parent.path.pop().unwrap() else {
            panic!();
        };
        if x == 0 { return; }
        let parent = &mut self[&parent];
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            if col_has_field(contents, *width, *height, x - 1) {
                return; // a struct grid must keep all its field cells
            }
            for i in (0 .. *height).map(|i| i * *width + x - 1).rev() {
                contents.remove(i);
            }
            *width -= 1;
        }
    }
    pub fn delete_column_right(&mut self, cell: &CellPath) {
        let mut parent = cell.clone();
        let PathStep::Tree(x, _) = parent.path.pop().unwrap() else {
            panic!();
        };
        let parent = &mut self[&parent];
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            if x == *width - 1 { return; }
            if col_has_field(contents, *width, *height, x + 1) {
                return; // a struct grid must keep all its field cells
            }
            for i in (0 .. *height).map(|i| i * *width + x + 1).rev() {
                contents.remove(i);
            }
            *width -= 1;
        }
    }
}
impl Index<Region> for Document {
    type Output = Cell;
    fn index(&self, index: Region) -> &Self::Output {
        match index {
            Region::Rim => &self.rim,
            Region::Ram => &self.ram,
            Region::Rom => &self.rom,
        }
    }
}
impl IndexMut<Region> for Document {
    fn index_mut(&mut self, index: Region) -> &mut Self::Output {
        match index {
            Region::Rim => &mut self.rim,
            Region::Ram => &mut self.ram,
            Region::Rom => &mut self.rom,
        }
    }
}

impl Index<&PathStep> for Cell {
    type Output = Cell;
    fn index(&self, step: &PathStep) -> &Self {
        match (self, step) {
            (Cell::Field(field), PathStep::IntoField) => &field.value,
            (Cell::Struct(stru), PathStep::IntoStruct) => stru.grid.as_ref().unwrap(),
            (Cell::Tree(tree), &PathStep::Tree(x, y)) => &tree[(x, y)],
            _ => panic!(),
        }
    }
}
impl IndexMut<&PathStep> for Cell {
    fn index_mut(&mut self, step: &PathStep) -> &mut Self {
        match (self, step) {
            (Cell::Field(field), PathStep::IntoField) => &mut field.value,
            (Cell::Struct(stru), PathStep::IntoStruct) => stru.grid.as_mut().unwrap(),
            (Cell::Tree(tree), &PathStep::Tree(x, y)) => &mut tree[(x, y)],
            _ => panic!(),
        }
    }
}

impl Index<(usize, usize)> for Tree {
    type Output = Cell;
    fn index(&self, (x, y): (usize, usize)) -> &Self::Output {
        &self.contents[self.width * y + x]
    }
}
impl IndexMut<(usize, usize)> for Tree {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        &mut self.contents[self.width * y + x]
    }
}

// ===========================================================================
// Validity checking
// ===========================================================================
//
// `copy` and `mova` apply their change to a trial clone of the document and
// keep it only if the result is still well-formed. Encoding legality as "the
// result is a valid document" covers every case uniformly: copying a field
// cell loose into a free tree, copying a mistyped value into a field, moving a
// field cell into a different struct's grid, overwriting a field cell - each
// leaves the document invalid and so is rejected.

/// Whether a path ends by stepping into a tree cell (so the cell may be `Empty`).
fn ends_in_tree(path: &CellPath) -> bool {
    matches!(path.path.last(), Some(PathStep::Tree(..)))
}

/// Whether row `r` of a `width`-wide grid contains a field cell.
fn row_has_field(contents: &[Cell], width: usize, r: usize) -> bool {
    (0..width).any(|x| matches!(contents[r * width + x], Cell::Field(_)))
}

/// Whether column `c` of a `width`-wide, `height`-tall grid contains a field cell.
fn col_has_field(contents: &[Cell], width: usize, height: usize, c: usize) -> bool {
    (0..height).any(|y| matches!(contents[y * width + c], Cell::Field(_)))
}

/// Whether the whole document is structurally well-formed and type-correct.
pub fn is_valid(doc: &Document, types: &Types) -> bool {
    valid_field_value(&doc.rim, &types.rim, types)
        && valid_region_tree(&doc.ram, types)
        && valid_region_tree(&doc.rom, types)
}

/// A region root: a tree of free (untyped) cells.
fn valid_region_tree(cell: &Cell, types: &Types) -> bool {
    match cell {
        Cell::Tree(t) => valid_shape(t) && t.contents.iter().all(|c| valid_free_cell(c, types)),
        _ => false,
    }
}

/// A tree's `contents` length must match its declared dimensions.
fn valid_shape(t: &Tree) -> bool {
    t.contents.len() == t.width * t.height
}

/// A cell living directly in a free (untyped) tree - Ram, Rom, or a tree
/// nested inside one. Anything is allowed except a loose `Field` cell, which
/// may only ever live inside a struct's grid.
fn valid_free_cell(cell: &Cell, types: &Types) -> bool {
    match cell {
        Cell::Empty | Cell::Symbol(_) => true,
        Cell::Struct(sv) => valid_struct(sv, types),
        Cell::Tree(t) => valid_shape(t) && t.contents.iter().all(|c| valid_free_cell(c, types)),
        Cell::Field(_) => false,
    }
}

/// The value held by a field, or by the Rim root, checked against its `FieldDef`.
fn valid_field_value(cell: &Cell, def: &FieldDef, types: &Types) -> bool {
    if def.is_tree {
        match cell {
            Cell::Tree(t) => {
                valid_shape(t) && t.contents.iter().all(|c| valid_element(c, def.value, types))
            }
            _ => false,
        }
    } else {
        valid_typed(cell, def.value, types)
    }
}

/// An element of a typed tree: a blank cell, or a value of the element type.
fn valid_element(cell: &Cell, value: CellValue, types: &Types) -> bool {
    cell.is_empty() || valid_typed(cell, value, types)
}

/// A cell that must be exactly the given non-tree, non-empty value type.
fn valid_typed(cell: &Cell, value: CellValue, types: &Types) -> bool {
    match (cell, value) {
        (Cell::Symbol(_), CellValue::Symbol) => true,
        (Cell::Struct(sv), CellValue::Struct(id)) => sv.struct_id == id && valid_struct(sv, types),
        _ => false,
    }
}

/// A struct: ids in range, and its grid holds exactly one correctly-tagged
/// `Field` cell per field of the variant (every other cell `Empty`), or is
/// `None` for a fieldless variant.
fn valid_struct(sv: &StructVal, types: &Types) -> bool {
    let Some(def) = types.types.get(sv.struct_id) else {
        return false;
    };
    let Some(variant) = def.variants.get(sv.variant_id) else {
        return false;
    };
    let field_count = variant.fields.len();

    let Some(grid) = &sv.grid else {
        return field_count == 0; // a fieldless variant has no grid
    };
    let Cell::Tree(t) = grid.as_ref() else {
        return false;
    };
    if field_count == 0 || !valid_shape(t) {
        return false;
    }

    // every field 0..field_count must appear exactly once, as a correctly
    // tagged Field cell; every other cell must be Empty
    let mut seen = vec![false; field_count];
    for c in &t.contents {
        match c {
            Cell::Empty => {}
            Cell::Field(fv) => {
                if fv.struct_id != sv.struct_id
                    || fv.variant_id != sv.variant_id
                    || fv.field_id >= field_count
                    || seen[fv.field_id]
                {
                    return false;
                }
                seen[fv.field_id] = true;
                if !valid_field_value(&fv.value, &variant.fields[fv.field_id], types) {
                    return false;
                }
            }
            _ => return false, // struct grids hold only Field/Empty cells
        }
    }
    seen.into_iter().all(|s| s)
}