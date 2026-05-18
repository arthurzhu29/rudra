# Rudra — Implementation Specification

**Version:** 2. Supersedes `impl-spec-v1.md` (now `impl-spec-v1-outdated.md`) in
full.

**Status:** Implementation spec. Companion to the design spec, `spec-v4.md`.
The design spec answers *what the editor is*; this document answers *how it is
built*. Where the two ever disagree, **the design spec wins** — this document
is downstream of it and may be revised freely to stay consistent. Section
references of the form "design §N" point into `spec-v4.md`.

**Target:** Bevy 0.18 (released January 2026). This document assumes the design
spec's vocabulary — schema, struct, variant, field, cell, content, value, tree,
Rom/Ram/Rim, governed/ungoverned, flatten — without redefining it.

This is a rewrite, not a patch, because the design it implements was rewritten.
The data model collapsed (`content = value | tree`, emptiness moved into the
grid cell), the operation set collapsed (one structural operation, `copy`), and
structs lost their partial states. Rationale is kept inline wherever a decision
is non-obvious, mirroring both specs' convention.

---

## 1. Scope and relation to the design spec

The design spec is deliberately implementation-agnostic. This document fills
that gap: it pins down the concrete data structures, the ECS architecture, the
systems and observers, and the mapping from the design's abstract operations to
Bevy code.

Two kinds of open question must not be confused:

- **Design open questions** — the six in design §10 (in-program schema
  editing, re-selecting the Rim root field, operation labels, the variant-0
  placeholder label, abandoning copy mode, canvas export shape). This document
  does **not** resolve those; it is written so each resolution slots in cleanly.
- **Implementation open questions** — calls the *code* must make that the
  design spec never addressed. These are collected in §11.

One note on scope. The earlier design spec (`spec-v3`) kept open an optional
"compile-time variant" where pre-known struct types were real Rust types via
`bevy_reflect`. `spec-v4` did not carry that idea forward, so this document
does not cover it. If it returns to the design, the architecture of §2 is what
would keep it cheap — but until then it is out of scope.

---

## 2. Architecture — the document is the source of truth, the ECS is a view

**This is the load-bearing decision of this document.** It is unchanged from
v1; the model rewrite did not touch it. It was reached by ruling out the
alternative, and the alternative is recorded so it is not reintroduced.

### 2.1 The rule

There are two layers:

1. **The document** — the schema registry and the three cell trees — is a
   plain Rust data structure held in a Bevy `Resource`. It is **not** built
   from entities. All *semantics* live here: flatten, validation, the integrity
   check, and every operation in design §7.
2. **The ECS / `bevy_ui` layer** is a **rebuilt view** of the document. Each
   visible cell is rendered as an entity carrying a `Node`, presentation state,
   and a back-reference into the document (§6). View entities are disposable:
   when the document changes, the affected region's entity subtree is despawned
   and respawned from data.

One-line summary: **a node on screen is the *view* of a cell, not the cell.**

### 2.2 Why, not the alternative

> **Rejected alternative — "cells *are* entities; the cell tree lives in ECS
> relationships."** Do not reintroduce this. It rebuilds friction at two
> points:
>
> - **Semantics through the rendering layer.** Flatten (§5), the validation
>   walk (§9.2), and the integrity check (§9.4) are short recursive functions
>   over plain data, and they are unit-testable in isolation precisely because
>   they touch no entities. Done over entities they become relationship
>   traversals with component fetches — the same logic, coupled to the
>   renderer, far harder to test.
> - **Copy becomes expensive and fragile.** Copy is `CellContent::clone()` on
>   plain data (§8.1). On entities it is recursive subtree cloning with
>   relationship fix-up — more code, and a new class of bug.

The view layer still uses entities heavily — unavoidable and correct, because
`bevy_ui` *is* entity-based. The distinction is only about **where identity and
storage live**: in the document, never in the view.

### 2.3 The rebuild boundary

Because the view is derivable, the editor never mutates an entity to change
*meaning*. The cycle is always:

```
input  →  an operation mutates the document  →  mark region(s) dirty  →  view rebuilds
```

A per-region dirty flag (§4.6) drives a system that despawns and respawns that
region's view subtree. Start with **whole-region rebuilds**; subtree diffing is
an optimization (§11).

---

## 3. Crate and feature setup

The editor is a UI application, not a 3D game. Bevy 0.18's feature collections
keep the build thin:

```toml
[package]
name = "rudra"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = { version = "0.18", default-features = false, features = [
    "ui",                            # bevy_ui + the renderer pieces UI needs
    "experimental_bevy_ui_widgets",  # the standard widget set — see note below
    "bevy_window",
    "bevy_winit",
    "x11", "wayland",                # platform windowing backends
] }
serde = { version = "1", features = ["derive"] }
bincode = "2"                        # binary save format (§9.5)
serde_json = "1"                     # JSON export (§9.5)

[profile.dev]
opt-level = 1                        # keep our crate's debug builds responsive

[profile.dev.package."*"]
opt-level = 3                        # but keep dependencies (Bevy) fast
```

A note on the widget feature, because v1's prose got this wrong. The widget
collection — buttons, and the text input the Symbol cells need — lives in the
crate `bevy_ui_widgets`, but the **cargo feature that gates it is named
`experimental_bevy_ui_widgets`** in Bevy 0.18 (the `experimental_` prefix is
the feature flag, not the crate). The feature line above is correct; any prose
that says "the feature is `bevy_ui_widgets`" is not.

---

## 4. The document model

All types in this section are plain Rust — `#[derive(Clone, Serialize,
Deserialize, ...)]`, no `Component` derive. They live inside the `Document`
resource (§4.6). Design §2 lists the invariants these types must uphold; this
section notes, per type, which invariants are **structural** (guaranteed by the
type's shape) and which are **maintained** (the type can express a violation;
§8 operations and §9.4 keep it from happening).

### 4.1 The schema registry

```rust
struct Schema {
    structs: Vec<StructDef>,          // indexed by StructId
}

struct StructDef {
    name: String,
    variants: Vec<VariantDef>,        // variants[0] is the nameless empty variant
}

struct VariantDef {
    name: String,                     // empty string for variant 0
    fields: Vec<FieldDef>,            // empty for variant 0
}

struct FieldDef {
    name: String,
    elem: TypeRef,                    // the element type
    is_tree: bool,                    // the depth-1 toggle (design §3.4)
}

enum TypeRef {
    Symbol,
    Struct(StructId),                 // StructId is an index into Schema::structs
}

type StructId = usize;
```

`is_tree` is a **`bool`, not a wrapper**. This is the mechanical reason
`Tree<Tree<T>>` is unexpressible (design §3.4): there is nowhere in `FieldDef`
to write a second `Tree`. The depth-1 cap is **structural**.

Every `StructDef` has `variants[0]` = the nameless empty variant — a
`VariantDef` with `name: String::new()` and `fields: vec![]` (design §3.2).
Unlike v1, **a zero-*variant* struct is not representable as a valid schema**:
every struct has at least variant 0, so a struct can always be instantiated.
That `variants[0]` is the empty variant is a *maintained* invariant — the type
does not force it — and §9.4 checks it at load.

`StructId` being a bare `usize` index means every `TypeRef::Struct(id)` is an
assertion that `id` resolves; that is invariant 2, maintained by §9.4.

### 4.2 The cell tree

Content is two cases — no third. Emptiness is **not** a content case; it is the
absence of content at a grid position.

```rust
enum CellContent {
    Value(LeafValue),                 // a value: a Symbol or a Struct
    Tree(Grid),                       // a tree: a rectangular grid of cells
}

struct Grid {
    cells: Vec<Option<CellContent>>,  // row-major; len() == width * height
    width: usize,
    height: usize,
}
```

This is design §3.3 in code. Read it carefully, because the whole model turns
on it:

- A **grid cell** is an element of `Grid::cells`: an **`Option<CellContent>`**.
  `None` is the empty cell; `Some(c)` is a cell holding content. Emptiness is
  one of its states.
- A **field cell** — a struct field slot (§4.3) and the Rim root (§4.6) — is a
  bare **`CellContent`**. It has no `None`; it is never empty.

Invariant 5 ("fields are never empty") is therefore **structural**: a field
slot is typed `CellContent`, which has no empty case to express. There is no
runtime check and no `Empty` variant to forget to handle. v1's
`CellContent::Empty` is **gone**; do not reintroduce it (§9.3 gravestone).

A consequence: a `Grid` is **non-empty and rectangular** — `width >= 1`,
`height >= 1`, `cells.len() == width * height`, cell `(row, col)` at
`cells[row * width + col]`. There is no zero-sized grid; "an empty tree" is not
a thing. A tree you think of as empty is a `Grid` (of at least one cell) every
cell of which is `None`. This is a **maintained** invariant (invariant 3),
preserved by the breadth operations (§8.2) — which add and remove only whole
rows and whole columns, so raggedness is unreachable — and checked at load by
§9.4.

There is no `Cell` wrapper struct. v1 had `struct Cell { content: CellContent }`;
it carried nothing else and is dropped. A "cell" is a *position* — a slot — and
the slot's type (`Option<CellContent>` for a grid cell, `CellContent` for a
field cell) already says everything. `Tree` and `Grid` are likewise one type:
the design distinguishes "tree" (the content case) from "grid" (its internal
shape) as concepts, but the code needs only `CellContent::Tree(Grid)`.

### 4.3 Leaf values and struct instances

```rust
enum LeafValue {
    Symbol(String),
    Struct(StructInstance),
}

struct StructInstance {
    struct_id: StructId,
    variant: usize,                   // index into StructDef::variants — always valid
    fields: Vec<CellContent>,         // one field cell per field of the chosen variant
}
```

`variant` is a **plain `usize`, not `Option`**. v1 made it optional to model "a
struct before a variant is picked"; that partial state no longer exists (design
§3.2) — a struct is always at a variant, defaulting to 0. `fields` is a `Vec` of
**`CellContent`** (not `Option<CellContent>`): struct fields are field cells,
never empty (§4.2).

That `variant` is in range, and that `fields.len()` equals the chosen variant's
field count, are **maintained** invariants (invariant 7), upheld by the
variant-selection operation (§8.3) and checked at load (§9.4).

> **Variant 0 gets no special-casing in code.** A variant-0 struct is the
> ordinary `StructInstance { struct_id, variant: 0, fields: vec![] }` — it
> arises through the same constructors, renders through the same view code,
> validates through the same `check_value` (§9.2), and is integrity-checked
> through the same walk as any other variant. Do **not** write `if variant == 0`
> shortcuts. The empty variant is special in *meaning* (it is the struct's
> "empty" form, design §3.2), never in *mechanism*. Its only structural
> property — zero fields — is just what `variants[0].fields` being empty
> already gives you.

### 4.4 Cells carry no type

Neither `CellContent` nor `LeafValue` carries a type tag, and this absence is
intentional — it is design §3.5 in code. A type lives only on `FieldDef`
(`elem` + `is_tree`) and on the Rim root field (§4.6). Content is checked
against a type only at the moment it is copied into a governed slot (§9).

This is what makes Ram and Rim one data model with the checks turned off in one
of them. If a future change adds a `CellContent::ty` field "for convenience," it
has reintroduced the contradiction design §3.5 exists to kill. Do not (§9.3
gravestone).

### 4.5 Addressing cells — `CellLocation` and slot resolution

Every operation and every view entity needs to name a cell. A location is a
region tag plus a path down the two axes:

```rust
struct CellLocation {
    region: Region,
    path: Vec<PathStep>,
}

enum PathStep {
    Grid { row: usize, col: usize },  // descend the grid axis into a tree's cell
    Field { index: usize },           // descend the field axis into a struct's field
}

enum Region { Rom, Ram, Rim }
```

An empty `path` denotes the region's root cell. Resolution has a wrinkle the v1
model did not: because a grid cell is `Option<CellContent>` and a field cell is
`CellContent`, a path does not resolve to one uniform place type. It resolves to
a **slot**:

```rust
enum CellSlot<'a> {
    Field(&'a CellContent),            // region root, or a struct field cell — never empty
    Grid(&'a Option<CellContent>),     // a tree's grid cell — may be empty
}
// and a CellSlotMut<'a> mirror with &mut.
```

Resolution walks the path, carrying a "current place." The region root is a
`Field`-kind place (`&Document::rom/ram/rim`, all `CellContent`). Each step:

- from a `CellContent` place: a `Field { index }` step requires
  `Value(Struct(inst))` and moves to `&inst.fields[index]` (a `Field`-kind
  place); a `Grid { row, col }` step requires `Tree(grid)` and moves to
  `&grid.cells[row*width+col]` (a `Grid`-kind place).
- from an `Option<CellContent>` place: it must be `Some(content)`; descend into
  `content` and continue as above.

A path that contradicts the data it walks (a `Field` step into a tree, a step
through a `None`) is malformed; resolution treats this as a programming error.
Paths are positional, so a structural mutation can invalidate outstanding ones;
the discipline (§11) is **resolve immediately before acting, rebuild the view
after every mutation** — so a stale path never crosses a frame boundary.

Two derived helpers used throughout:

- `CellSlot::content(&self) -> Option<&CellContent>` — `Field(c)` gives
  `Some(c)`; `Grid(o)` gives `o.as_ref()`. This is how a copy reads a *source*
  and how validation reads a *candidate*.
- writing a copy *payload* (an `Option<CellContent>`, §8.1) into a
  `CellSlotMut`: into `Grid(o)`, `*o = payload`; into `Field(c)`, the payload is
  `Some(p)` — guaranteed, because a field destination that received an empty
  payload would have failed validation (§9.2) — so `*c = p`.

### 4.6 The document resource

```rust
#[derive(Resource)]
struct Document {
    schema: Schema,
    rim_root_field: FieldDef,         // types the Rim root cell (§9.1)
    rom: CellContent,                 // always Tree — the read-only palette
    ram: CellContent,                 // always Tree — untyped scratch
    rim: CellContent,                 // conforms to rim_root_field
    selection: Selection,             // §7
    dirty: RegionMask,                // which regions need a view rebuild (§2.3)
}

#[derive(Default, Clone, Copy)]
struct RegionMask { rom: bool, ram: bool, rim: bool }
```

Holding all three regions in one resource keeps a copy — which touches a source
region, a destination region, and the selection at once — a single borrow.

The three region roots are bare `CellContent` (never `Option`): a region always
has a root cell, and no region root is ever empty. `rom` and `ram` are always
`Tree(_)`; `rim` is whatever `rim_root_field` declares.

`rom` is **rebuilt from `schema`** at construction, never edited and never
saved (§9.5). Its grid holds, in order: one cell per `StructDef` holding that
struct as a variant-0 instance (`Value(Struct(StructInstance { struct_id, variant: 0,
fields: vec![] }))`); one cell holding `Value(Symbol(String::new()))`; one cell
holding a **default tree** — `Tree(Grid { cells: vec![None], width: 1, height: 1 })`.
These are exactly the canonical copy sources design §5.1 and §7.5 require: a
variant-0 struct to clear a struct cell, the empty Symbol to clear a symbol
cell, and — as the single `None` cell *inside* that default tree — an empty
source to clear a grid cell.

`rim_root_field` types the Rim root cell. The Rim root sits in no struct, so no
struct `FieldDef` governs it; instead this document-level `FieldDef` does, and
`rim` relates to `rim_root_field` exactly as `StructInstance::fields[i]` relates
to its variant's `fields[i]`. It is an ordinary `FieldDef` — `elem` and
`is_tree` describe the canvas, `name` is unused by validation. It is chosen at
document creation and, per design §5.3, fixed thereafter (re-selection is design
open question §10). Because it is not derivable from anything, it **is**
captured by snapshots (§8.4), unlike `rom`.

`Default` is deliberately **not** derived for `CellContent`. v1 needed it so an
operation could `mem::take` a cell and leave a well-formed default behind; the
new model has no move and no `mem::take` — copy is clone-overwrite (§8.1) — so
nothing needs `CellContent: Default`. Defaults are explicit constructors:
`default_value(TypeRef) -> CellContent`, `default_tree() -> CellContent`, and
`default_content(&FieldDef) -> CellContent` (a default tree if `is_tree`, else a
default value). None of them needs to consult the schema beyond a `StructId`,
because a struct's default is its variant-0 instance and variant 0 has no fields.

---

## 5. Flatten and semantic equality

Flatten walks the **grid axis** in row-major order and bottoms out at values
(design §4):

```rust
fn flatten<'a>(content: &'a CellContent, out: &mut Vec<&'a LeafValue>) {
    match content {
        CellContent::Value(v) => out.push(v),
        CellContent::Tree(grid) => {
            for cell in &grid.cells {
                if let Some(c) = cell {       // an empty cell contributes nothing
                    flatten(c, out);
                }
            }
        }
    }
}
```

Flatten does **not** descend into a struct's fields: a `LeafValue::Struct` is
*one element* of the flattened sequence, however much field structure it has.
The two recursions — the **grid axis** (`Grid::cells`) and the **field axis**
(`StructInstance::fields`) — are distinct and must never be conflated (design
§3.4). Flatten walks the grid axis only.

The flattened sequence is **semantic identity**; grid shape is **layout
identity** only. Semantic equality: two trees are equal iff their flattens are
equal; two `LeafValue`s are equal when two `Symbol`s have equal strings, or two
`Struct`s have equal `struct_id`, equal `variant`, and pairwise
semantically-equal `fields` (a recursion down the *field* axis). The editor
keeps the full `Grid` (it must, to draw the canvas), but no piece of *meaning*
depends on layout.

---

## 6. The ECS view layer

### 6.1 Rebuilding the view

A system reads `Document::dirty`; for each dirty region it despawns that
region's view subtree and respawns it from the region's root `CellContent`,
then clears the flag. Per visible cell, spawn an entity carrying:

```rust
#[derive(Component)]
struct CellView {
    loc: CellLocation,                // the back-reference into the document
}
```

plus a `Node` for layout, a background, and content-specific children:

- `Value(Symbol(s))` → a `bevy_ui_widgets` text-input child showing `s`;
- `Value(Struct(inst))` → a variant selector, plus one child view per field
  cell (descending the field axis);
- `Tree(grid)` → a `Display::Grid` `Node`, `width` columns by `height` rows,
  one child view per grid cell (descending the grid axis). An empty cell
  (`None`) still gets a view entity — it is selectable and is a valid copy
  source and destination — rendered as an empty slot.

`CellView::loc` is the only document link a view entity needs. The three
highlight states (§7) are presentation state on the entity, applied during the
rebuild from `Document::selection`.

### 6.2 Picking

Picking uses `bevy_picking`: each `CellView` entity carries a click observer.
The observer reads `CellView::loc` and issues the corresponding change — a
selection update (§7), or, for an operation button, an `Operation` passed to
`apply` (§8). Keep observer ordering assumptions out of the design; Bevy does
not guarantee relative ordering of observers for one event.

---

## 7. Selection and highlight

Selection is document state — it must survive view rebuilds, which destroy
entities — so it lives in `Document`, never on entities:

```rust
struct Selection {
    rom: CellLocation,                // the one highlighted Rom cell  (invariant 9)
    rim: CellLocation,                // the one highlighted Rim cell  (invariant 9)
    ram: Vec<CellLocation>,           // the highlighted Ram cells, 0..n (invariant 9)
    superhighlighted: CellLocation,   // the single anchor, any region  (invariant 8)
    red: Vec<CellLocation>,           // destinations of the last failed copy — transient
}
```

The design's three per-cell states (design §6) map on as follows. **Highlighted**
is membership: a cell is highlighted iff it is `rom`, is `rim`, or is in `ram`.
**Superhighlighted** is the single `superhighlighted` field — exactly one cell,
any region, the most recently selected. **Red** is membership in `red`.

`superhighlighted` is stored separately rather than derived because in Ram it
can be a cell that is *not* in `ram` (design §6.2): a successful Ram copy
empties `ram` but leaves `superhighlighted` on its cell, and a Ram toggle-off of
the most-recent cell does the same. In Rom and Rim the two never separate —
there `superhighlighted`, when it points into that region, equals `rom` or
`rim` — but that coincidence is maintained by the selection logic below, not by
the type.

Selection transitions, all performed by the picking observer (§6.2):

- **A pick in Rom or Rim** sets that region's highlight field to the clicked
  cell (replacing the one previous), sets `superhighlighted` to it, **and clears
  `ram`** — selecting a cell outside Ram de-highlights every Ram cell. *(This
  last clause is the design's amendment in `spec-v4`'s change note; it overrides
  the "highlight is persistent across regions" wording of design §6.1 for the
  Ram case.)*
- **A pick in Ram** toggles the clicked cell's membership in `ram`. A
  toggle-*on* additionally sets `superhighlighted` to it; a toggle-*off* does
  **not** move `superhighlighted` (de-selecting is not selecting). Picks in Ram
  do not disturb `rom` or `rim`.
- **Any selection** clears `red`.

Starting state, after the user has chosen the root field's type (§8 / design
§6.4): `rom` and `superhighlighted` both at the Rom root; `rim` at the Rim root;
`ram` empty; `red` empty. Every selection invariant holds immediately.

---

## 8. Operations

Every operation is a plain function over the document:

```rust
fn apply(doc: &mut Document, op: Operation) -> Outcome;

enum Outcome { Applied, Rejected, NoOp }

enum Operation {
    Copy        { source: CellLocation },
    AddRow      { at: CellLocation, side: VSide },   // VSide  = Above | Below
    AddColumn   { at: CellLocation, side: HSide },   // HSide  = Left  | Right
    DeleteRow   { at: CellLocation, side: VSide },
    DeleteColumn{ at: CellLocation, side: HSide },
    SelectVariant { at: CellLocation, variant: usize },
    EditSymbol    { at: CellLocation, text: String },
}
```

A UI button's observer builds an `Operation` and calls `apply`. On
`Outcome::Applied`, `apply` has marked the touched region(s) in `doc.dirty` and
the view rebuilds (§2.3). The v1 `Operation` enum (`ToRam`/`RamToRim`/`Invalid`)
and the cross-region move table are **gone** — see the §9.3 gravestone on
per-region fallibility.

### 8.1 Copy

Copy is the one structural operation. The UI gesture is three-part (design
§7.1): the user selects the destination, presses the copy button — which puts
the editor in a brief **copy mode** — and clicks the source cell, which is what
produces `Operation::Copy { source }`. Copy mode and its abandonment are a UI
concern (design open question §10); `apply` sees only the finished operation.

`apply` for `Copy`:

1. **Determine the destinations** from `selection`, keyed on the region of
   `selection.superhighlighted`:
   - superhighlighted in **Rim** → destinations = `[selection.rim]`;
   - superhighlighted in **Ram** → destinations = `selection.ram.clone()` (may
     be empty);
   - superhighlighted in **Rom** → no destinations (Rom is read-only).
   No destinations → `Outcome::NoOp`.
2. **Read the payload**: resolve `source`, take `slot.content().cloned()` — an
   `Option<CellContent>`. `None` means the source is an empty grid cell; that is
   a legitimate payload (it is how a cell is cleared, §8.4 / design §7.5).
3. **Validate, all-or-nothing.** For each destination, compute its governance
   (§9.1) and validate the payload against it (§9.2). If **any** destination
   fails: write nothing, set `selection.red` to the destination list, return
   `Outcome::Rejected`. The whole-cell red marking (design §6.3) is why
   validation need only return pass/fail — no offending-leaf path (§9.2).
4. **Write.** Every destination passed: into each destination slot, write a
   fresh clone of the payload (§4.5 — into a `Grid` slot the `Option` directly;
   into a `Field` slot the unwrapped `CellContent`, which is present because a
   field destination rejects an empty payload at step 3).
5. **Update selection.** Clear `selection.red`. If the destinations were in
   Ram, clear `selection.ram` (design §6.2, invariant 9); `superhighlighted`
   stays where it is, now possibly un-highlighted. If in Rim, `selection.rim`
   and `superhighlighted` are unchanged.
6. **Mark dirty**: the destination region only. Copy never mutates the source
   (invariant 1 in particular makes a Rom source safe), so the source region is
   not dirtied unless it is also the destination region.

A cell copied onto itself (the source is also a destination) needs no special
handling: the payload was cloned at step 2, and writing that clone back into the
source's own slot is a no-op. It does not, by itself, make the copy fail.

### 8.2 Breadth operations

`AddRow`/`AddColumn`/`DeleteRow`/`DeleteColumn` act on the `Grid` that *contains*
the cell named by `at` — that is, `at.path` must end in a `Grid { row, col }`
step, and the grid acted on is the tree one level up. These are the **only**
operations that change a grid's `width`/`height` (invariant 4), and because each
adds or removes a *whole* row (`width` cells) or *whole* column (`height` cells),
the grid stays rectangular (invariant 3).

- **Add** inserts a row or column of empty (`None`) cells immediately above /
  below / left / right of `at`'s row or column.
- **Delete** removes the *neighbouring* row or column on the given side. A cell
  **cannot delete its own** row or column — only an adjacent one. If there is no
  neighbour on that side (`at` is in row 0 and `side` is `Above`, etc.), the
  operation is `Outcome::NoOp` and the UI should disable the button.

This neighbour-only rule is the mechanism behind invariant 3: `at`'s own row and
column always survive a delete, so a grid can never be reduced below 1×1, and
"an empty tree" never arises (design §7.3). It is settled and load-bearing — not
an open question.

### 8.3 Variant selection and symbol editing

`SelectVariant { at, variant }` — `at` resolves to a `Value(Struct(inst))`. Set
`inst.variant = variant` and rebuild `inst.fields` as one
`default_content(field_def)` (§4.6) per field of
`schema.structs[inst.struct_id].variants[variant].fields`. This is the operation
that maintains invariant 7 for a variant change: the new `fields` has exactly
the new variant's arity, each cell a valid default. Mark the region dirty.

`EditSymbol { at, text }` — `at` resolves to a `Value(Symbol(_))`; replace the
string with `text`. Mark the region dirty.

Both are content edits; neither changes any grid's dimensions (invariant 4).

### 8.4 Undo / redo

Undo/redo is in scope and is cheap, because the document is plain data and the
single source of truth. It is **snapshot-based** to start.

```rust
#[derive(Resource, Default)]
struct History {
    undo: Vec<DocumentSnapshot>,
    redo: Vec<DocumentSnapshot>,
}

struct DocumentSnapshot {
    schema: Schema,
    rim_root_field: FieldDef,
    ram: CellContent,
    rim: CellContent,
    selection: Selection,
}
```

Before any operation that returns `Outcome::Applied`, push a snapshot of the
current document and clear `redo`. A snapshot captures `schema`,
`rim_root_field`, `ram`, `rim`, and `selection` — **not** `rom` (the fixed
palette, rebuilt from `schema`) and **not** `dirty` (a restore marks every
region dirty regardless). `schema` and `rim_root_field` are included so that, if
in-program schema or root-field editing is ever built (design §10), those edits
are undoable too.

- **Undo** — pop `undo`, push the current document onto `redo`, restore the
  popped snapshot, mark every region dirty.
- **Redo** — the mirror.
- A failed copy (`Outcome::Rejected`) mutates no document content; it sets only
  the transient `selection.red`. It is **not** an undo step.
- A pure selection click is **not** an undo step either; `selection` rides
  along inside snapshots so that undoing a *content* operation also restores the
  selection that accompanied it.

Two deferred refinements: **coalescing** consecutive `EditSymbol`s to the same
cell into one undo step (otherwise undo steps per keystroke); and switching from
whole-document snapshots to stored inverses if memory becomes a concern — every
operation in §8 has a clean inverse, so this is tractable later.

---

## 9. Validation, integrity, and persistence

### 9.1 Governance

Validation runs on a copy into a **governed** destination and is skipped for an
**ungoverned** one (design §8.1). Which a destination is — and what governs it —
is computed by walking `to.path` and carrying a governance value:

```rust
enum Governance {
    Field(FieldDef),          // destination is a field cell — a struct field, or the Rim root
    GoverledGridCell(TypeRef),// destination is a grid cell of a governed Tree<elem>
    Ungoverned,               // destination is a grid cell of an ungoverned tree
}
```

The walk starts at the region root:

- region **Rim**, empty path → `Field(rim_root_field.clone())`;
- region **Rom** or **Ram**, empty path → `Ungoverned` (their roots are
  ungoverned trees).

and each step updates it:

- a **`Field { index }`** step (descending into a struct) → `Field(fd)`, where
  `fd` is `schema.structs[struct_id].variants[variant].fields[index]`. Descending
  into a struct always lands in a governed field, in any region — typing is a
  property of the field, not the region (§4.4).
- a **`Grid { row, col }`** step (descending into a tree's grid) depends on the
  governance *before* the step:
  - from `Field(fd)` with `fd.is_tree` → `GovernedGridCell(fd.elem)` — the tree
    is that field's value, and its grid cells carry the field's element type;
  - from `GovernedGridCell(elem)` → `GovernedGridCell(elem)` — a sub-tree on the
    grid axis carries the same element type (it is layout, not a deeper type;
    design §3.4);
  - from `Ungoverned` → `Ungoverned`.

The final governance value is the destination's. `Ungoverned` destinations skip
§9.2 entirely and a copy into them is infallible (invariant 10). Since Rim is
`Field`- or `GovernedGridCell`-governed everywhere and Rom is never a
destination, the only infallible copy is one into an ungoverned Ram cell.

### 9.2 The validation functions

Validation is a structural walk of the payload, inspecting the **kinds** of
values it contains — never a cell type tag, because there are none (§4.4). It
returns a plain pass/fail; the whole-cell red marking (design §6.3) means no
offending-leaf location is needed, so v1's `ValidationError { offending,
expected }` is gone.

```rust
// A copy of `payload` into a destination with this `Governance` is valid iff:
fn validates(payload: &Option<CellContent>, gov: &Governance, schema: &Schema) -> bool {
    match gov {
        Governance::Ungoverned => true,
        Governance::Field(fd) => match payload {
            None => false,                       // a field cell rejects empty (invariant 5)
            Some(CellContent::Value(v)) => !fd.is_tree && value_ok(v, &fd.elem),
            Some(CellContent::Tree(g))  =>  fd.is_tree && tree_ok(g, &fd.elem),
        },
        Governance::GovernedGridCell(elem) => match payload {
            None => true,                        // a grid cell of a tree may be empty
            Some(CellContent::Value(v)) => value_ok(v, elem),
            Some(CellContent::Tree(g))  => tree_ok(g, elem),
        },
    }
}

// every grid-axis leaf of the tree has the element kind `elem`
fn tree_ok(g: &Grid, elem: &TypeRef) -> bool {
    g.cells.iter().all(|cell| match cell {
        None => true,                            // empty contributes no leaf
        Some(CellContent::Value(v)) => value_ok(v, elem),
        Some(CellContent::Tree(sub)) => tree_ok(sub, elem),
    })
}

// a single value matches the element kind
fn value_ok(v: &LeafValue, elem: &TypeRef) -> bool {
    matches!(
        (v, elem),
        (LeafValue::Symbol(_),   TypeRef::Symbol)
      | (LeafValue::Struct(si),  TypeRef::Struct(id)) if si.struct_id == *id
    )
}
```

Three governance cases, one shared leaf core. A **`Field`** destination is
strict about kind: a non-tree field takes a value, a tree field takes a tree,
and neither takes an empty payload. A **`GovernedGridCell`** destination is the
inner rule of a tree field — it accepts a value, a sub-tree, *or* empty, as long
as every leaf matches — which is exactly what makes a `Tree<T>`'s grid cell
"empty, a value, or a sub-tree of T."

### 9.3 Why structs are not recursed; `Any`

`value_ok`, reaching a `Struct`, confirms the `struct_id` and **stops** — it
does not recurse into `StructInstance::fields`. It does not need to: a struct's
fields are always governed (a field is governed in every region, §9.1), so by
invariants 6 and 7 every struct in a live document is already internally
well-formed and well-typed. Validation need only confirm the *top-level*
compatibility of the thing being copied. A **tree**, by contrast, must be walked
(`tree_ok`), because an ungoverned tree carries no such guarantee — its leaves
were never checked. This shortcut is sound only because §9.4 establishes the
invariants for every document that goes live; if a struct could ever enter a
live document unchecked, `value_ok` would have to recurse the field axis.

> **`Any` is not in the code.** There is no `TypeRef::Any`, no `Any` value,
> nothing that "becomes `Any`." `Any` is the *name for the absence of a
> governing field* — for the `Governance::Ungoverned` case, where `validates`
> returns `true` without inspecting anything. "A tree in Ram is a `Tree<Any>`"
> means only that Ram's root tree is ungoverned. Do not give `Any` a
> representation; doing so reintroduces the question "what happens when an `Any`
> value enters a typed field," a question that exists only if `Any` is a thing.

> **Fallibility is per-destination, not per-region.** Do not reintroduce v1's
> region-pair table marking some moves "infallible" and one "fallible." A copy
> is validated iff its destination's governance is not `Ungoverned` (§9.1) —
> a property of the *destination cell*. The only thing the region of the source
> or destination affects is which cells are reachable, never whether a check
> runs.

### 9.4 The integrity check

A document arriving from disk has the right *shape* — it deserializes into
`Schema`, `CellContent`, `Grid`, `StructInstance` — but deserialization does not
verify *indices or arities*. A corrupt, hand-edited, or version-mismatched file
can carry an out-of-range `StructId` or `variant`, a `StructInstance` whose
`fields.len()` disagrees with its variant, a `Grid` whose `cells.len()`
contradicts `width * height`, or Rim content that does not conform to
`rim_root_field`.

Loading is therefore two steps — deserialize, then check:

```rust
fn check_integrity(doc: &Document) -> Result<(), IntegrityError>;
```

`check_integrity` walks the schema and all three regions and confirms the
maintained invariants 2, 3, 5, 6, 7:

- **Schema (invariant 2).** Every `TypeRef::Struct(id)` appearing in any
  `FieldDef` — and in `rim_root_field` — has `id < schema.structs.len()`. Every
  `StructDef` has `variants.len() >= 1` and `variants[0].fields.is_empty()`
  (invariant 7's variant-0 clause).
- **Grids (invariant 3).** Every `Grid` reached anywhere has `width >= 1`,
  `height >= 1`, `cells.len() == width * height`.
- **Struct instances (invariant 7).** Every `StructInstance` has a valid
  `struct_id`, a `variant < schema.structs[struct_id].variants.len()`, and
  `fields.len()` equal to that variant's field count.
- **Fields are non-empty (invariant 5).** Structural — `StructInstance::fields`
  and the region roots are `CellContent`, with no empty case — so there is
  nothing to check at runtime; it is listed only for completeness.
- **Content conforms to type (invariant 6).** Run the §9.2 walk over every
  *governed* part of the document: `rim` against `rim_root_field`, and every
  struct field cell (in any region — struct fields are governed everywhere)
  against its `FieldDef`. Ungoverned cells — the Rom/Ram root trees' grid
  axes — get the structural checks above but no type check, by definition.

A document that fails any check is rejected with a user-facing `IntegrityError`
and never becomes a live `Document`. This check is what *earns* the rest of the
system its right to assume the invariants — it is why §9.2 may confirm a
`struct_id` and stop (§9.3), and why `apply` resolves schema indices directly.
It must also run after any in-program schema edit, should that ever be built
(design §10) — a schema edit is the other way a live document's indices could be
invalidated. `check_integrity` is not an afterthought; it is the load-bearing
guarantee behind every "maintained" invariant in §4.

`rom` is exempt: it is rebuilt from `schema` on every load (§4.6), never
deserialized, so it is correct by construction once `schema` itself has passed.

### 9.5 Serialization and export

The document is plain data, so it derives `serde`'s `Serialize`/`Deserialize`.
Three outputs:

- **Save format — binary.** The working save/load format is `bincode` over the
  `serde` derives. A save file captures `schema`, `rim_root_field`, `ram`,
  `rim`, and `selection`; `rom` is **not** saved — it is rebuilt from `schema`
  on load. Load is: deserialize → `check_integrity` (§9.4) → reject, or rebuild
  `rom` and go live. Binary is canonical because it round-trips the *exact* grid
  shape, which the editor needs to redraw the canvas faithfully (layout is
  storage truth; design §4).
- **Full JSON export — optional.** A feature-gated action serializes the same
  full structure to human-readable JSON via `serde_json` — a one-way export for
  inspection, not a working format. It preserves grid shape.
- **Canvas export — JSON, flattened.** A separate action exports **only Rim**,
  with every tree flattened (§5): a tree becomes a JSON array of its flattened
  values in row-major order; a `Symbol` becomes its string; a struct becomes a
  JSON object recording its `variant` and each field by the same rule
  recursively. A variant-0 struct, having no fields, exports as `{"variant": 0}`
  (design §10 open question 6). This is the *semantic* export — meaning, with
  the editor's layout memory discarded.

---

## 10. UI layout, schedule, and frame flow

**Layout.** Three top-level `Node`s under the UI root — Rom, Ram, Rim — as a
flex row or as resizable panels. A region's tree maps onto a `Display::Grid`
`Node`, `width` columns by `height` rows (the `Grid` is a true rectangular
matrix, §4.2). A `Symbol` cell is the `bevy_ui_widgets` text input; a `Struct`
cell is a variant selector plus a field sub-layout. Variant 0 appears in the
selector under a placeholder label, since it has no name (design open question
§10).

**The three highlight states must be simultaneously distinguishable.** A cell
can be highlighted, superhighlighted, and red at once (design §6), and all eight
combinations must be told apart — so the three cannot all be encoded as
"background colour." Use three independent visual channels (for example: a fill
tint for highlighted, a border for superhighlighted, a distinct error treatment
for red), chosen so no two collide.

**Operation buttons** — copy, the four add and four delete breadth verbs —
rendered on or beside the relevant cell's view, wired to `Operation` values
(§8). Keep label text in one place; the labels are design open question §10.

**Schedule.** The editor needs little global state; a `States` enum is
warranted only if the in-program schema editor (design §10) is built. Frame
flow:

- **Input** — `bevy_picking` observers on `CellView` entities (§6.2) translate
  clicks into selection changes (§7) and `Operation`s.
- **Apply** — `apply` mutates `Document` and sets `Document::dirty` (§8).
- **Rebuild** — a system reads `dirty`, rebuilds each dirty region's view
  subtree (§6.1), clears `dirty`.

Use observers and triggered events for point-in-time things (a click, a
completed operation); use buffered messages only where frame-batching is
genuinely wanted.

---

## 11. Implementation open questions

Distinct from the design spec's §10. With the data model, the operation set, and
undo/redo and serialization now specified, the genuinely open implementation
calls are:

1. **View rebuild granularity.** Whole-region rebuild (§2.3, §6.1) is the
   simple start. Diffing the document against the existing entity subtree, so
   only changed cells respawn, is the optimization — defer it until a region is
   large enough to matter.

2. **`CellLocation` stability.** Paths are positional (§4.5); a structural
   mutation can invalidate outstanding ones. The "resolve immediately before
   acting, rebuild the view after every mutation" discipline keeps a stale path
   from crossing a frame boundary. If a future feature ever needs a cell
   reference to survive across frames, a stable per-cell id would be required —
   but nothing in the current operation set does.

3. **Copy-mode representation.** Copy mode (§8.1) — the interval between
   pressing copy and clicking a source — is editor UI state, not document
   state. Whether it lives in a small `Resource`, a `States` value, or an
   entity component, and how it is abandoned (design open question §10), is
   left to the UI implementation.
