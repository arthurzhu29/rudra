
impl CellValue {
    pub fn default(&self) -> Cell {
        match self {
            Self::Symbol => Cell::Symbol(String::new()),
            Self::Struct(n) => Cell::Struct(StructVal {
                id: *n,
                variant: 0,
                fields: vec![],
            }),
        }
    }
}

impl Document {
    pub fn new(types: &[StructDef], rim_field: FieldDef) -> Self {
        let mut contents = Vec::new();

        for i in 0..types.len() {
            contents.push(Cell::Struct(StructVal {
                id: i,
                variant: 0,
                fields: vec![],
            }));
        }

        contents.push(Cell::Symbol("".to_string()));

        contents.push(Cell::Tree(Tree {
            contents: vec![Cell::Empty],
            width: 1,
            height: 1,
        }));

        let height = contents.len();

        let rom = Cell::Tree(Tree {
            contents,
            width: 1,
            height,
        });

        let rim = if rim_field.is_tree {
            Cell::Tree(Tree {
                contents: vec![Cell::Empty],
                width: 1,
                height: 1,
            })
        } else {
            rim_field.value.default()
        };

        let ram = Cell::Tree(Tree {
            contents: vec![Cell::Empty],
            width: 1,
            height: 1,
        });

        Self {
            rim_field,
            rim,
            ram,
            rom,
        }
    }

    pub fn root(&self, reg: Region) -> &Cell {
        match reg {
            Region::Rim => &self.rim,
            Region::Ram => &self.ram,
            Region::Rom => &self.rom,
        }
    }

    pub fn root_mut(&mut self, reg: Region) -> &mut Cell {
        match reg {
            Region::Rim => &mut self.rim,
            Region::Ram => &mut self.ram,
            Region::Rom => &mut self.rom,
        }
    }

    pub fn resolve(&self, loc: &CellLocation) -> &Cell {
        let mut current = self.root(loc.region);

        for step in &loc.path {
            current = current.index(step);
        }

        current
    }
    pub fn resolve_mut(&mut self, loc: &CellLocation) -> &mut Cell {
        let mut current = self.root_mut(loc.region);

        for step in &loc.path {
            current = current.index_mut(step);
        }

        current
    }

    pub fn copy(&mut self, dest: &CellLocation, src: &CellLocation) {
        *self.resolve_mut(dest) = self.resolve(src).clone();
    }
    pub fn add_column_right(&mut self, cell: &CellLocation) {
        self.add_column(cell, true);
    }
    pub fn add_column_left(&mut self, cell: &CellLocation) {
        self.add_column(cell, false);
    }
    pub fn add_column(&mut self, cell: &CellLocation, is_right: bool) {
        let mut parent = cell.clone();
        let PathStep::Tree(x, _) = parent.path.pop().unwrap() else {
            panic!();
        };
        let parent = self.resolve_mut(&parent);
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            for i in (0 .. *height).map(|i| i * *width + x + is_right as usize).rev() {
                contents.insert(i, Cell::Empty);
            }
            *width += 1;
        }
    }
    pub fn add_row(&mut self, cell: &CellLocation, is_down: bool) {
        let mut parent = cell.clone();
        let PathStep::Tree(_, y) = parent.path.pop().unwrap() else {
            panic!();
        };
        let parent = self.resolve_mut(&parent);
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            for _ in 0 .. *width {
                contents.insert((y + is_down as usize) * *width, Cell::Empty);
                // contents.insert(y * *width, Cell::Empty);
            }
            *height += 1;
        }
    }
    pub fn add_row_above(&mut self, cell: &CellLocation) {
        self.add_row(cell, false);
    }
    pub fn add_row_below(&mut self, cell: &CellLocation) {
        self.add_row(cell, true);
    }
    pub fn edit_symbol(&mut self, cell: &CellLocation, new: &str) {
        let Cell::Symbol(s) = self.resolve_mut(cell) else {
            panic!();
        };
        *s = new.to_owned();
    }
    pub fn edit_variant(&mut self, cell: &CellLocation, variant_to: usize, types: &Types) {
        let Cell::Struct(StructVal { id, variant, fields }) = self.resolve_mut(cell) else {
            panic!();
        };
        let new_fields = types.0[*id].variants[variant_to].fields.iter().map(|field: &FieldDef| field.default()).collect::<Vec<_>>();
        *fields = new_fields;
        *variant = variant_to;
    }
    pub fn delete_row_above(&mut self, cell: &CellLocation) {
        let mut parent = cell.clone();
        let PathStep::Tree(_, y) = parent.path.pop().unwrap() else {
            panic!();
        };
        if y == 0 { return; }
        let parent = self.resolve_mut(&parent);
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            for _ in 0 .. *width {
                contents.remove((y - 1) * *width);
            }
            *height -= 1;
        }
    }
    pub fn delete_row_below(&mut self, cell: &CellLocation) {
        let mut parent = cell.clone();
        let PathStep::Tree(_, y) = parent.path.pop().unwrap() else {
            panic!();
        };
        let parent = self.resolve_mut(&parent);
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
    pub fn delete_column_left(&mut self, cell: &CellLocation) {
        let mut parent = cell.clone();
        let PathStep::Tree(x, _) = parent.path.pop().unwrap() else {
            panic!();
        };
        if x == 0 { return; }
        let parent = self.resolve_mut(&parent);
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            for i in (0 .. *height).map(|i| i * *width + x - 1).rev() {
                contents.remove(i);
            }
            *width -= 1;
        }
    }
    pub fn delete_column_right(&mut self, cell: &CellLocation) {
        let mut parent = cell.clone();
        let PathStep::Tree(x, _) = parent.path.pop().unwrap() else {
            panic!();
        };
        let parent = self.resolve_mut(&parent);
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            if x == *width - 1 { return; }
            for i in (0 .. *height).map(|i| i * *width + x + 1).rev() {
                contents.remove(i);
            }
            *width -= 1;
        }
    }
}

impl Cell {
    pub fn index(&self, path: &PathStep) -> &Self {
        match (self, path) {
            (
                Cell::Tree(Tree {
                    contents,
                    width,
                    height: _,
                }),
                PathStep::Tree(x, y),
            ) => &contents[y * width + x],

            (
                Cell::Struct(StructVal {
                    id: _,
                    variant: _,
                    fields,
                }),
                PathStep::Struct(id),
            ) => &fields[*id],

            _ => panic!(),
        }
    }
    pub fn index_mut(&mut self, path: &PathStep) -> &mut Self {
        match (self, path) {
            (
                Cell::Tree(Tree {
                    contents,
                    width,
                    height: _,
                }),
                PathStep::Tree(x, y),
            ) => &mut contents[*y * *width + x],

            (
                Cell::Struct(StructVal {
                    id: _,
                    variant: _,
                    fields,
                }),
                PathStep::Struct(id),
            ) => &mut fields[*id],

            _ => panic!(),
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Types(pub Vec<StructDef>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructDef {
    pub name: String,
    pub variants: Vec<StructVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructVariant {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Document {
    pub rim_field: FieldDef,
    pub rim: Cell,
    pub ram: Cell,
    pub rom: Cell,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellLocation {
    pub region: Region,
    pub path: Vec<PathStep>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathStep {
    Struct(usize),
    Tree(usize, usize),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Region {
    Rim,
    Ram,
    Rom,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cell {
    Symbol(String),
    Struct(StructVal),
    Tree(Tree),
    Empty,
}

impl Cell {
    pub fn tree(&self) -> &Tree {
        let Self::Tree(t) = self else { panic!(); };
        t
    }
    pub fn struct_val(&self) -> &StructVal {
        let Self::Struct(s) = self else { panic!(); };
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tree {
    pub contents: Vec<Cell>,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructVal {
    pub id: usize,
    pub variant: usize,
    pub fields: Vec<Cell>,
}

impl FieldDef {
    pub fn default(&self) -> Cell {
        if self.is_tree {
            Cell::Tree(Tree { contents: vec![Cell::Empty], width: 1, height: 1 })
        } else {
            self.value.default()
        }
    }
}