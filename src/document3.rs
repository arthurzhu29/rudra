use std::{mem, ops::{Index, IndexMut}};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Document {
    pub rim_field: FieldDef,
    pub rim: Cell,
    pub ram: Cell,
    pub rom: Cell,
}


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Cell {
    Symbol(String),
    Struct(StructVal),
    Tree(Tree),
    #[default]
    Empty,
    Field(FieldVal),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldVal {
    pub struct_id: usize,
    pub variant_id: usize,
    pub field_id: usize,
    pub value: Box<Cell>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructVal {
    pub struct_id: usize,
    pub variant_id: usize,
    pub grid: Option<Box<Cell>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tree {
    pub contents: Vec<Cell>,
    pub width: usize,
    pub height: usize,
}


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellPath {
    pub region: Region,
    pub path: Vec<PathStep>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathStep {
    IntoStruct,
    IntoField,
    Tree(usize, usize),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Region {
    Rim,
    Ram,
    Rom,
}


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Types(pub Vec<StructDef>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariantDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldDef {
    pub name: String,
    pub value: CellValue,
    pub is_tree: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    fn symbol() -> Self {
        Self::Symbol(String::new())
    }
}

impl Types {
    pub fn default_struct_variant(&self, struct_id: usize, variant_id: usize) -> Cell {
        Cell::Struct(StructVal {
            struct_id,
            variant_id: 0,
            grid: {
                let contents = self.0[struct_id].variants[variant_id].fields.iter().enumerate().map(|(field_id, fd)| Cell::Field(FieldVal { struct_id, variant_id, field_id, value: Box::new(fd.default(self)) })).collect::<Vec<_>>();
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
    pub fn new(types: &Types, rim_field: FieldDef) -> Self {
        Document {
            rim: rim_field.default(types),
            rim_field,
            ram: Tree::any_default(),
            rom: Cell::Tree({
                let width = types.0.iter().map(|sd| sd.variants.len()).max().unwrap_or(1);
                let height = types.0.len() + 2;
                let mut contents = Vec::with_capacity(width * height);
                for (struct_id, sd) in types.0.iter().enumerate() {
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
    pub fn copy(&mut self, dest: &CellPath, src: &CellPath) {
        self[dest] = self[src].clone();
    }
    // only eligible for cells within trees, as only they may be empty.
    pub fn mova(&mut self, src: &CellPath, dest: &CellPath) {
        self[dest] = mem::take(&mut self[src]);
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