# Implementation Specification — Bevy 0.18

**Status:** Implementation spec. Companion to the design spec (`spec-v3.md`).
The design spec answers *what the editor is*; this document answers *how it is
built*. Where the two ever disagree, **the design spec wins** — this document
is downstream of it and may be revised freely to stay consistent.

**Target:** Bevy 0.18 (released January 2026; current latest). This document
assumes the design spec's terminology — Symbol, Struct, Variant, Field, Tree,
Cell, cell tree, Rom/Ram/Rim, `Any` — without redefining it.

Rationale is included inline wherever a decision is non-obvious, mirroring the
design spec's convention, because several implementation rules only stay
consistent if the reasoning is preserved.

---

## 1. Scope and relation to the design spec

The design spec is deliberately implementation-agnostic. This document fills
the gap it names in §11: it pins down the data structures, the ECS architecture,
the systems and observers, and the mapping from the design's abstract
operations to concrete Bevy code.

Two kinds of open question must not be confused:

- **Design open questions** — the five in design §10 (verb naming, deleting
  your own row/column, the in-app type editor, type-representation strategy,
  the always-selected invariant). This document does **not** resolve those; it
  is written so that each resolution slots in cleanly.
- **Implementation open questions** — calls the *code* must make that the
  design spec never addressed. These are collected here in §12.

---

## 2. Architecture — the document is the source of truth, the ECS is a view

**This is the load-bearing decision of this document.** It was reached by
ruling out the alternative; the alternative is recorded so it is not
reintroduced.

### 2.1 The rule

There are two layers:

1. **The document** — the schema registry and the three cell trees — is a
   plain Rust data structure held in Bevy `Resource`s. It is **not** built
   from entities. All *semantics* live here: flatten, validation, every
   operation in design §7, every movement rule in design §8.
2. **The ECS / `bevy_ui` layer** is a **rebuilt view** of the document. Each
   visible Cell is rendered as an entity with a `Node` and (for leaves) a text
   input. These entities hold only a back-reference into the document
   (§4.5) plus presentation state. They are disposable: when the document
   changes, the affected region's entity subtree is rebuilt from data.

A useful one-line summary: **a node on screen is the *view* of a Cell, not the
Cell.**

### 2.2 Why, not the alternative

> **Rejected alternative — "Cells *are* entities; the cell tree lives in ECS
> relationships."** Do not reintroduce this. It rebuilds friction at three
> points:
>
> - **Design §11 already decided.** It states the document "is its own data
>   structure, validated by hand-written structural checks," and that Bevy
>   provides the ECS/UI "for the *editor*." The document and the editor are
>   named as separate things.
> - **Semantics through the rendering layer.** Flatten (§4.6) and the §6.4
>   validation tree-walk are short recursive functions over plain data. Done
>   over entities they become relationship traversals with component fetches —
>   the same logic, coupled to the renderer, harder to test in isolation.
> - **Copy and the compile-time variant get worse.** Rom→out is a *copy*
>   (design §8); on plain data that is `Cell::clone()`. On entities it is
>   recursive subtree cloning with relationship fix-up. And the compile-time
>   variant (design §10.4, this doc §11) needs one clean abstraction boundary
>   for "runtime record vs. real Rust struct" — easy as a data enum, smeared
>   everywhere if Cells are entities.

The view layer still uses entities heavily — that is unavoidable and correct,
because `bevy_ui` *is* entity-based. The distinction is only about **where
identity and storage live**: in the document, never in the view.

### 2.3 Consequence — the rebuild boundary

Because the view is derivable, the editor never mutates entities to change
meaning. The cycle is always:

```
input → operation mutates the document → mark region dirty → view rebuilds
```

A "dirty region" flag (or a `RegionChanged` message) drives a system that
despawns and respawns that region's view subtree. Start with **whole-region
rebuilds**; diffing is an optimization (§12).

---

## 3. Crate and feature setup

The editor is a UI application, not a 3D game. Bevy 0.18's feature collections
make this a thin build:

```toml
[dependencies]
bevy = { version = "0.18", default-features = false, features = [
    "ui",            # bevy_ui + the renderer pieces UI needs
    "bevy_ui_widgets",   # standard widgets (buttons, sliders, text input)
    "bevy_window",
    "bevy_winit",
    "x11", "wayland", # or platform-appropriate windowing backends
] }
```

Notes:

- The `ui` feature collection is the supported way to use Bevy "as a UI
  framework, without pulling in the rest of the engine" — exactly this
  project's situation.
- The widget layer is **`bevy_ui_widgets`** — the standard 0.18 widget set
  (buttons, and the text input the Symbol leaves need). This is the committed
  baseline; the rest of this document assumes it. **Bevy Feathers**, the
  tooling-oriented widget library, is experimental in 0.18 and an attractive
  *later* upgrade for editor polish, but nothing here depends on it — the
  upgrade can happen independently.
- `bevy_reflect` is **not** a dependency of the default build. It enters only
  in the compile-time variant (§11).

---

## 4. The document model

All types in this section are plain Rust — `#[derive(Clone, ...)]`, no
`Component` derive. They live inside resources (§5.1).

### 4.1 The schema registry

The schema is a runtime registry of records (design §4.1):

```rust
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
```

`is_tree` is a **`bool`, not a wrapper** — this is the mechanical reason
`Tree<Tree<T>>` is unexpressible (design §3.2). There is nowhere in `FieldDef`
to write a second `Tree`. The depth-1 cap is therefore enforced *by the shape
of `FieldDef`*, not by a runtime check.

`StructId` is an index into `Schema::structs`. A `StructDef` with zero variants
is representable and is rejected only at *instantiation* time — there is no
variant to pick — exactly as design §2 permits.

### 4.2 The cell tree

A Cell carries a value-holder and nothing else:

```rust
struct Cell {
    content: CellContent,
}

enum CellContent {
    Value(LeafValue),                 // the T case
    Grid(Grid),                       // the Grid case; may be empty
}
```

This is the design's `T | Grid` (design §4.2) verbatim. **`Cell` has no `type`
field** — see §4.4; the absence is deliberate and load-bearing.

The grid is a genuine 2D **rectangular matrix** of Cells:

```rust
struct Grid {
    cells: Vec<Cell>,                 // row-major; len() == width * height
    width: usize,
    height: usize,
}
```

**Invariant: `cells.len() == width * height`.** A grid is always rectangular
`width × height`; cell `(r, c)` is `cells[r * width + c]`. Raggedness — rows of
differing length — is **unrepresentable by construction**, in the same spirit
as `Tree<Tree<T>>` being unexpressible (§4.1). An *empty* tree is
`width == 0, height == 0, cells == []` — "empty" is not a third state (design
§4.2).

Rectangularity is preserved *by the operations themselves*: the only breadth
operations (design §7.2, this doc §6) add or remove **whole rows** and **whole
columns**. No operation builds a grid one dimension at a time, so no operation
can ever produce a ragged grid. There is consequently no "which row does
column-left affect" question — a column operation necessarily spans every row.

This matches design §4.3 (`spec-v3.md`), which describes the grid as a 2D
rectangular matrix. (Earlier design drafts up to `spec-v2.md` carried a
misleading offset example and a "need not align into a clean matrix"
parenthetical; both were removed in v3.)

### 4.3 Leaf values, and the two independent recursions

```rust
enum LeafValue {
    Symbol(String),
    Struct(StructInstance),
}

struct StructInstance {
    struct_id: StructId,
    variant: usize,                   // index into StructDef::variants
    fields: Vec<Cell>,                // one Cell per field of that variant
}
```

A field of a variant is always stored as a `Cell` — uniformly, whether the
field's type is `T` or `Tree<T>`. The `is_tree` flag does not change the
*storage*; it changes only what the **validator** (§8) accepts:

- `is_tree == false` → the field's Cell content must be `Value`.
- `is_tree == true`  → the field's Cell is the root of a `Tree<T>`; its content
  may be `Value` or `Grid`, any shape.

**There are two recursions in this model, on different axes, and they must not
be conflated:**

1. **The grid axis** — `CellContent::Grid` containing `Cell`s. This is the
   recursion design §4.2 means by "every Cell is itself a Tree." Flatten
   (§4.6) walks *this* axis and **stops at `Value`**.
2. **The field axis** — `StructInstance::fields` containing `Cell`s. A struct
   instance is internally a tree of values (design §4.1), but along a
   *different* axis.

Flatten of a `Tree<T>` does **not** descend into a struct instance's fields. A
`LeafValue::Struct(...)` is *one element* `T` of the flattened sequence, even
though it has internal field structure. Keeping these axes separate is what
makes both the flatten invariant and structural validation coherent.

### 4.4 Cells carry no type — enforced by absence

`Cell` has no `type` field, and this is intentional. It is the data-model
expression of design §6.1: *typing is a property of fields, not of Cells.*

A type lives only on `FieldDef::elem` (+ `is_tree`). A `Cell` holds a value; it
is checked against a type only at the moment it enters a typed field (§8).
If a future change adds a `Cell::ty` field "for convenience," it has
reintroduced the contradiction design §6 was written to kill. Do not.

### 4.5 Addressing cells — `CellPath`

The view layer and every operation need to name a cell inside the document. A
path is a region tag plus a sequence of steps down the two axes:

```rust
struct CellLocation {
    region: Region,                   // Rom | Ram | Rim
    path: Vec<PathStep>,
}

enum PathStep {
    Grid { row: usize, col: usize },  // descend the grid axis
    Field { index: usize },           // descend the field axis
}

enum Region { Rom, Ram, Rim }
```

An empty `path` denotes the region's root Cell itself (§5.1). Paths are
*positional*, so a structural mutation can invalidate outstanding paths;
operations resolve a path to a `&mut Cell` immediately before acting, and the
view is rebuilt (with fresh paths) after every mutation — so stale paths never
survive a frame boundary. Path stability is noted in §12.

### 4.6 Flatten and semantic equality

Flatten walks the grid axis in **row-major reading order** and bottoms out at
`Value`:

```rust
fn flatten<'a>(cell: &'a Cell, out: &mut Vec<&'a LeafValue>) {
    match &cell.content {
        CellContent::Value(v) => out.push(v),
        CellContent::Grid(g) => {
            for c in &g.cells { flatten(c, out); }   // cells are row-major
        }
    }
}
```

This is the design §4.4 flatten invariant in code: the flattened sequence is
the **semantic** identity; the tree shape is **storage/layout** identity only.
Two cell trees are semantically equal iff their flattens are equal — where
`LeafValue` equality compares two struct instances by `struct_id`, `variant`,
and field-Cell semantic equality (a recursion down the *field* axis).

The editor keeps the full `Cell` (it must, to redraw the canvas), but no piece
of *meaning* may depend on tree shape.

### 4.7 Serialization and export

The document is plain data (§2), so all of it derives `serde`'s `Serialize` /
`Deserialize` in the runtime build. Three distinct outputs:

**Save format — binary.** The working save/load format is **binary** (e.g.
`bincode` over the `serde` derives). A save file captures `schema`, `ram`,
`rim`, and `selection`; `rom` is *not* saved — it is the fixed palette, rebuilt
from `schema` on load (design §5.1, §9). Binary is the primary format because
it is compact and round-trips the *exact tree shape*, which the editor needs to
redraw the canvas faithfully (design §4.4 — the tree is the storage source of
truth).

**Full JSON export — optional.** A feature-gated action serializes the same
full document structure to human-readable JSON via `serde_json`. This is a
one-way *export* for inspection or interchange, not a working format; the
binary save remains canonical. It preserves tree shape.

**Canvas export — JSON, flattened.** A separate action exports **only Rim**,
the canvas, as JSON, with **every `Tree<T>` flattened**. This is deliberately a
*semantic* export and is motivated directly by design §4.4: the flattened
sequence is the semantic source of truth; the tree shape is mere layout memory.
Canvas export discards layout and emits meaning:

- a `Tree<T>` Cell becomes a JSON array of its flattened `T` values (§4.6 —
  grid axis, row-major reading order);
- a `Symbol` becomes its string;
- a struct instance becomes a JSON object — its variant, plus each field
  exported by the same rule recursively (a `Tree<T>` field → array, a plain `T`
  field → value).

The flatten stops at `T` and recurses the *field* axis separately, exactly as
§4.3 require — so canvas export yields clean semantic JSON: the *meaning* of
the canvas, stripped of the editor's 2D layout memory. (The full JSON export,
by contrast, keeps tree shape; the two exports answer different questions.)

In the compile-time variant (§11) the leaf is a reflected value rather than
`LeafValue`; its serialization routes through `bevy_reflect`'s serializers
instead of plain `serde` derives. The save/export *structure* above is
unchanged — only the leaf seam differs.

---

## 5. The ECS view layer

### 5.1 Region resources

Each region is a grid of Cells, equivalently a single Cell whose content is a
`Grid` (design §5). The whole document is one resource:

```rust
#[derive(Resource)]
struct Document {
    schema: Schema,
    rom: Cell,                        // content is always Grid
    ram: Cell,                        // content is always Grid
    rim: Cell,                        // content is always Grid
    selection: Selection,
    dirty: RegionMask,                // which regions need a view rebuild
}
```

Holding the regions in one resource (rather than three) keeps cross-region
moves — which touch two regions and the selection at once — a single
borrow.

`rom` is initialized once from `schema`: one Cell per defined Struct (a
`Value(Struct(default instance))`), one Cell for `Symbol`
(`Value(Symbol(String::new()))`), and one **bare Tree** — a Cell with
`content: Grid(empty)` (design §5.1). The bare Tree is *not* typed and does not
become typed when placed; it is simply a Cell that later sits in a typed field,
which is where the type comes from (design §6 — no conversion event).

### 5.2 Rebuilding the view from the document

A system reacts to `Document::dirty`: for each dirty region it despawns that
region's view subtree and respawns it from the region's `Cell`.

Per visible Cell, spawn an entity with:

```rust
#[derive(Component)]
struct CellView {
    loc: CellLocation,                // back-reference into the document
}
```

plus a `Node` for layout, a background, and:

- if `CellContent::Value(Symbol(s))` → a text-input child showing `s`;
- if `CellContent::Value(Struct(inst))` → the variant selector + a child view
  per field Cell (descending the field axis);
- if `CellContent::Grid(g)` → nested `Node`s, one per row, each holding its
  cells' views (descending the grid axis).

`CellView::loc` is the *only* document link a view entity needs; it is how a
click is translated back into a document mutation (§5.3).

### 5.3 Picking and selection

Picking uses `bevy_picking`: attach an observer to each `CellView` entity for
pointer-click events (`On<Pointer<Click>>`-style). The observer reads
`CellView::loc` and issues the corresponding document change — a selection
update, or, for the operation buttons, the operation.

**Selection lives in the document, not in entities:**

```rust
struct Selection {
    rom: CellLocation,                // always present (design §5.4)
    rim: CellLocation,                // always present (design §5.4)
    ram: Vec<CellLocation>,           // may be empty (design §5.2, §8.2)
}
```

It must survive view rebuilds — entities are disposable — so it cannot be
entity state. The always-selected invariant (design §5.4, *provisional* per
design §10.5) is enforced here: `rom` and `rim` are non-optional fields, `ram`
is a `Vec` that may be empty. Persisting a Ram selection is the
select-then-click-again gesture (design §5.2), tracked as appends to
`Selection::ram`.

---

## 6. Operations as document mutations

Every operation in design §7 is a plain function over the document:

```rust
fn apply(doc: &mut Document, op: Operation) -> Result<(), MoveError>;
```

UI buttons are `CellView`-adjacent entities; their click observers build an
`Operation` and call `apply`. After a successful `apply`, the touched
region(s) are marked in `doc.dirty` and the view rebuilds.

The operation set (settled in design §7 and §10.1 — only the *labels* are
open):

- **move** — the per-Cell move (design §7.1).
- **row above / row below / column left / column right** — change one grid
  level's *breadth*; never change depth (design §7.2). Each inserts a full row
  (`width` cells) or full column (`height` cells), preserving the grid's
  rectangular invariant (§4.2).
- **delete + {above/below/left/right}** — delete a *neighboring* row/column.
  A Cell cannot delete its own row/column (design §7.2; lifting this is design
  open question §10.2 — trivial to enable here, it is one guard).
- **move self / copy self** — act on the whole subtree rooted at the Cell.
- **move contents / copy contents** — act on the Cell's `CellContent`.

There is **no within/without verb** (design §7.3) and none should be added.
Depth changes are emergent:

- **Depth decrease** — the Ram round-trip: `move self` to Ram, edit the parent,
  `move self` back; or `move contents` to flatten.
- **Depth increase** — `move self` on a region's *root* Cell. There is no
  parent to move within, so the only coherent effect is: the root Cell leaves,
  and a **fresh empty root Cell** (`content: Grid(empty)`) is created in its
  place. In code this is a special case in `apply` keyed on
  `loc.path.is_empty()`. This behavior is intended and **manual-worthy**
  (design §7.3) — implement it deliberately, do not "fix" it.

`move contents` of a grid moves the whole grid as one tree into one Ram cell —
no carrier-wrapping, because every Ram cell already holds a whole tree (design
§7.4).

### 6.1 Undo/redo

Undo/redo is in scope. The §2 architecture makes it cheap: the document is
plain data and the single source of truth, so an undo step is just a remembered
document state.

**Snapshot-based, to start.** Before each mutating `apply`, push a snapshot of
the document onto an undo stack and clear the redo stack. A snapshot captures
`schema`, `ram`, `rim`, and `selection` — *not* `rom`, which is the fixed
palette and never changes (§5.1). `schema` is included so that, if the in-app
type editor (design §10.3) is built, schema edits are undoable too.

```rust
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
```

- **Undo** — pop `undo`, push the *current* state onto `redo`, restore the
  popped snapshot.
- **Redo** — the mirror: pop `redo`, push current onto `undo`, restore.
- **A fresh edit clears `redo`** — standard linear-history behavior.
- **Restoring marks every region dirty**, so the view rebuilds from the
  restored document through the normal §2.3 cycle. Undo and redo are ordinary
  inputs (e.g. Ctrl+Z / Ctrl+Y) that mutate the document; nothing in the view
  layer is special-cased.

Two refinements, deferred but worth noting:

- **Coalescing.** Typing into a Symbol fires a mutation per keystroke; without
  coalescing, undo would step character by character. Group consecutive text
  edits to the *same* Cell into one undo step.
- **Snapshots vs. diffs.** Whole-document snapshots are simple but grow with
  tree size. If memory becomes a concern, switch to storing each operation's
  inverse (or a structural diff) — the operation set (§6) is small and every
  operation has a clean inverse, so this is tractable later.

---

## 7. Movement rules

`apply` dispatches cross-region moves through one function implementing the
design §8 table:

| From → To   | Implementation                                                            |
|-------------|---------------------------------------------------------------------------|
| Rom → Ram   | `cell.clone()` into Ram. Infallible. Rom is never mutated.                 |
| Rom → Rim   | Rejected before any work — must route via Ram.                            |
| within Rom  | Rejected — no copy, no move.                                              |
| within Ram  | Move or clone between Ram cells. Infallible. Multi-select duplicates.      |
| Rim → Ram   | Move the Cell into Ram. Infallible.                                       |
| Ram → Rim   | **Fallible** — validate (§8), then place. See below.                      |
| within Rim  | No direct op — round-trip via Ram (design §8.1).                          |

Two derived behaviors:

- **Deletion** (design §8.2) is `Rim → Ram` performed while `Selection::ram` is
  empty. The moved Cell lands nowhere and is dropped. Zero-selection is valid
  only in Ram, so this is unambiguous.
- **Copy / move within Rim** (design §8.1) is a Ram round-trip — `Rim → Ram`
  (using Ram multi-select to duplicate, for copy) `→ Rim`. The intermediate is
  **Ram**, never Rom.

### 7.1 The fallible move

`Ram → Rim` is the **only** move that can fail. The destination is a specific
typed Rim field; the candidate is the Ram Cell:

```rust
fn move_ram_to_rim(doc: &mut Document, from: &CellLocation, to: &CellLocation)
    -> Result<(), MoveError>
{
    let dest_field = resolve_field_def(&doc.schema, &doc.rim, to)?;
    let candidate  = resolve_cell(&doc.ram, from)?;
    validate(candidate, dest_field, &doc.schema)
        .map_err(MoveError::Validation)?;
    // only on success: detach from Ram, attach at `to`
    ...
}
```

Failure carries the offending leaf's path so the UI can reject the drop and
highlight it (design §8.3):

```rust
enum MoveError {
    Forbidden,                        // Rom→Rim, within-Rom
    Validation(ValidationError),
}

struct ValidationError {
    offending: CellLocation,          // the leaf that failed
    expected: TypeRef,
}
```

---

## 8. Validation and `Any` — the load-bearing section

### 8.1 `Any` is not in the code

Design §6.2 is emphatic and this implementation honors it literally:

- There is **no `TypeRef::Any` variant.**
- There is **no `Any` value, no `Cell::ty`, nothing that "becomes `Any`."**
- `Any` is the *name for the absence of a `validate()` call.* "A tree in Ram is
  `Tree<Any>`" means exactly: **Ram code paths never call `validate`.** Nothing
  more.

> **Rejected alternatives — do not reintroduce (each rebuilds a design §6
> contradiction):**
> - a `TypeRef::Any` variant;
> - a `Cell::ty` field set to "untyped" in Ram;
> - marking a Ram grid `Tree<Any>` and treating it as unmovable;
> - allowing such a value into a Rim field.
>
> Heterogeneous Ram trees need **no machinery** — they are simply the result of
> Ram never validating. Heterogeneity is a *consequence*, not a feature with
> code behind it.

### 8.2 The structural tree-walk

Validation runs **only** on entry into a typed Rim field (design §6.4, §8.3).
It is a structural walk of the candidate — root and every grid-axis child —
confirming every leaf is a valid `T` for the destination's element type:

```rust
fn validate(cell: &Cell, field: &FieldDef, schema: &Schema)
    -> Result<(), ValidationError>
{
    match &cell.content {
        CellContent::Grid(g) => {
            if !field.is_tree { return Err(/* non-tree field, got a grid */); }
            for c in &g.cells { validate_as_elem(c, &field.elem, schema)?; }
            Ok(())
        }
        CellContent::Value(_) => validate_as_elem(cell, &field.elem, schema),
    }
}
```

`validate_as_elem` checks one leaf against a `TypeRef`: `Symbol` accepts any
string; `Struct(id)` requires a `StructInstance` with matching `struct_id`, an
in-range `variant`, and — recursing the *field* axis — every field Cell valid
against its own `FieldDef`.

The check inspects **values**, never Cell-types — there are no Cell-types to
inspect (§4.4). A homogeneous tree matching the target passes; a heterogeneous
or wrongly-typed tree fails, at the moment of the move, against a named
destination, with the offending leaf reported. This *is* what "a `Tree<Any>`
converts back to `Tree<T>` on the way to Rim" means: a **check**, never a
mutation of data.

---

## 9. UI layout with `bevy_ui`

- **Three regions** — three top-level `Node`s under the UI root, laid out as a
  flex row (Rom palette, Ram scratch, Rim canvas) or as resizable panels.
- **A region's grid** — since the document's `Grid` is a true `width × height`
  matrix (§4.2), it maps directly onto a `Display::Grid` `Node` with `width`
  columns and `height` rows. (Nested flex — one row `Node` per row — works
  equally well and may be simpler for per-cell controls; either is fine, the
  grid being rectangular.)
- **A leaf Cell** — `Symbol` → the `bevy_ui_widgets` text input; `Struct` → a
  variant selector (dropdown/segmented control) plus a field sub-layout.
- **Operation buttons** — `move`, the row/column verbs, `delete + direction`,
  `self`/`contents` verbs — rendered on or beside the selected Cell's view,
  using `bevy_ui_widgets` buttons. Labels are design open question §10.1; wire
  the buttons to `Operation` values and keep label text in one place so
  renaming is a one-file change.
- **Move/validation feedback** — a rejected `Ram → Rim` drop highlights the
  `CellView` whose `loc` matches `ValidationError::offending`.

---

## 10. Schedule and state

The default editor needs little global state. A minimal `States` enum is
warranted only if design open question §10.3 (the in-app type editor) is
built — e.g. `EditorMode::Build` vs `EditorMode::EditSchema`, with `OnEnter`
swapping the visible panels.

Frame flow:

- **Input** — `bevy_picking` observers on `CellView` entities translate clicks
  into `Operation`s and selection changes.
- **Apply** — a system (or the observers directly) calls `apply`, mutating
  `Document` and setting `Document::dirty`.
- **Rebuild** — a system reads `dirty`, rebuilds the view subtree of each dirty
  region, clears `dirty`.

Use observers + triggered `Event`s for reactive, point-in-time things (a click,
a completed move); use buffered `Message`s only where frame-batching is
genuinely wanted. Keep observer ordering assumptions out of the design — Bevy
does not guarantee relative ordering of observers for the same event.

---

## 11. The compile-time variant (design §10.4)

Design §10.4 keeps open an optional build where pre-known types are real Rust
structs with `bevy_reflect`, selected via `#[cfg]`, with **identical frontend
behavior**. The architecture of §2 is what makes "identical frontend" achievable:
the view layer, the operations, and the movement rules all act on `Cell` /
`CellContent` and never on `LeafValue`'s internals directly.

So the compile-time variant changes exactly one seam — the leaf and the schema:

- **Default (runtime) build** — `LeafValue` as in §4.3; `Schema` as in §4.1,
  hand-validated. `bevy_reflect` unused.
- **Compile-time build** — `#[cfg]`-selected. Structs are real Rust types with
  `#[derive(Reflect)]`; the schema is derived from `bevy_reflect`'s
  `TypeRegistry` instead of a hand-built `Vec<StructDef>`; the leaf wraps a
  reflected value. `bevy_reflect` is used **here and only here** (design §11).

Everything above the leaf/schema seam — §4.2, §4.5, §4.6, §5, §6, §7, §8, §9 —
compiles unchanged for both builds. Keep that seam narrow: if compile-time
specifics leak into the view or the operations, "identical frontend behavior"
is no longer free. Proceed with the runtime build first (design §10.4); the
compile-time route turns the project into a library and stays open.

---

## 12. Implementation-level open questions

Distinct from the design spec's §10. With the grid model settled as a
rectangular matrix (§4.2), the widget library chosen (§3), and undo/redo and
serialization now specified (§6.1, §4.7), two implementation calls remain
genuinely open:

1. **View rebuild granularity.** Whole-region rebuild (§2.3) is the simple
   start. Diffing the document against the existing entity subtree is the
   optimization; defer it until a region is large enough to matter.

2. **`CellPath` stability.** Paths are positional (§4.5) and a mutation can
   invalidate outstanding ones. The "resolve immediately, rebuild after"
   discipline avoids stale paths within a frame; if a future feature needs
   paths to survive across frames, a stable per-Cell id would be needed.
