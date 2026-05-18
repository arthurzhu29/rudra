
impl CellValue {
    fn default(&self) -> Cell {
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
    fn new(types: &[StructDef], rim_field: FieldDef) -> Self {
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

    fn root(&self, reg: Region) -> &Cell {
        match reg {
            Region::Rim => &self.rim,
            Region::Ram => &self.ram,
            Region::Rom => &self.rom,
        }
    }

    fn root_mut(&mut self, reg: Region) -> &mut Cell {
        match reg {
            Region::Rim => &mut self.rim,
            Region::Ram => &mut self.ram,
            Region::Rom => &mut self.rom,
        }
    }

    fn resolve(&self, loc: &CellLocation) -> &Cell {
        let mut current = self.root(loc.region);

        for step in &loc.path {
            current = current.index(step);
        }

        current
    }
    fn resolve_mut(&mut self, loc: &CellLocation) -> &mut Cell {
        let mut current = self.root_mut(loc.region);

        for step in &loc.path {
            current = current.index_mut(step);
        }

        current
    }

    fn copy(&mut self, dest: &CellLocation, src: &CellLocation) {
        *self.resolve_mut(dest) = self.resolve(src).clone();
    }
    fn add_column_right(&mut self, cell: &CellLocation) {
        self.add_column(cell, true);
    }
    fn add_column_left(&mut self, cell: &CellLocation) {
        self.add_column(cell, false);
    }
    fn add_column(&mut self, cell: &CellLocation, is_right: bool) {
        let mut parent = cell.clone();
        let PathStep::Tree(x, y) = parent.path.pop().unwrap() else {
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
    fn add_row(&mut self, cell: &CellLocation, is_down: bool) {
        let mut parent = cell.clone();
        let PathStep::Tree(x, y) = parent.path.pop().unwrap() else {
            panic!();
        };
        let parent = self.resolve_mut(&parent);
        if let Cell::Tree(Tree { contents, width, height }) = parent {
            for _ in (0 .. *width) {
                contents.insert(y * *width, Cell::Empty);
            }
            *height += 1;
        }
    }
    fn add_row_above(&mut self, cell: &CellLocation) {
        self.add_row(cell, false);
    }
    fn add_row_below(&mut self, cell: &CellLocation) {
        self.add_row(cell, true);
    }
    fn edit_symbol(&mut self, cell: &CellLocation, new: &str) {
        let Cell::Symbol(s) = self.resolve_mut(cell) else {
            panic!();
        };
        *s = new.to_owned();
    }
    fn edit_variant(&mut self, cell: &CellLocation, variant_to: usize, types: &Types) {
        let Cell::Struct(StructVal { id, variant, fields }) = self.resolve_mut(cell) else {
            panic!();
        };
        let new_fields = types.0[*id].variants[variant_to].fields.iter().map(|field: &FieldDef| field.default()).collect::<Vec<_>>();
        *fields = new_fields;
        *variant = variant_to;
    }
}

impl Cell {
    fn index(&self, path: &PathStep) -> &Self {
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
    fn index_mut(&mut self, path: &PathStep) -> &mut Self {
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
struct Types(Vec<StructDef>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct StructDef {
    name: String,
    variants: Vec<StructVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct StructVariant {
    name: String,
    fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct FieldDef {
    name: String,
    value: CellValue,
    is_tree: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CellValue {
    Symbol,
    Struct(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Document {
    rim_field: FieldDef,
    rim: Cell,
    ram: Cell,
    rom: Cell,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CellLocation {
    region: Region,
    path: Vec<PathStep>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum PathStep {
    Struct(usize),
    Tree(usize, usize),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Region {
    Rim,
    Ram,
    Rom,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Cell {
    Symbol(String),
    Struct(StructVal),
    Tree(Tree),
    Empty,
}

impl Cell {
    fn tree(&self) -> &Tree {
        let Self::Tree(t) = self else { panic!(); };
        t
    }
    fn struct_val(&self) -> &StructVal {
        let Self::Struct(s) = self else { panic!(); };
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Tree {
    contents: Vec<Cell>,
    width: usize,
    height: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct StructVal {
    id: usize,
    variant: usize,
    fields: Vec<Cell>,
}

impl FieldDef {
    fn default(&self) -> Cell {
        if self.is_tree {
            Cell::Tree(Tree { contents: vec![Cell::Empty], width: 1, height: 1 })
        } else {
            self.value.default()
        }
    }
}