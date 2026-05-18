//! Rudra — the document core.
//!
//! This module is the semantic layer described in `impl-spec-v2.md` §2: the
//! schema and the three cell trees, plus flatten, validation, the integrity
//! check, and every operation. It is **plain data** and depends on no UI
//! framework — the Bevy view layer (impl spec §6+) is a separate, downstream
//! layer that holds a `Document` and rebuilds itself from it.
//!
//! Section references `§N` point into `impl-spec-v2.md`; `design §N` into
//! `spec-v4.md`.

use serde::{Deserialize, Serialize};

// ===========================================================================
// §4.1  The schema registry
// ===========================================================================

/// An index into `Schema::structs`. Every `StructId` in a live document
/// resolves — invariant 2, established by `check_integrity`.
pub type StructId = usize;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    pub structs: Vec<StructDef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StructDef {
    pub name: String,
    /// `variants[0]` is the nameless empty variant (design §3.2). Maintained,
    /// not structural — `check_integrity` confirms it.
    pub variants: Vec<VariantDef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariantDef {
    pub name: String, // empty string for variant 0
    pub fields: Vec<FieldDef>, // empty for variant 0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub elem: TypeRef,
    /// The depth-1 toggle (design §3.4). It is a `bool`, not a wrapper, which
    /// is the structural reason `Tree<Tree<T>>` is unexpressible.
    pub is_tree: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeRef {
    Symbol,
    Struct(StructId),
}

// ===========================================================================
// §4.2  The cell tree
// ===========================================================================

/// The content of a cell — two cases, never three. Emptiness is not a content
/// case (design §3.3); it is the absence of content at a grid slot, i.e. the
/// `None` of an `Option<CellContent>`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CellContent {
    /// A value: a `Symbol` or a `Struct`.
    Value(LeafValue),
    /// A tree: a rectangular grid of cells.
    Tree(Grid),
}

/// A rectangular, non-empty grid of cells.
///
/// Each cell is an `Option<CellContent>`: `None` is an empty cell, `Some` is a
/// cell holding content. Field cells (struct fields, the Rim root) are bare
/// `CellContent` instead — they cannot be empty (invariant 5, structural).
///
/// Fields are private so the rectangular/non-empty invariant (invariant 3) is
/// maintained: only `new`, `single`, and the breadth operations construct or
/// reshape a grid, and each keeps `cells.len() == width * height`,
/// `width >= 1`, `height >= 1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    cells: Vec<Option<CellContent>>, // row-major
    width: usize,
    height: usize,
}

impl Grid {
    /// Build a grid, asserting the invariant. (Deserialization bypasses this;
    /// `check_integrity` is what re-checks a loaded grid — §9.4.)
    pub fn new(cells: Vec<Option<CellContent>>, width: usize, height: usize) -> Self {
        assert!(width >= 1 && height >= 1, "a grid must be non-empty");
        assert_eq!(cells.len(), width * height, "a grid must be rectangular");
        Self { cells, width, height }
    }

    /// A 1x1 grid holding a single cell — the shape of a default tree.
    pub fn single(cell: Option<CellContent>) -> Self {
        Self { cells: vec![cell], width: 1, height: 1 }
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    pub fn cells(&self) -> &[Option<CellContent>] {
        &self.cells
    }

    pub fn get(&self, row: usize, col: usize) -> &Option<CellContent> {
        &self.cells[row * self.width + col]
    }
    pub fn get_mut(&mut self, row: usize, col: usize) -> &mut Option<CellContent> {
        let w = self.width;
        &mut self.cells[row * w + col]
    }

    // -- breadth operations (§8.2). The only operations that change w/h. ----

    fn insert_row(&mut self, at_row: usize) {
        let w = self.width;
        let start = at_row * w;
        for i in 0..w {
            self.cells.insert(start + i, None);
        }
        self.height += 1;
    }
    fn insert_col(&mut self, at_col: usize) {
        let (w, h) = (self.width, self.height);
        for r in (0..h).rev() {
            self.cells.insert(r * w + at_col, None);
        }
        self.width += 1;
    }
    fn delete_row(&mut self, row: usize) {
        let w = self.width;
        let start = row * w;
        for _ in 0..w {
            self.cells.remove(start);
        }
        self.height -= 1;
    }
    fn delete_col(&mut self, col: usize) {
        let (w, h) = (self.width, self.height);
        for r in (0..h).rev() {
            self.cells.remove(r * w + col);
        }
        self.width -= 1;
    }
}

// ===========================================================================
// §4.3  Leaf values and struct instances
// ===========================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LeafValue {
    Symbol(String),
    Struct(StructInstance),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StructInstance {
    pub struct_id: StructId,
    /// A plain `usize`, never `Option`: a struct is always at a variant,
    /// defaulting to 0 (design §3.2). Validity is invariant 7.
    pub variant: usize,
    /// One field cell per field of the chosen variant. Field cells are
    /// `CellContent`, never empty (invariant 5).
    pub fields: Vec<CellContent>,
}

// Note on variant 0: a variant-0 struct is the ordinary
// `StructInstance { variant: 0, fields: vec![] }`. It is built, rendered,
// validated, and integrity-checked through exactly the same code paths as any
// other variant — there are deliberately no `if variant == 0` shortcuts
// anywhere in this module (design §3.2).

// ===========================================================================
// §4.5  Addressing cells — CellLocation and slot resolution
// ===========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Region {
    Rom,
    Ram,
    Rim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathStep {
    Grid { row: usize, col: usize },
    Field { index: usize },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellLocation {
    pub region: Region,
    pub path: Vec<PathStep>,
}

impl CellLocation {
    pub fn root(region: Region) -> Self {
        Self { region, path: Vec::new() }
    }
}

/// A resolved cell: a place that holds content. `Field` slots (struct fields,
/// the region roots) are never empty; `Grid` slots (a tree's grid cells) may
/// be. `CellSlot` is `Copy` — it is just a shared reference.
#[derive(Clone, Copy)]
pub enum CellSlot<'a> {
    Field(&'a CellContent),
    Grid(&'a Option<CellContent>),
}

pub enum CellSlotMut<'a> {
    Field(&'a mut CellContent),
    Grid(&'a mut Option<CellContent>),
}

impl<'a> CellSlot<'a> {
    /// The content at this cell, or `None` if it is an empty grid cell. This
    /// is how a copy reads a source and how validation reads a candidate.
    pub fn content(&self) -> Option<&'a CellContent> {
        match self {
            CellSlot::Field(c) => Some(c),
            CellSlot::Grid(o) => o.as_ref(),
        }
    }
}

/// Walk a path from a root `CellContent`. A path that contradicts the data it
/// walks is a programming error (impl spec §4.5) and panics.
fn resolve_content<'a>(root: &'a CellContent, path: &[PathStep]) -> CellSlot<'a> {
    let mut place = CellSlot::Field(root);
    for step in path {
        let content: &'a CellContent = match place {
            CellSlot::Field(c) => c,
            CellSlot::Grid(o) => o.as_ref().expect("path descends through an empty cell"),
        };
        place = match (content, step) {
            (CellContent::Value(LeafValue::Struct(si)), PathStep::Field { index }) => {
                CellSlot::Field(&si.fields[*index])
            }
            (CellContent::Tree(grid), PathStep::Grid { row, col }) => {
                CellSlot::Grid(grid.get(*row, *col))
            }
            _ => panic!("malformed path: step does not match content"),
        };
    }
    place
}

fn resolve_content_mut<'a>(root: &'a mut CellContent, path: &[PathStep]) -> CellSlotMut<'a> {
    let mut place = CellSlotMut::Field(root);
    for step in path {
        let content: &'a mut CellContent = match place {
            CellSlotMut::Field(c) => c,
            CellSlotMut::Grid(o) => o.as_mut().expect("path descends through an empty cell"),
        };
        place = match (content, step) {
            (CellContent::Value(LeafValue::Struct(si)), PathStep::Field { index }) => {
                CellSlotMut::Field(&mut si.fields[*index])
            }
            (CellContent::Tree(grid), PathStep::Grid { row, col }) => {
                CellSlotMut::Grid(grid.get_mut(*row, *col))
            }
            _ => panic!("malformed path: step does not match content"),
        };
    }
    place
}

// ===========================================================================
// §4.6  The document
// ===========================================================================

/// Per-region "needs a view rebuild" flags (impl spec §2.3). Lives only on a
/// live `Document`, never serialized.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RegionMask {
    pub rom: bool,
    pub ram: bool,
    pub rim: bool,
}

impl RegionMask {
    pub fn all() -> Self {
        Self { rom: true, ram: true, rim: true }
    }
    pub fn mark(&mut self, region: Region) {
        match region {
            Region::Rom => self.rom = true,
            Region::Ram => self.ram = true,
            Region::Rim => self.rim = true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    /// The one highlighted Rom cell (invariant 9).
    pub rom: CellLocation,
    /// The one highlighted Rim cell (invariant 9).
    pub rim: CellLocation,
    /// The highlighted Ram cells — zero or more (invariant 9).
    pub ram: Vec<CellLocation>,
    /// The single superhighlighted cell, in any region (invariant 8).
    pub superhighlighted: CellLocation,
    /// Destinations of the last failed copy — transient, cleared on the next
    /// selection (design §6.3).
    pub red: Vec<CellLocation>,
}

/// The whole document. Plain data — the Bevy layer holds this inside a
/// `Resource`; nothing here depends on Bevy. Not serialized directly; the
/// save/undo payload is `DocumentSnapshot`.
#[derive(Clone, Debug)]
pub struct Document {
    pub schema: Schema,
    /// Types the Rim root cell (impl spec §9.1) — an ordinary `FieldDef`
    /// attached to the document rather than to a struct.
    pub rim_root_field: FieldDef,
    pub rom: CellContent, // always Tree — the read-only palette
    pub ram: CellContent, // always Tree — untyped scratch
    pub rim: CellContent, // conforms to rim_root_field
    pub selection: Selection,
    pub dirty: RegionMask,
}

/// The save / undo payload (impl spec §8.4, §9.5). `rom` is omitted — it is
/// rebuilt from `schema` — and `dirty` is omitted — a restore marks all.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub schema: Schema,
    pub rim_root_field: FieldDef,
    pub ram: CellContent,
    pub rim: CellContent,
    pub selection: Selection,
}

// ---- defaults (§4.6) ------------------------------------------------------

/// The default value of a value type: an empty-string `Symbol`, or a
/// variant-0 `Struct`.
pub fn default_value(elem: TypeRef) -> CellContent {
    match elem {
        TypeRef::Symbol => CellContent::Value(LeafValue::Symbol(String::new())),
        TypeRef::Struct(id) => CellContent::Value(LeafValue::Struct(StructInstance {
            struct_id: id,
            variant: 0,
            fields: Vec::new(),
        })),
    }
}

/// The default tree: a 1x1 grid holding one empty cell.
pub fn default_tree() -> CellContent {
    CellContent::Tree(Grid::single(None))
}

/// The default content for a field: a default tree if `is_tree`, else a
/// default value.
pub fn default_content(field: &FieldDef) -> CellContent {
    if field.is_tree {
        default_tree()
    } else {
        default_value(field.elem)
    }
}

/// Build the Rom palette from a schema (impl spec §4.6): a 1xN column holding
/// the variant-0 instance of every struct, an empty Symbol, and a default
/// tree.
fn build_rom(schema: &Schema) -> CellContent {
    let mut cells: Vec<Option<CellContent>> = Vec::new();
    for id in 0..schema.structs.len() {
        cells.push(Some(CellContent::Value(LeafValue::Struct(StructInstance {
            struct_id: id,
            variant: 0,
            fields: Vec::new(),
        }))));
    }
    cells.push(Some(CellContent::Value(LeafValue::Symbol(String::new()))));
    cells.push(Some(default_tree()));
    let h = cells.len();
    CellContent::Tree(Grid::new(cells, 1, h))
}

impl Document {
    /// Create a new document: Rom from the schema, Ram a default tree, Rim the
    /// default of the chosen root field, and the starting selection of design
    /// §6.4.
    pub fn new(schema: Schema, rim_root_field: FieldDef) -> Self {
        let rom = build_rom(&schema);
        let ram = default_tree();
        let rim = default_content(&rim_root_field);
        let rom_root = CellLocation::root(Region::Rom);
        let selection = Selection {
            rom: rom_root.clone(),
            rim: CellLocation::root(Region::Rim),
            ram: Vec::new(),
            superhighlighted: rom_root,
            red: Vec::new(),
        };
        Document { schema, rim_root_field, rom, ram, rim, selection, dirty: RegionMask::all() }
    }

    /// Capture a snapshot for undo or save (impl spec §8.4).
    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            schema: self.schema.clone(),
            rim_root_field: self.rim_root_field.clone(),
            ram: self.ram.clone(),
            rim: self.rim.clone(),
            selection: self.selection.clone(),
        }
    }

    /// Reconstruct a document from a snapshot. `rom` is rebuilt from `schema`;
    /// every region is marked dirty. The caller must run `check_integrity` on
    /// the result before treating it as live (impl spec §9.4).
    pub fn from_snapshot(snap: DocumentSnapshot) -> Self {
        let rom = build_rom(&snap.schema);
        Document {
            schema: snap.schema,
            rim_root_field: snap.rim_root_field,
            rom,
            ram: snap.ram,
            rim: snap.rim,
            selection: snap.selection,
            dirty: RegionMask::all(),
        }
    }

    fn region_root(&self, region: Region) -> &CellContent {
        match region {
            Region::Rom => &self.rom,
            Region::Ram => &self.ram,
            Region::Rim => &self.rim,
        }
    }
    fn region_root_mut(&mut self, region: Region) -> &mut CellContent {
        match region {
            Region::Rom => &mut self.rom,
            Region::Ram => &mut self.ram,
            Region::Rim => &mut self.rim,
        }
    }

    pub fn resolve(&self, loc: &CellLocation) -> CellSlot<'_> {
        resolve_content(self.region_root(loc.region), &loc.path)
    }
    pub fn resolve_mut(&mut self, loc: &CellLocation) -> CellSlotMut<'_> {
        let region = loc.region;
        resolve_content_mut(self.region_root_mut(region), &loc.path)
    }

    // -- §9.1  Governance ---------------------------------------------------

    /// The governance of the cell at `loc` — what, if anything, types it
    /// (impl spec §9.1). Walks the path carrying both the content and the
    /// governance.
    pub fn governance(&self, loc: &CellLocation) -> Governance {
        let mut gov = match loc.region {
            Region::Rim => Governance::Field(self.rim_root_field.clone()),
            Region::Rom | Region::Ram => Governance::Ungoverned,
        };
        let mut place = CellSlot::Field(self.region_root(loc.region));
        for step in &loc.path {
            let content = place.content().expect("path descends through an empty cell");
            match step {
                PathStep::Field { index } => {
                    let si = match content {
                        CellContent::Value(LeafValue::Struct(si)) => si,
                        _ => panic!("malformed path: Field step into non-struct"),
                    };
                    let fd = self.schema.structs[si.struct_id].variants[si.variant].fields[*index]
                        .clone();
                    place = CellSlot::Field(&si.fields[*index]);
                    gov = Governance::Field(fd);
                }
                PathStep::Grid { row, col } => {
                    let grid = match content {
                        CellContent::Tree(g) => g,
                        _ => panic!("malformed path: Grid step into non-tree"),
                    };
                    place = CellSlot::Grid(grid.get(*row, *col));
                    gov = match gov {
                        Governance::Field(fd) if fd.is_tree => {
                            Governance::GovernedGridCell(fd.elem)
                        }
                        Governance::Field(_) => {
                            panic!("malformed path: Grid step into a non-tree field")
                        }
                        Governance::GovernedGridCell(elem) => {
                            Governance::GovernedGridCell(elem)
                        }
                        Governance::Ungoverned => Governance::Ungoverned,
                    };
                }
            }
        }
        gov
    }

    // -- §7  Selection ------------------------------------------------------

    /// A selection click on `loc` (impl spec §7). Moves the superhighlight
    /// anchor to `loc`, updates the clicked region's highlight, clears `red`,
    /// and — for a Rom/Rim pick — clears Ram's highlights (the `spec-v4`
    /// amendment to design §6.1).
    pub fn select(&mut self, loc: CellLocation) {
        self.selection.red.clear();
        match loc.region {
            Region::Rom => {
                self.selection.rom = loc.clone();
                self.selection.ram.clear();
            },
            Region::Rim => {
                self.selection.rim = loc.clone();
                self.selection.ram.clear();
            },
            Region::Ram => {
                // A Ram click toggles this cell's highlight on or off.
                if let Some(pos) = self.selection.ram.iter().position(|l| *l == loc) {
                    self.selection.ram.remove(pos); // toggle off
                } else {
                    self.selection.ram.push(loc.clone()); // toggle on
                }
            },
        }
        // Every selection click moves the superhighlight anchor to the clicked
        // cell — in any region, and on a Ram toggle-off too. After a toggle-off
        // the cell is superhighlighted but no longer highlighted: the two
        // states come apart (design §6.2).
        self.selection.superhighlighted = loc;
        // A selection changes which cells render highlighted; rebuild the view.
        self.dirty = RegionMask::all();
    }
}

/// What types a destination cell — the result of walking its path (§9.1).
#[derive(Clone, Debug, PartialEq)]
pub enum Governance {
    /// A field cell: a struct field, or the Rim root.
    Field(FieldDef),
    /// A grid cell of a governed `Tree<elem>`.
    GovernedGridCell(TypeRef),
    /// A grid cell of an ungoverned tree (the Rom/Ram root trees).
    Ungoverned,
}

// ===========================================================================
// §5  Flatten and semantic equality
// ===========================================================================

/// The flatten of a content: its values, in grid-axis row-major order. Does
/// not descend the field axis — a struct is one element.
pub fn flatten<'a>(content: &'a CellContent, out: &mut Vec<&'a LeafValue>) {
    match content {
        CellContent::Value(v) => out.push(v),
        CellContent::Tree(grid) => {
            for cell in grid.cells() {
                if let Some(c) = cell {
                    flatten(c, out);
                }
            }
        }
    }
}

/// Semantic equality (design §4): equal flattens, with `LeafValue` equality
/// recursing the field axis. Distinct from the derived `PartialEq`, which is
/// layout-sensitive structural equality.
pub fn content_eq(a: &CellContent, b: &CellContent) -> bool {
    let (mut fa, mut fb) = (Vec::new(), Vec::new());
    flatten(a, &mut fa);
    flatten(b, &mut fb);
    fa.len() == fb.len() && fa.iter().zip(&fb).all(|(x, y)| value_eq(x, y))
}

pub fn value_eq(a: &LeafValue, b: &LeafValue) -> bool {
    match (a, b) {
        (LeafValue::Symbol(x), LeafValue::Symbol(y)) => x == y,
        (LeafValue::Struct(x), LeafValue::Struct(y)) => {
            x.struct_id == y.struct_id
                && x.variant == y.variant
                && x.fields.len() == y.fields.len()
                && x.fields.iter().zip(&y.fields).all(|(p, q)| content_eq(p, q))
        }
        _ => false,
    }
}

// ===========================================================================
// §9.2  Validation
// ===========================================================================

/// A copy of `payload` (the source's content, or `None` for an empty source)
/// into a destination with this governance is valid iff this returns `true`.
pub fn validates(payload: &Option<CellContent>, gov: &Governance) -> bool {
    match gov {
        Governance::Ungoverned => true,
        Governance::Field(fd) => match payload {
            Some(c) => conforms(c, fd),
            None => false, // a field cell rejects an empty payload (invariant 5)
        },
        Governance::GovernedGridCell(elem) => match payload {
            None => true, // a grid cell of a tree may be empty
            Some(CellContent::Value(v)) => value_ok(v, *elem),
            Some(CellContent::Tree(g)) => tree_ok(g, *elem),
        },
    }
}

/// Whether `content` is a valid value for the field `fd` — the §9.2 field
/// rule, and also the per-field check of `check_integrity` (invariant 6).
fn conforms(content: &CellContent, fd: &FieldDef) -> bool {
    match content {
        CellContent::Value(v) => !fd.is_tree && value_ok(v, fd.elem),
        CellContent::Tree(g) => fd.is_tree && tree_ok(g, fd.elem),
    }
}

/// Whether every grid-axis leaf of the tree has element kind `elem`.
fn tree_ok(g: &Grid, elem: TypeRef) -> bool {
    g.cells().iter().all(|cell| match cell {
        None => true, // empty contributes no leaf
        Some(CellContent::Value(v)) => value_ok(v, elem),
        Some(CellContent::Tree(sub)) => tree_ok(sub, elem),
    })
}

/// Whether a single value matches the element kind. Reaching a struct, this
/// confirms the identifier and stops — invariants 6/7 guarantee struct
/// internals are already well-typed (§9.3).
fn value_ok(v: &LeafValue, elem: TypeRef) -> bool {
    match (v, elem) {
        (LeafValue::Symbol(_), TypeRef::Symbol) => true,
        (LeafValue::Struct(si), TypeRef::Struct(id)) => si.struct_id == id,
        _ => false,
    }
}

// ===========================================================================
// §9.4  The integrity check
// ===========================================================================

/// Why a document failed `check_integrity`.
#[derive(Clone, Debug, PartialEq)]
pub struct IntegrityError(pub String);

/// Confirm the maintained invariants 2, 3, 6, 7 over a whole document
/// (invariant 5 is structural). A document that fails must be rejected and
/// never go live — this is what earns the rest of the system its right to
/// index the schema and trust struct internals (§9.3, §9.4).
pub fn check_integrity(doc: &Document) -> Result<(), IntegrityError> {
    let schema = &doc.schema;

    // -- schema well-formedness (invariant 2, and variant-0 of invariant 7) --
    for (sid, sd) in schema.structs.iter().enumerate() {
        if sd.variants.is_empty() {
            return Err(IntegrityError(format!("struct {sid} ('{}') has no variants", sd.name)));
        }
        if !sd.variants[0].fields.is_empty() {
            return Err(IntegrityError(format!(
                "struct {sid} ('{}'): variant 0 is not the empty variant",
                sd.name
            )));
        }
        for vd in &sd.variants {
            for fd in &vd.fields {
                check_typeref(fd.elem, schema)?;
            }
        }
    }
    check_typeref(doc.rim_root_field.elem, schema)?;

    // -- structural + struct-field-conformance walk of all three regions ----
    // (invariants 3, 7, and 6-for-struct-fields).
    verify_content(&doc.rom, schema)?;
    verify_content(&doc.ram, schema)?;
    verify_content(&doc.rim, schema)?;

    // -- invariant 6 at the Rim root -----------------------------------------
    if !conforms(&doc.rim, &doc.rim_root_field) {
        return Err(IntegrityError(
            "the Rim root does not conform to the root field".into(),
        ));
    }
    Ok(())
}

fn check_typeref(t: TypeRef, schema: &Schema) -> Result<(), IntegrityError> {
    match t {
        TypeRef::Symbol => Ok(()),
        TypeRef::Struct(id) => {
            if id < schema.structs.len() {
                Ok(())
            } else {
                Err(IntegrityError(format!("struct id {id} is out of range")))
            }
        }
    }
}

/// Recursively check one content tree: every grid rectangular and non-empty
/// (invariant 3), every struct instance well-formed (invariant 7) with each
/// of its field cells conforming to its `FieldDef` (invariant 6).
fn verify_content(content: &CellContent, schema: &Schema) -> Result<(), IntegrityError> {
    match content {
        CellContent::Value(LeafValue::Symbol(_)) => Ok(()),
        CellContent::Value(LeafValue::Struct(si)) => {
            let sd = schema.structs.get(si.struct_id).ok_or_else(|| {
                IntegrityError(format!("struct id {} is out of range", si.struct_id))
            })?;
            let vd = sd.variants.get(si.variant).ok_or_else(|| {
                IntegrityError(format!(
                    "variant {} is out of range for struct {}",
                    si.variant, si.struct_id
                ))
            })?;
            if si.fields.len() != vd.fields.len() {
                return Err(IntegrityError(format!(
                    "struct {} variant {}: has {} field cells, expected {}",
                    si.struct_id,
                    si.variant,
                    si.fields.len(),
                    vd.fields.len()
                )));
            }
            for (cell, fd) in si.fields.iter().zip(&vd.fields) {
                if !conforms(cell, fd) {
                    return Err(IntegrityError(format!(
                        "field '{}' of struct {} does not conform to its declared type",
                        fd.name, si.struct_id
                    )));
                }
                verify_content(cell, schema)?;
            }
            Ok(())
        }
        CellContent::Tree(grid) => {
            if grid.width < 1 || grid.height < 1 {
                return Err(IntegrityError("a grid is empty".into()));
            }
            if grid.cells.len() != grid.width * grid.height {
                return Err(IntegrityError(format!(
                    "a grid is not rectangular: {} cells, expected {}x{}",
                    grid.cells.len(),
                    grid.width,
                    grid.height
                )));
            }
            for cell in &grid.cells {
                if let Some(c) = cell {
                    verify_content(c, schema)?;
                }
            }
            Ok(())
        }
    }
}

// ===========================================================================
// §8  Operations
// ===========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VSide {
    Above,
    Below,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HSide {
    Left,
    Right,
}

#[derive(Clone, Debug)]
pub enum Operation {
    /// The destination(s) are read from the selection; `source` is the cell
    /// the user clicked while in copy mode.
    Copy { source: CellLocation },
    AddRow { at: CellLocation, side: VSide },
    AddColumn { at: CellLocation, side: HSide },
    DeleteRow { at: CellLocation, side: VSide },
    DeleteColumn { at: CellLocation, side: HSide },
    SelectVariant { at: CellLocation, variant: usize },
    EditSymbol { at: CellLocation, text: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The document's content changed. The caller snapshots for undo (§8.4)
    /// and rebuilds the dirtied region(s).
    Applied,
    /// The operation was coherent but a semantic check refused it — at present
    /// only a copy whose source fails validation against its destination
    /// (§9.2). No content changed, but `selection.red` was set, so the caller
    /// still rebuilds (to show the red) and does *not* snapshot. This is the
    /// only non-`Applied` outcome that has a side effect.
    Rejected,
    /// The operation was coherent but vacuous — it legitimately had nothing to
    /// do (e.g. a copy with no destination selected, or a breadth operation
    /// with no neighbouring row/column to act on). Nothing changed at all and
    /// the caller does nothing. The line against `Rejected`: a `NoOp` asked
    /// for nothing, a `Rejected` asked for something and was turned down.
    NoOp,
}

/// Apply an operation. The single entry point for every document mutation
/// (impl spec §6). The caller (the Bevy layer) pushes an undo snapshot before
/// any call that returns `Applied`.
pub fn apply(doc: &mut Document, op: Operation) -> Outcome {
    // Rom is read-only (invariant 1): reject any operation that targets it for
    // mutation. (A Copy's *source* may be in Rom — that only reads.)
    let mutation_target = match &op {
        Operation::Copy { .. } => None,
        Operation::AddRow { at, .. }
        | Operation::AddColumn { at, .. }
        | Operation::DeleteRow { at, .. }
        | Operation::DeleteColumn { at, .. }
        | Operation::SelectVariant { at, .. }
        | Operation::EditSymbol { at, .. } => Some(at.region),
    };
    if mutation_target == Some(Region::Rom) {
        return Outcome::NoOp;
    }

    match op {
        Operation::Copy { source } => apply_copy(doc, &source),
        Operation::AddRow { at, side } => apply_add_row(doc, &at, side),
        Operation::AddColumn { at, side } => apply_add_column(doc, &at, side),
        Operation::DeleteRow { at, side } => apply_delete_row(doc, &at, side),
        Operation::DeleteColumn { at, side } => apply_delete_column(doc, &at, side),
        Operation::SelectVariant { at, variant } => apply_select_variant(doc, &at, variant),
        Operation::EditSymbol { at, text } => apply_edit_symbol(doc, &at, text),
    }
}

// -- §8.1  Copy -------------------------------------------------------------

fn apply_copy(doc: &mut Document, source: &CellLocation) -> Outcome {
    // 1. Destinations, keyed on the superhighlighted cell's region.
    let dest_region = doc.selection.superhighlighted.region;
    let dests: Vec<CellLocation> = match dest_region {
        Region::Rim => vec![doc.selection.rim.clone()],
        Region::Ram => doc.selection.ram.clone(),
        Region::Rom => Vec::new(), // Rom is read-only — no destination
    };
    if dests.is_empty() {
        return Outcome::NoOp;
    }

    // 2. Payload — the source's content, or None for an empty source cell.
    let payload: Option<CellContent> = doc.resolve(source).content().cloned();

    // 3. Validate every destination — all-or-nothing.
    for d in &dests {
        let gov = doc.governance(d);
        if !validates(&payload, &gov) {
            doc.selection.red = dests.clone();
            doc.dirty = RegionMask::all(); // red must render
            return Outcome::Rejected;
        }
    }

    // 4. Write a fresh clone of the payload into every destination.
    for d in &dests {
        match doc.resolve_mut(d) {
            CellSlotMut::Field(c) => {
                // payload is Some — a field destination with None failed step 3.
                *c = payload.clone().expect("a validated field destination");
            }
            CellSlotMut::Grid(o) => {
                *o = payload.clone();
            }
        }
    }

    // 5. Update the selection (design §6.2).
    doc.selection.red.clear();
    if dest_region == Region::Ram {
        doc.selection.ram.clear();
    }
    // 6. Mark the destination region dirty. Copy never mutates the source.
    doc.dirty.mark(dest_region);
    Outcome::Applied
}

// -- §8.2  Breadth operations ----------------------------------------------

impl Document {
    /// The grid that *contains* the cell at `at`, plus that cell's (row, col)
    /// within it. `at.path` must end in a `Grid` step.
    fn enclosing_grid_mut(
        &mut self,
        at: &CellLocation,
    ) -> Option<(&mut Grid, usize, usize)> {
        let (last, parent) = at.path.split_last()?;
        let (row, col) = match *last {
            PathStep::Grid { row, col } => (row, col),
            PathStep::Field { .. } => return None,
        };
        let parent_loc = CellLocation { region: at.region, path: parent.to_vec() };
        let content = match self.resolve_mut(&parent_loc) {
            CellSlotMut::Field(c) => c,
            CellSlotMut::Grid(Some(c)) => c,
            CellSlotMut::Grid(None) => return None,
        };
        match content {
            CellContent::Tree(g) => Some((g, row, col)),
            _ => None,
        }
    }
}

fn apply_add_row(doc: &mut Document, at: &CellLocation, side: VSide) -> Outcome {
    let applied = match doc.enclosing_grid_mut(at) {
        Some((g, row, _col)) => {
            let idx = match side {
                VSide::Above => row,
                VSide::Below => row + 1,
            };
            g.insert_row(idx);
            true
        }
        None => false,
    };
    finish_breadth(doc, at, applied)
}

fn apply_add_column(doc: &mut Document, at: &CellLocation, side: HSide) -> Outcome {
    let applied = match doc.enclosing_grid_mut(at) {
        Some((g, _row, col)) => {
            let idx = match side {
                HSide::Left => col,
                HSide::Right => col + 1,
            };
            g.insert_col(idx);
            true
        }
        None => false,
    };
    finish_breadth(doc, at, applied)
}

fn apply_delete_row(doc: &mut Document, at: &CellLocation, side: VSide) -> Outcome {
    // A cell cannot delete its own row — only a neighbouring one. This is what
    // keeps a grid >= 1 row (invariant 3).
    let applied = match doc.enclosing_grid_mut(at) {
        Some((g, row, _col)) => {
            let target = match side {
                VSide::Above if row > 0 => Some(row - 1),
                VSide::Below if row + 1 < g.height => Some(row + 1),
                _ => None,
            };
            match target {
                Some(t) => {
                    g.delete_row(t);
                    true
                }
                None => false,
            }
        }
        None => false,
    };
    finish_breadth(doc, at, applied)
}

fn apply_delete_column(doc: &mut Document, at: &CellLocation, side: HSide) -> Outcome {
    let applied = match doc.enclosing_grid_mut(at) {
        Some((g, _row, col)) => {
            let target = match side {
                HSide::Left if col > 0 => Some(col - 1),
                HSide::Right if col + 1 < g.width => Some(col + 1),
                _ => None,
            };
            match target {
                Some(t) => {
                    g.delete_col(t);
                    true
                }
                None => false,
            }
        }
        None => false,
    };
    finish_breadth(doc, at, applied)
}

fn finish_breadth(doc: &mut Document, at: &CellLocation, applied: bool) -> Outcome {
    if applied {
        doc.dirty.mark(at.region);
        Outcome::Applied
    } else {
        Outcome::NoOp
    }
}

// -- §8.3  Variant selection and symbol editing ----------------------------

fn apply_select_variant(doc: &mut Document, at: &CellLocation, variant: usize) -> Outcome {
    // Read the struct id (immutable), then look up the target variant's
    // fields, then mutate — three steps to keep the borrows disjoint.
    let struct_id = match doc.resolve(at).content() {
        Some(CellContent::Value(LeafValue::Struct(si))) => si.struct_id,
        _ => return Outcome::NoOp,
    };
    let variant_def = match doc
        .schema
        .structs
        .get(struct_id)
        .and_then(|sd| sd.variants.get(variant))
    {
        Some(vd) => vd,
        None => return Outcome::NoOp, // variant out of range
    };
    let new_fields: Vec<CellContent> =
        variant_def.fields.iter().map(default_content).collect();

    // should panic instead of defaulting to NoOp.
    match doc.resolve_mut(at) {
        CellSlotMut::Field(CellContent::Value(LeafValue::Struct(si)))
        | CellSlotMut::Grid(Some(CellContent::Value(LeafValue::Struct(si)))) => {
            si.variant = variant;
            si.fields = new_fields;
        }
        _ => return Outcome::NoOp,
    }
    doc.dirty.mark(at.region);
    Outcome::Applied
}

fn apply_edit_symbol(doc: &mut Document, at: &CellLocation, text: String) -> Outcome {
    match doc.resolve_mut(at) {
        CellSlotMut::Field(CellContent::Value(LeafValue::Symbol(s)))
        | CellSlotMut::Grid(Some(CellContent::Value(LeafValue::Symbol(s)))) => {
            *s = text;
        }
        _ => return Outcome::NoOp,
    }
    doc.dirty.mark(at.region);
    Outcome::Applied
}

// ===========================================================================
// Undo / redo (impl spec §8.4) — pure history over snapshots.
// ===========================================================================

#[derive(Default)]
pub struct History {
    undo: Vec<DocumentSnapshot>,
    redo: Vec<DocumentSnapshot>,
}

impl History {
    /// Call before an operation that will return `Outcome::Applied`.
    pub fn record(&mut self, doc: &Document) {
        self.undo.push(doc.snapshot());
        self.redo.clear();
    }
    /// Restore the previous state, if any. Returns the document to install.
    pub fn undo(&mut self, current: &Document) -> Option<Document> {
        let snap = self.undo.pop()?;
        self.redo.push(current.snapshot());
        Some(Document::from_snapshot(snap))
    }
    pub fn redo(&mut self, current: &Document) -> Option<Document> {
        let snap = self.redo.pop()?;
        self.undo.push(current.snapshot());
        Some(Document::from_snapshot(snap))
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // struct 0 "Point": variant 0 "" (empty), variant 1 "xy" {x: Symbol, y: Symbol}
    // struct 1 "List":  variant 0 "" (empty), variant 1 "items" {items: Tree<Symbol>}
    fn sample_schema() -> Schema {
        let sym = |name: &str| FieldDef { name: name.into(), elem: TypeRef::Symbol, is_tree: false };
        Schema {
            structs: vec![
                StructDef {
                    name: "Point".into(),
                    variants: vec![
                        VariantDef { name: String::new(), fields: vec![] },
                        VariantDef {
                            name: "xy".into(),
                            fields: vec![sym("x"), sym("y")],
                        },
                    ],
                },
                StructDef {
                    name: "List".into(),
                    variants: vec![
                        VariantDef { name: String::new(), fields: vec![] },
                        VariantDef {
                            name: "items".into(),
                            fields: vec![FieldDef {
                                name: "items".into(),
                                elem: TypeRef::Symbol,
                                is_tree: true,
                            }],
                        },
                    ],
                },
            ],
        }
    }

    // a Rim whose root field is a single Point
    fn point_doc() -> Document {
        Document::new(
            sample_schema(),
            FieldDef { name: "root".into(), elem: TypeRef::Struct(0), is_tree: false },
        )
    }

    // a Rim whose root field is a Tree<Symbol>
    fn tree_doc() -> Document {
        Document::new(
            sample_schema(),
            FieldDef { name: "root".into(), elem: TypeRef::Symbol, is_tree: true },
        )
    }

    fn sym(s: &str) -> CellContent {
        CellContent::Value(LeafValue::Symbol(s.into()))
    }

    #[test]
    fn new_document_passes_integrity() {
        assert_eq!(check_integrity(&point_doc()), Ok(()));
        assert_eq!(check_integrity(&tree_doc()), Ok(()));
    }

    #[test]
    fn new_document_has_the_starting_selection() {
        // design §6.4: Rom root highlighted+super, Rim root highlighted, Ram empty.
        let d = point_doc();
        assert_eq!(d.selection.rom, CellLocation::root(Region::Rom));
        assert_eq!(d.selection.rim, CellLocation::root(Region::Rim));
        assert!(d.selection.ram.is_empty());
        assert_eq!(d.selection.superhighlighted, CellLocation::root(Region::Rom));
    }

    #[test]
    fn rom_holds_the_palette() {
        let d = point_doc();
        // 2 structs + 1 symbol + 1 default tree == 4 cells.
        if let CellContent::Tree(g) = &d.rom {
            assert_eq!(g.cells().len(), 4);
            assert_eq!((g.width(), g.height()), (1, 4));
        } else {
            panic!("Rom root is not a tree");
        }
    }

    #[test]
    fn flatten_walks_the_grid_axis_only() {
        // a 1x3 tree: ["a", empty, "b"]  -> flattens to [a, b]
        let g = Grid::new(vec![Some(sym("a")), None, Some(sym("b"))], 3, 1);
        let tree = CellContent::Tree(g);
        let mut out = Vec::new();
        flatten(&tree, &mut out);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn semantic_equality_ignores_layout() {
        // a bare value and a 1x1 tree holding it are semantically equal...
        let bare = sym("hi");
        let wrapped = CellContent::Tree(Grid::single(Some(sym("hi"))));
        assert!(content_eq(&bare, &wrapped));
        // ...but structurally (the derived PartialEq) distinct.
        assert_ne!(bare, wrapped);
    }

    #[test]
    fn governance_of_roots() {
        let d = point_doc();
        // Rim root is governed by the root field.
        assert!(matches!(
            d.governance(&CellLocation::root(Region::Rim)),
            Governance::Field(_)
        ));
        // Ram and Rom roots are ungoverned trees.
        assert_eq!(d.governance(&CellLocation::root(Region::Ram)), Governance::Ungoverned);
        assert_eq!(d.governance(&CellLocation::root(Region::Rom)), Governance::Ungoverned);
    }

    #[test]
    fn copy_into_ungoverned_ram_cell_always_succeeds() {
        let mut d = tree_doc();
        // select the Ram root cell as the (single) destination
        d.select(CellLocation::root(Region::Ram));
        // source: the Rom symbol cell (index 2 of Rom's column: structs 0,1 then symbol)
        let src = CellLocation { region: Region::Rom, path: vec![PathStep::Grid { row: 2, col: 0 }] };
        assert_eq!(apply(&mut d, Operation::Copy { source: src }), Outcome::Applied);
        // Ram root is now a Symbol (a tree was overwritten by a value — fine, ungoverned).
        assert!(matches!(d.ram, CellContent::Value(LeafValue::Symbol(_))));
    }

    #[test]
    fn copy_type_match_into_rim_field_succeeds() {
        // Rim is a Point; give variant 1 so it has two Symbol fields.
        let mut d = point_doc();
        assert_eq!(
            apply(&mut d, Operation::SelectVariant { at: CellLocation::root(Region::Rim), variant: 1 }),
            Outcome::Applied
        );
        // destination: field 0 (x) of the Rim Point
        let dest = CellLocation { region: Region::Rim, path: vec![PathStep::Field { index: 0 }] };
        d.select(dest.clone());
        // source: Rom's symbol cell
        let src = CellLocation { region: Region::Rom, path: vec![PathStep::Grid { row: 2, col: 0 }] };
        assert_eq!(apply(&mut d, Operation::Copy { source: src }), Outcome::Applied);
        assert_eq!(check_integrity(&d), Ok(()));
    }

    #[test]
    fn copy_type_mismatch_into_rim_field_is_rejected_and_reds() {
        let mut d = point_doc();
        apply(&mut d, Operation::SelectVariant { at: CellLocation::root(Region::Rim), variant: 1 });
        let dest = CellLocation { region: Region::Rim, path: vec![PathStep::Field { index: 0 }] };
        d.select(dest.clone());
        // source: Rom's default tree (index 3) — a tree, into a non-tree Symbol field
        let src = CellLocation { region: Region::Rom, path: vec![PathStep::Grid { row: 3, col: 0 }] };
        assert_eq!(apply(&mut d, Operation::Copy { source: src }), Outcome::Rejected);
        assert_eq!(d.selection.red, vec![dest]);
        // the document content is unchanged: still passes integrity
        assert_eq!(check_integrity(&d), Ok(()));
    }

    #[test]
    fn copy_empty_into_a_field_is_rejected() {
        let mut d = point_doc();
        apply(&mut d, Operation::SelectVariant { at: CellLocation::root(Region::Rim), variant: 1 });
        let dest = CellLocation { region: Region::Rim, path: vec![PathStep::Field { index: 0 }] };
        d.select(dest);
        // source: the empty cell inside Rom's default tree (Rom cell 3 -> its cell (0,0))
        let src = CellLocation {
            region: Region::Rom,
            path: vec![PathStep::Grid { row: 3, col: 0 }, PathStep::Grid { row: 0, col: 0 }],
        };
        assert_eq!(apply(&mut d, Operation::Copy { source: src }), Outcome::Rejected);
    }

    #[test]
    fn copy_into_a_governed_tree_grid_cell_accepts_a_value() {
        // Rim is a Tree<Symbol>; copy a Symbol into the root grid's only cell.
        let mut d = tree_doc();
        let dest = CellLocation { region: Region::Rim, path: vec![PathStep::Grid { row: 0, col: 0 }] };
        d.select(dest);
        let src = CellLocation { region: Region::Rom, path: vec![PathStep::Grid { row: 2, col: 0 }] };
        assert_eq!(apply(&mut d, Operation::Copy { source: src }), Outcome::Applied);
        assert_eq!(check_integrity(&d), Ok(()));
    }

    #[test]
    fn add_and_delete_rows_keep_the_grid_rectangular() {
        let mut d = tree_doc(); // Rim root is a 1x1 Tree<Symbol>
        let cell = CellLocation { region: Region::Rim, path: vec![PathStep::Grid { row: 0, col: 0 }] };
        assert_eq!(apply(&mut d, Operation::AddRow { at: cell.clone(), side: VSide::Below }), Outcome::Applied);
        assert_eq!(apply(&mut d, Operation::AddColumn { at: cell.clone(), side: HSide::Right }), Outcome::Applied);
        if let CellContent::Tree(g) = &d.rim {
            assert_eq!((g.width(), g.height()), (2, 2));
            assert_eq!(g.cells().len(), 4);
        } else {
            panic!();
        }
        assert_eq!(check_integrity(&d), Ok(()));
    }

    #[test]
    fn a_cell_cannot_delete_its_own_row() {
        // a 1x1 tree: there is no neighbouring row, so delete is a NoOp.
        let mut d = tree_doc();
        let cell = CellLocation { region: Region::Rim, path: vec![PathStep::Grid { row: 0, col: 0 }] };
        assert_eq!(apply(&mut d, Operation::DeleteRow { at: cell.clone(), side: VSide::Above }), Outcome::NoOp);
        assert_eq!(apply(&mut d, Operation::DeleteRow { at: cell, side: VSide::Below }), Outcome::NoOp);
    }

    #[test]
    fn delete_row_removes_the_neighbour() {
        let mut d = tree_doc();
        let cell = CellLocation { region: Region::Rim, path: vec![PathStep::Grid { row: 0, col: 0 }] };
        apply(&mut d, Operation::AddRow { at: cell.clone(), side: VSide::Below }); // now 1x2
        assert_eq!(apply(&mut d, Operation::DeleteRow { at: cell, side: VSide::Below }), Outcome::Applied);
        if let CellContent::Tree(g) = &d.rim {
            assert_eq!(g.height(), 1);
        } else {
            panic!();
        }
    }

    #[test]
    fn select_variant_rebuilds_the_field_list() {
        let mut d = point_doc();
        let root = CellLocation::root(Region::Rim);
        // variant 0 -> no fields
        if let CellContent::Value(LeafValue::Struct(si)) = &d.rim {
            assert_eq!(si.variant, 0);
            assert!(si.fields.is_empty());
        }
        apply(&mut d, Operation::SelectVariant { at: root, variant: 1 });
        if let CellContent::Value(LeafValue::Struct(si)) = &d.rim {
            assert_eq!(si.variant, 1);
            assert_eq!(si.fields.len(), 2); // x, y
        } else {
            panic!();
        }
    }

    #[test]
    fn edit_symbol_changes_the_string() {
        let mut d = tree_doc();
        let cell = CellLocation { region: Region::Rim, path: vec![PathStep::Grid { row: 0, col: 0 }] };
        // first put a symbol there
        let src = CellLocation { region: Region::Rom, path: vec![PathStep::Grid { row: 2, col: 0 }] };
        d.select(cell.clone());
        apply(&mut d, Operation::Copy { source: src });
        // now edit it
        assert_eq!(
            apply(&mut d, Operation::EditSymbol { at: cell.clone(), text: "hello".into() }),
            Outcome::Applied
        );
        match d.resolve(&cell).content() {
            Some(CellContent::Value(LeafValue::Symbol(s))) => assert_eq!(s, "hello"),
            _ => panic!(),
        }
    }

    #[test]
    fn selection_rom_pick_clears_ram_highlights() {
        let mut d = tree_doc();
        // highlight two Ram cells
        d.select(CellLocation::root(Region::Ram));
        // (Ram root is 1x1; toggle the same cell on then a Rim pick)
        assert_eq!(d.selection.ram.len(), 1);
        d.select(CellLocation::root(Region::Rim)); // a non-Ram pick
        assert!(d.selection.ram.is_empty(), "a Rom/Rim pick clears Ram highlights");
    }

    #[test]
    fn selection_ram_toggle() {
        let mut d = tree_doc();
        // give Ram a second cell, so two distinct cells can be selected
        let a = CellLocation { region: Region::Ram, path: vec![PathStep::Grid { row: 0, col: 0 }] };
        apply(&mut d, Operation::AddRow { at: a.clone(), side: VSide::Below });
        let b = CellLocation { region: Region::Ram, path: vec![PathStep::Grid { row: 1, col: 0 }] };

        d.select(a.clone()); // toggle A on
        d.select(b.clone()); // toggle B on
        assert_eq!(d.selection.ram, vec![a.clone(), b.clone()]);
        assert_eq!(d.selection.superhighlighted, b);

        d.select(a.clone()); // toggle A off
        assert_eq!(d.selection.ram, vec![b.clone()]); // A is no longer highlighted
        // a Ram click — even a toggle-off — moves the superhighlight to that
        // cell; A is now superhighlighted but not highlighted (design §6.2)
        assert_eq!(d.selection.superhighlighted, a);
    }

    #[test]
    fn rom_is_read_only() {
        let mut d = point_doc();
        let rom_cell = CellLocation { region: Region::Rom, path: vec![PathStep::Grid { row: 0, col: 0 }] };
        assert_eq!(
            apply(&mut d, Operation::SelectVariant { at: rom_cell.clone(), variant: 1 }),
            Outcome::NoOp
        );
        assert_eq!(
            apply(&mut d, Operation::EditSymbol { at: rom_cell, text: "x".into() }),
            Outcome::NoOp
        );
    }

    #[test]
    fn integrity_catches_a_bad_variant_index() {
        let mut d = point_doc();
        if let CellContent::Value(LeafValue::Struct(si)) = &mut d.rim {
            si.variant = 99; // out of range
        }
        assert!(check_integrity(&d).is_err());
    }

    #[test]
    fn integrity_catches_a_non_rectangular_grid() {
        let mut d = tree_doc();
        // hand-build a broken grid: claims 2x2 but has 3 cells
        d.rim = CellContent::Tree(Grid {
            cells: vec![None, None, None],
            width: 2,
            height: 2,
        });
        assert!(check_integrity(&d).is_err());
    }

    #[test]
    fn integrity_catches_a_field_arity_mismatch() {
        let mut d = point_doc();
        if let CellContent::Value(LeafValue::Struct(si)) = &mut d.rim {
            si.variant = 1; // variant 1 declares 2 fields
            si.fields = vec![]; // but carries 0
        }
        assert!(check_integrity(&d).is_err());
    }

    #[test]
    fn snapshot_round_trip() {
        let mut d = point_doc();
        apply(&mut d, Operation::SelectVariant { at: CellLocation::root(Region::Rim), variant: 1 });
        let snap = d.snapshot();
        let restored = Document::from_snapshot(snap);
        assert_eq!(check_integrity(&restored), Ok(()));
        assert_eq!(restored.rim, d.rim);
        assert_eq!(restored.dirty, RegionMask::all());
    }

    #[test]
    fn undo_redo() {
        let mut d = tree_doc();
        let mut hist = History::default();
        let cell = CellLocation { region: Region::Rim, path: vec![PathStep::Grid { row: 0, col: 0 }] };

        hist.record(&d);
        apply(&mut d, Operation::AddRow { at: cell, side: VSide::Below });
        if let CellContent::Tree(g) = &d.rim {
            assert_eq!(g.height(), 2);
        }

        d = hist.undo(&d).expect("can undo");
        if let CellContent::Tree(g) = &d.rim {
            assert_eq!(g.height(), 1, "undo restored the 1x1 grid");
        }

        d = hist.redo(&d).expect("can redo");
        if let CellContent::Tree(g) = &d.rim {
            assert_eq!(g.height(), 2, "redo re-applied the row");
        }
    }
}
