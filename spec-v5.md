# Rudra — Design Specification

**Version:** 5. Supersedes `spec-v4.md` in full.

**Status:** Design spec. It answers *what the editor is*. A separate
implementation specification (`impl-spec-v3.md`, to be rewritten alongside this
one) answers *how it is built* and is downstream of this document — where the
two ever disagree, this document wins.

This version inverts a chunk of v4. v4 carried a clean conceptual model in
which "empty" was the absence of content at a grid cell, struct fields lived
on a separate "field axis," and only `copy` was a real operation. The
implementation arrived at a different settlement: emptiness is its own content
kind, struct fields live inside an ordinary grid as a kind of cell that
carries its slot's identity, and `move` exists as a peer to `copy`. The
implemented model is simpler in the places day-to-day code lives — there is
one cell enum, one grid type, one rendering path — at the price of moving
several invariants from *structural* (the data model cannot express the
violation) to *maintained* (a runtime predicate must keep them true). v5 is
that settlement, written down. Rationale is kept inline wherever a decision
inverts v4 or is otherwise non-obvious, because several rules below only
remain consistent if the reasoning behind them is preserved.

---

## 1. What the editor is

Rudra is an editor for **typed, tree-structured data**. The user assembles a
document by copying and moving pieces between a palette, a scratch area, and a
typed canvas, and by reshaping the 2D layout of those pieces.

There are three regions, always visible:

- **Rom** — a read-only palette. It holds one ready-made instance of every
  shape the user can build from: every variant of every defined struct, a bare
  symbol, and a bare tree. Rom never changes.
- **Ram** — an untyped scratch area. Anything may be assembled here in any
  shape; nothing is type-checked. Ram is where heterogeneous, in-progress, or
  experimental structure lives before it is committed.
- **Rim** — the typed canvas. This is the document being built. Every cell in
  Rim is governed by some declared type, and nothing enters Rim without being
  checked against it.

The structural operations are **copy** and **move**. Copy clones a source
cell's content into a destination cell; move does the same but additionally
empties the source. Everything else — clearing a cell, changing a struct's
variant, typing into a symbol, growing or shrinking a grid — is one of those
two operations, or one of a small set of grid-shape operations (§8.3), or one
of the two content edits (§8.4–8.5).

The data the editor manipulates is a **cell tree** (§4). The defining idea
remains v4's: **a value's type is a property of the slot it sits in, never of
the value itself.** That decision is what makes Ram (untyped) and Rim (typed)
the *same* data model with the type checks simply turned off in one of them.

Where v5 departs from v4 is in *how the data shape realises the model*. v4
made several of the model's properties structural; v5 lets the data shape be
more permissive and asks a single validation predicate (§9) to defend the
properties. The motivation is uniformity in the implementation: every cell
goes through one `Cell` enum, every grid goes through one `Tree` type, every
operation walks one `CellPath`. The trade is recorded honestly in §4 and §11.

---

## 2. Invariants

These are the statements that are true of every valid document at every
quiescent moment — between operations, never mid-operation. Every operation is
a function whose precondition and postcondition are "all of these hold." If an
operation cannot preserve them, the operation is rejected.

Some invariants are **structural** — guaranteed by the shape of the data
model, so they cannot be violated even in principle. Others are
**maintained** — the data model *can* express a violation, and it is the
operations and the validity predicate (§9.1) that keep it from happening. v5
has more maintained invariants than v4 did; this is the cost of the data-model
simplifications discussed in §4.

1. **Rom is constant.** No cell in Rom ever changes — not its content, not a
   struct's variant, not a symbol's text. Rom is never the destination of a
   copy or a move. *(Maintained — by the operation rules of §8.)*

2. **Index integrity.** In any live document, every struct identifier, every
   variant index, and every struct-typed field's referent resolves against the
   schema; every grid is well-formed (invariant 3). The integrity check that
   would establish this for a document arriving from disk is deferred — see
   §10.1; for now this is assumed of every document the editor handles.
   *(Maintained — by §10.1 once wired in; presently assumed.)*

3. **Grids are rectangular and non-empty.** Every grid has width ≥ 1 and
   height ≥ 1, and holds exactly width × height cells. There is no ragged grid
   and no zero-sized grid. *(Maintained — by the breadth operations of §8.3,
   which are the only operations that change a grid's dimensions, and which
   add or remove only whole rows and whole columns; and by the prohibition
   against deleting one's own row or column.)*

4. **Grid shape is stable under copy and move.** Only the four breadth
   operations change any grid's width or height. Copy, move, variant
   selection, and symbol editing never do. *(Maintained — by the operation
   definitions of §8.)*

5. **Field cells are never `Empty`.** A field cell — a `Cell::Field` — always
   carries a value. Emptiness, encoded as `Cell::Empty`, may only appear as a
   cell in a free grid (a grid of an ordinary tree, including the Ram and Rom
   roots). *(Maintained — by §9: any operation that would leave a field cell
   empty fails validation.)*

6. **Field cells live only inside their struct's grid.** A `Cell::Field` may
   appear only as one of the cells of the grid belonging to the struct whose
   `(struct_id, variant_id)` it carries, and only with a `field_id` valid for
   that variant. Every declared field of the chosen variant appears exactly
   once in that grid; the remaining cells in the grid are `Empty`. *(Maintained
   — by §9.)*

7. **Field content matches its declared type.** A non-tree field holds a value
   of its declared element type; a tree field holds a tree whose every value,
   along the grid axis, has a kind matching the element type. The Rim root
   cell conforms to the Rim's root field, declared on the schema (§3.4). The
   Ram and Rom root trees are ungoverned and impose no element type on their
   cells. *(Maintained — by §9.)*

8. **Selection is single-anchor.** Exactly one cell in the editor is
   *focused* — the most recently selected cell. The previous focus moves to
   the newly clicked cell. Aside from focus, the editor carries one transient
   marker — the *rejected* cell, set when a copy or move fails validation, and
   cleared by the next cell click (§7). *(Maintained — by §7.)*

A note on what is *not* an invariant here that was one in v4. v4's "every
struct definition has, as variant 0, a nameless empty variant" is gone. Struct
definitions carry whatever variants their author wrote, in any order, with any
names and any fields. Schemas must in practice have at least one variant per
struct or instantiation will panic (`default_struct_variant` indexes
`variants[0]`); this is a known sharp edge tolerated for now, noted in §10.7.

---

## 3. The type schema

### 3.1 What the schema is

The set of struct types the user can build with is the **schema** — a
registry of **struct definitions**. A struct definition has a name and an
ordered list of **variants**. A variant has a name and an ordered list of
**fields**. A field has a name, an **element type**, and an **is-tree** flag.

The element type is either **Symbol** or **a struct** (named by its index in
the schema). The is-tree flag is a plain boolean. A field is therefore one of
exactly four shapes: a symbol, a struct, a tree of symbols, or a tree of
structs. In the code this is the `Types`, `StructDef`, `VariantDef`,
`FieldDef`, and `CellValue` triad — the names are awkward (the field is
called `types: Vec<StructDef>` on a struct *also* named `Types`); v5 keeps
those names because they are what the code uses, and notes in §10 that
renaming them to `Schema { structs }` and `TypeRef` is a worthwhile cleanup.

### 3.2 No special status for any variant

In v4, variant 0 of every struct was required to be the nameless empty
variant — the canonical "empty form" of the struct, available everywhere as a
cleared-struct source. v5 removes this. A schema declares variants however its
author wants them; the editor does not look for, prefer, or enforce a
particular variant index.

A consequence: there is no longer a single canonical "empty struct" to copy
over a struct cell to clear it. Clearing a struct cell, if the user wants
that, is done by copying *some* struct instance over it — typically variant 0
because Rom's per-row layout puts variant 0 first, but this is convention, not
mechanism. If a struct's author wants a designated "empty" variant, they
declare one explicitly and place it wherever they like in the variants list.

The rationale for removing v4's rule is partly that nothing in the
implementation depended on variant 0 having special semantics, and partly that
mandating one structurally was awkward (it required either a separate
"empty-variant slot" in `StructDef` or a maintained invariant whose
enforcement added complexity for a property nothing in the rest of the system
needed). With the rule gone, every variant is treated the same way by every
operation.

### 3.3 The depth-1 cap on field types

A field's type can be a tree of values, but never a tree of trees. This is
**structural**: a `FieldDef` carries exactly one is-tree flag and one element
type, and the element type is either `Symbol` or a struct — never a tree.
There is simply nowhere in a field definition to write a second tree.

This does *not* forbid a grid cell from holding a tree whose cells hold
trees — that is how 2D layout works. Layout nesting on the grid axis is
unrestricted. The depth-1 cap is a statement about *field types*, not about
runtime cell layout. v5 drops v4's "two recursions" vocabulary that framed
this contrast (see §11.2 for the explicit reversal); the cap survives the
vocabulary change because it is enforced by the shape of `FieldDef`, not by
the framing.

### 3.4 The Rim's root field lives on the schema

The Rim root cell's declared type is given by a single `FieldDef` stored on
the schema itself, as `Types.rim`. This locates a piece of per-document state
on the shared schema, which is conceptually a coupling — the schema is the
type system, the root field is a property of a particular document. The code
keeps them together because the editor presently runs one document at a time
under one schema, and the coupling has no consequences while that holds. §10.3
flags this as a cleanup that becomes interesting if the editor ever holds
multiple documents.

---

## 4. Cells, content, and grids

### 4.1 The cell

The unit the user sees, selects, and copies is a **cell**. Every cell in the
data model is a value of one enum, `Cell`:

```
Cell ::= Symbol(String)
       | Struct(StructVal)
       | Tree(Tree)
       | Empty
       | Field(FieldVal)
```

Five kinds. Three of them — `Symbol`, `Struct`, `Tree` — are the content
kinds, the things a cell can "be." Two of them — `Empty` and `Field` — are
*positional* kinds that record information about *where* a cell sits:

- **`Empty`** is what an empty grid cell looks like. A grid cell may be
  `Empty`; a field cell may not.
- **`Field`** is what a slot inside a struct's grid looks like. It wraps the
  field's actual content and carries `(struct_id, variant_id, field_id)` —
  the identity of the field-slot it occupies — alongside that content.

These two — `Empty` and `Field` — are the load-bearing departure from v4 (see
§11 for the inversion). The remainder of this section justifies them.

### 4.2 Why `Empty` is its own cell kind

v4 made emptiness the *absence* of content at a grid position: a grid cell
was `Option<CellContent>`, with `None` for empty and `Some(...)` for filled.
A field cell was a bare `CellContent`, which had no empty case, so "field
cells are never empty" became a property the type system enforced for free.

v5 makes `Empty` an ordinary `Cell` variant, with the rule that it may only
appear as a *grid* cell (a cell inside a `Tree`'s contents), never as a field
cell. The rule is maintained by §9 rather than enforced by the type. The
gains:

- One uniform cell type. Every grid cell is a `Cell`, every field cell is a
  `Cell`, every operation walks one `CellPath` to a `Cell`, every rendering
  path consumes a `Cell`. The implementation does not branch on "is this an
  optional cell or a field cell?" at every step.
- Trivial `Default`. `Cell::default()` is `Cell::Empty`. This is what
  `mem::take` produces when a move blanks a source; it is also what the
  breadth operations (§8.3) fill new rows and columns with. Without an
  `Empty` variant these would each need their own little ceremony.
- One indexable container. A `Tree`'s contents is a `Vec<Cell>` for both
  free trees and struct grids. The same `Tree` type, the same `IndexMut`, the
  same `add_row` / `delete_column`, the same rendering layout (§spawn_cell).

The cost is honest: invariant 5 ("field cells never `Empty`") is now
maintained, not structural. The validity predicate of §9 contains the line of
code that defends it. v4's framing — that this rule should be impossible to
violate — is rejected here in favour of a uniform data model with a single
rule-enforcing pass.

### 4.3 Why `Field` is its own cell kind

In v4, a struct instance held its fields as a `Vec<CellContent>` indexed by
field number — a direct, type-axis container with no positional cells, no
empties between fields, and no 2D layout for the struct's contents. v5 takes
the other choice: a struct instance holds an `Option<Box<Cell>>` *grid* (a
`Cell::Tree`), and the cells of that grid are `Cell::Field` cells, one per
declared field, possibly interspersed with `Cell::Empty` cells the user has
added by inserting rows or columns.

Each field cell carries `(struct_id, variant_id, field_id)` so that the
struct's grid — which is allowed to be 2D and may have empties scattered in —
unambiguously associates each cell with the field it fills. The empties are
necessary for the breadth operations to apply to struct grids the same way
they apply to free grids: a user can grow a struct's grid into multiple rows
or columns for layout purposes, and the resulting empties carry no semantic
meaning.

The gains parallel §4.2:

- One grid type. A `Tree` is a `Tree`, whether it sits in Ram, Rim's root, or
  inside a `Struct`. Width, height, contents — same shape, same indexing.
- One set of breadth operations. `add_row_above`, `delete_column_left`, and
  their siblings work on struct grids and free grids without distinguishing;
  the safeguard against losing a field is the field-cell guard inside the
  deletion ops (§8.3), not a separate code path.
- One rendering path. Every cell on screen is the visualisation of a `Cell`;
  there is no separate "field-row" widget.

The cost: invariant 6 ("field cells live only in their own struct's grid,
exactly one per declared field") is maintained, not structural. The validity
predicate of §9 spends most of its lines defending invariant 6.

A second cost is conceptual: a `Field` cell carries identity information
(struct/variant/field IDs), which is a kind of *tag* on a cell. v4 §9 listed
"a type tag on cells" among rejected alternatives. v5 reintroduces a *slot*
tag — not a type tag, in the sense that nothing about a cell's content
changes because of it — but the line is thin. The honest reading is that v5
chose code uniformity over the cleaner conceptual story of v4.

### 4.4 Grids

A grid is `Tree { contents: Vec<Cell>, width: usize, height: usize }`,
rectangular and non-empty (invariant 3), cells in row-major order. The
content of a tree cell — `Cell::Tree(t)` — is the grid itself; there is no
separate "tree wrapper" structure.

The two flavours of grid — *free* grids (anywhere inside a tree, including
the Ram and Rom root trees) and *struct* grids (inside a `Cell::Struct`) —
differ only in what their cells may be:

- A **free grid** may contain any cell *except* `Field`. Its cells may be
  `Empty`, `Symbol`, `Struct`, or `Tree`. If the free grid is *governed* (it
  is the value of a tree-typed field — §9.1), the non-`Empty` cells must
  match the element type; otherwise (Ram and Rom root trees) anything goes.

- A **struct grid** contains exactly one `Field` cell for every declared
  field of the struct's chosen variant; every other cell is `Empty`. The
  field cell's `(struct_id, variant_id, field_id)` must match the enclosing
  struct's identity.

Both rules are maintained by §9.

### 4.5 Cells carry no *value* type

A cell has content and nothing else (taking the `Field`-tag question of §4.3
into account: a `Field` cell carries slot identity, but the *content* it
wraps carries no type tag). A value's type lives on the field — in the
schema — and a value is checked against a type only at the moment it is
copied into a typed slot.

This is what makes Ram and Rim one data model with the checks turned off in
one of them, and v5 keeps it from v4. Do not add a `type` field to `Cell`
"for convenience"; it reintroduces the contradiction §4.5 exists to kill.
Field cells are an exception that proves the rule — they carry *location*,
not type — but extending them to carry their content's type would cross the
same line.

---

## 5. Flatten and semantic equality

A tree's **flatten** is the sequence of values obtained by walking its grid
in row-major reading order:

- a `Cell::Symbol(s)` contributes the symbol `s` as one element;
- a `Cell::Struct(sv)` contributes the struct, as one atomic element (its
  grid is **not** descended into);
- a `Cell::Tree(t)` contributes its grid's flatten, in row-major order,
  concatenated;
- a `Cell::Empty` contributes nothing;
- a `Cell::Field` does not arise in a free grid (invariant 6); if it did, it
  would not be flattened in the tree sense.

The flattened sequence is the **semantic identity** of a tree; the grid shape
is **layout identity** only. Two trees are *semantically equal* exactly when
their flattens are equal.

Semantic equality of cells, more broadly:

- Two `Symbol(s)` cells are equal iff `s` is the same string.
- Two `Struct` cells are equal iff their `struct_id`, `variant_id`, and
  fields' contents (compared field-by-field, by `field_id`, *not* row-major)
  are all equal.
- Two `Tree` cells are equal iff their flattens are equal.
- Two `Empty` cells are equal.
- Two `Field` cells are equal iff their slot identities and their inner
  values are equal.

A struct's grid is therefore a layout-only artifact for the user: its 2D
shape, the positions of empties between field cells, and which row each
field cell sits in — none of that affects semantic equality of struct cells,
because equality of struct cells goes by `field_id`. v4's "two recursions"
framing made this same point by separating the grid axis from the field
axis; v5 makes it by saying *structs are compared by field, trees by
flatten*. The vocabulary is gone (§11.2), the consequence is the same.

---

## 6. The three regions

Each region is, at its root, a single cell. The three root cells are the
entry points to the three cell trees the document consists of.

### 6.1 Rom — the read-only palette

Rom's root is an **ungoverned tree** (§9.1) — a `Cell::Tree` whose grid
holds one ready-made source for every shape the user might want to build
with:

- one cell per *(struct, variant)* pair, each holding that struct at that
  variant — every variant of every struct in the schema, not just variant 0;
- one cell holding a **default Symbol** (the empty string);
- one cell holding a **default tree** — a bare 1×1 grid whose single cell is
  `Empty`.

The grid is laid out with one row per struct (containing that struct's
variants in order, left-padded with empties to a uniform width), followed by
two single-cell rows for the default tree and the default symbol. The
specific layout is incidental to the design; what matters is that every
buildable shape is reachable as a copy source.

Rom is **read-only in the strongest sense**: its cells cannot be edited,
cannot have their variant changed, cannot be typed into, and are never the
destination of a copy or a move (invariant 1). Rom is only ever a source.

### 6.2 Ram — the untyped scratch area

Ram's root is an **ungoverned tree**, beginning as a default tree (a 1×1
grid with one empty cell). Nothing in Ram's root tree is type-checked: a
copy or a move into an ungoverned grid cell is type-infallible (its only
remaining failure mode is the invariants of §9 that apply *everywhere* —
e.g. a `Field` cell may not land in a free grid).

Ram is where heterogeneous and in-progress structure is assembled. Because a
typed field needs a *whole* well-formed tree to be copied into it, and copy
has no incremental "build into a field" mode, Ram is the place that
assembling happens: the user builds a tree cell by cell in Ram, where no
type check interferes, and then performs one validated copy of the finished
tree into Rim.

"Ram is untyped" applies only to Ram's *root tree*. The moment a Ram cell
holds a struct, that struct's fields are governed by the schema exactly as
they would be anywhere — typing is a property of the field, not of the
region. Ram being untyped means only that Ram's root tree imposes no element
type on its direct cells.

### 6.3 Rim — the typed canvas

Rim's root is a single cell — the document being built. Its type is given by
the **root field**, declared on the schema as `Types.rim` (§3.4). Once a
schema is in use, the root field is fixed for the document's lifetime.

Because the Rim root cell is governed, every cell in Rim is governed in turn
— by the root field, by a struct field, or by the element type of an
enclosing typed tree — so every copy or move into Rim is validated.

---

## 7. Selection

The user works by selecting cells. Selection is a single-anchor model:

- **Focus.** Exactly one cell is *focused* at any time (or none, only before
  the first click of a session — invariant 8). Clicking any cell, anywhere
  in any region, moves focus to it; the previous focus is forgotten.
- **Rejection.** A copy or move whose validation fails sets a *rejected*
  marker on its blamed cell — typically the destination, sometimes the
  source (§8.2). The rejected marker is transient: the next cell click
  anywhere clears it.

There are no multi-cell selections. The Ram-as-multi-select idea of v4 is
not part of v5; if the user wants to perform an operation on several Ram
cells, they perform it several times. The selection model in v5 is what
the implementation already does, written down.

The editor also carries a transient *mode*: between pressing Copy or Move
and clicking the second cell, the editor is waiting for the source or the
destination of that gesture. The mode-state is internal — see §8.1 and §8.2
— but it is rendered to the user as a distinctive highlight on the anchor
cell of the pending operation. The visual presentation is impl-spec
territory (§impl-v3); from the design's perspective the mode is just "the
copy/move gesture is in flight."

---

## 8. Operations

The editor has two **structural** operations — copy and move — together with
four **breadth** operations that reshape grids, and two ordinary content
edits (choosing a struct's variant, typing into a symbol).

### 8.1 Copy

Copy is a two-click gesture: **focus the destination, press Copy, click the
source.** The destination is whatever cell is focused when the user presses
Copy; the editor enters a brief copy-mode awaiting the source click; the
next cell click anywhere is taken as the source, and the copy is performed.

The source's content is **cloned** into the destination cell. The
destination keeps its position and its identity, and its content becomes a
clone of the source's. Position in a grid, struct, or schema is unaffected
(invariant 4).

The copy is **validated** (§9). Every copy into a governed destination is
checked against that destination's declared type; a copy into an ungoverned
destination (a cell of the Ram or Rom root tree) is checked against nothing.
Validation is uniform: the change is applied to a clone of the document,
the predicate `is_valid` is run on the result, and only if it accepts is
the change committed.

- **On success**, the destination now holds a clone of the source.
- **On failure**, nothing is written. The destination is flagged
  *rejected* (§7), and rendered red until the next cell click.

A cell copied onto itself is a no-op for that cell and does not, by itself,
fail.

Rom can be a source but never a destination (invariant 1).

### 8.2 Move

Move is a two-click gesture: **focus the source, press Move, click the
destination.** The source's content is moved into the destination; the
source cell becomes `Empty`. The destination is overwritten — whatever was
there is replaced, including a struct-grid field cell, in which case
validation will reject the move (a struct grid must retain its field
cells).

Move is structurally `copy + clear-source`: it is the same one-step
trial-clone-and-validate as copy, with the additional rule that the source
must be a tree cell (since only tree cells may legally become `Empty` —
invariant 5). A move whose source is anywhere a tree cell is not — for
example, a `Field` cell — is rejected before the validity walk runs,
because emptying the source would violate invariant 5 structurally and
there is nothing to learn from running the walk.

Validation is otherwise the same uniform `is_valid` predicate. On failure,
the offending cell is flagged rejected — typically the destination, but the
source if the source's own eligibility is what made the move impossible
(Rom source, non-tree source).

**Why move is a peer of copy rather than a UI affordance over `copy +
clear`.** The semantics are clean either way. The argument for keeping move
as a structural operation is user-feel: "take this and put it there" is a
single act in the user's mind, and a two-click move gesture renders that
intention directly. The implementation cost is small — `mova` is forty
lines next to `copy`, sharing every line of validation — and the UI cost is
a button. v4 §9 rejected move on the grounds that it always reduced to copy
plus a clear of the empty form over the source; in v5 the reduction would
require an emptying source for *every* type (an empty grid cell from Rom's
default tree, when the source is a grid cell), then a copy, both wrapped in
the same all-or-nothing validation, and the resulting code is longer and
less direct than just having `mova`. The v5 settlement is that the cleaner
conceptual story doesn't actually save anything once trial-clone-and-validate
exists, and move is what the user actually thinks they are doing.

Rom is never source or destination of a move.

### 8.3 Breadth operations

From a focused cell inside a tree's grid, the user can:

- **Add row above / below** — insert a row of `Empty` cells, one per column,
  immediately above or below the focused cell's row, into that grid.
- **Add column left / right** — insert a column of `Empty` cells, one per
  row, immediately to the left or right of the focused cell's column, into
  that grid.
- **Delete row above / below** — remove the neighbouring row.
- **Delete column left / right** — remove the neighbouring column.

These are the **only** operations that change a grid's dimensions (invariant
4). Because they add and remove only whole rows and whole columns, grids
stay rectangular (invariant 3).

A cell cannot delete its **own** row or column — only a neighbouring one.
With only one row, there is no neighbouring row to delete; likewise for
columns. Therefore every grid keeps at least one cell, and "an empty tree"
never arises.

A row or column containing a `Field` cell is **never deleted**: the delete
operations check and refuse silently rather than removing a field. This
keeps invariant 6 — every declared field present in its struct's grid —
intact. The check lives in the breadth operations themselves (it is not
deferred to the post-hoc validity walk) because a delete that violates
invariant 6 is *structural* about the cell being removed, not about the
overall document; the operation simply refuses to apply.

Breadth operations are not trial-clone-validated: they cannot violate any
invariant the design model expresses (rectangularity is preserved by the
operation; the field-cell guard is checked up front; nothing else changes).
This is in contrast to copy and move, which can produce invalid documents
in many ways and so go through the uniform validate-or-reject path.

### 8.4 Variant selection

A struct cell carries a variant selector (the "Variant+" button in the
current UI cycles through them; future UI may present a picker). Choosing a
variant replaces the struct's field list with the chosen variant's fields,
each initialized to its declared type's default (§8.6).

This operation does not go through the validity check: it produces a fresh
struct at a valid variant with default fields, which is well-formed by
construction.

### 8.5 Symbol editing

A symbol cell is a text input; editing it changes the symbol's string. This
operation does not go through the validity check either: changing a
symbol's string cannot violate any invariant.

(How finely such edits group for undo, and whether typing on a non-symbol
cell does anything at all, are UI concerns, not design concerns.)

### 8.6 Defaults

A Symbol defaults to the empty string. A Struct defaults to its variant-0
instance (the first variant in the variants list; not because variant 0 is
special — §3.2 — but because *some* variant must be picked, and variant 0
is the first one). A tree defaults to a 1×1 grid whose single cell is
`Empty`. Initialising a freshly-variant-selected struct's fields and
initialising the Rim root from the root field both use these defaults.

A struct whose schema declares zero variants cannot be defaulted; the
current implementation panics in this case (`variants[0]` indexes an empty
vector). This is acknowledged as a sharp edge; see §10.7.

---

## 9. Validation

### 9.1 Governed and ungoverned cells

A cell is **governed** if some field definition declares its content's type,
and **ungoverned** otherwise. A cell is governed when it is:

- a **struct field cell** — typed by that field, in any region, Ram
  included;
- the **Rim root cell** — typed by the root field on the schema;
- a **grid cell of a governed tree** — typed by the element type of the
  field whose value that tree is.

A cell is ungoverned when it is a grid cell of an **ungoverned tree**, and
the only ungoverned trees are the **Ram and Rom root trees**. The
ungoverned cells are precisely the cells reachable from the Ram or Rom root
by grid steps alone, never crossing into a struct.

A copy or a move into a governed destination is validated against the
declared type. A copy or move into an ungoverned destination is checked
against nothing of *the destination's* doing, but is still subject to the
universal invariants (a `Field` cell may not appear in a free grid;
rectangularity must hold; etc.). Validation in v5 is uniform: the predicate
that decides is the same whether the destination is governed or not.

### 9.2 The validity predicate

The predicate `is_valid(doc, types)` walks the whole document and returns
whether every invariant of §2 — bar the ones flagged as assumed — holds.
It checks, in order:

- the Rim root cell against `Types.rim`;
- the Ram root as an ungoverned tree;
- the Rom root as an ungoverned tree (it is constant in any live document,
  but the predicate is uniform);

and, for each `Tree`, checks rectangularity (`contents.len() == width *
height`) and walks its cells; and, for each `Struct`, looks up the variant
in the schema, confirms the grid has exactly one `Field` cell per declared
field with matching identities and a valid value, and confirms every other
cell of the grid is `Empty`.

The bulk of `is_valid` is the struct-grid walk — it is what defends
invariants 5, 6, and most of 7. This concentration is the direct
consequence of the v5 data-model choices in §4: a more permissive cell
representation is paid for by a more substantial predicate.

### 9.3 The trial-clone-and-validate pattern

Every copy and every move is implemented as:

1. Apply the change to a *clone* of the document.
2. Run `is_valid` on the clone.
3. If valid, commit (replace the live document with the trial); if not,
   discard the trial and flag the offending cell as rejected.

The advantages of this pattern over a per-operation legality check are
practical:

- The same predicate covers every operation. Adding a future operation —
  paste from clipboard, undo replay, an import path — does not need its own
  legality logic; if it produces a document that passes `is_valid`, it
  succeeds, and if it doesn't, it fails the same way.
- It catches *indirect* violations. Some operations could in principle
  leave the document in a state no single rule names — e.g. an unusual
  source-destination pairing where the source is structurally fine and the
  destination is structurally fine but the combination produces an
  ill-formed nested struct. The whole-document walk catches such cases for
  free.
- `Document` is cheap to clone. Symbol strings dominate cell size, and even
  a deeply nested document clones in milliseconds at scale, which is fine
  for an interactive editor with single-operation latency.

The disadvantage — running validation on the entire document for every
operation — is acceptable at present and is noted in §10.5 as a future
optimisation target if and when it bites.

### 9.4 What the validation rule says

A field's type is an element type `E` (a Symbol or a struct) plus an
is-tree flag. A copy of source content `C` into a governed destination is
**valid** exactly when:

- **the field is not a tree** → `C` is a value whose kind matches `E`: a
  symbol if `E` is `Symbol`; a struct with the matching identifier if `E`
  is a struct. An `Empty`, a tree, or a `Field` cell is rejected.
- **the field is a tree** → `C` is a tree (a `Cell::Tree`), and every value
  reached by flattening it on the grid axis has a kind matching `E`. A
  bare value, an `Empty`, or a `Field` cell as source is rejected. An
  empty grid cell *within* the source tree is fine — it contributes no
  value, by definition.

The check is structural — it inspects the *kinds* of values, never any
cell-type tag, because there are none (§4.5).

### 9.5 Why structs are not recursed, and what `Any` means

When the check reaches a struct — as the source value of a non-tree field,
or as a leaf while flattening a tree — it confirms the **identifier** (and
that the struct's own grid is internally well-formed, which `is_valid`
checks as part of the recursive walk anyway) and stops. The check does not
re-derive the field types of the struct's fields, because every struct in
a live document is already well-formed and well-typed: its fields were
governed when they were filled, and validation ran then.

A tree, by contrast, must be walked: an ungoverned tree carries no such
guarantee — its cells were never type-checked, so its leaves may be of any
kind. Validating a copy of an ungoverned tree into a typed tree field is
exactly the walk that confirms its leaves are, in fact, homogeneous enough
for the destination.

This is the whole of what **`Any`** ever meant. There is no `Any` type, no
`Any` value, nothing in the data that "is `Any`." "A tree in Ram is a
`Tree<Any>`" is a *nickname* for "the Ram root tree is ungoverned, so no
validation runs on it." `Any` names the absence of a check. Do not give it
a representation (§11.1).

---

## 10. Outlook — deferred work and future directions

These are pieces of the design that v5 acknowledges as incomplete, deferred,
or future. The spec is written so that any of them can be added without
disturbing what is settled here.

### 10.1 Integrity check on load

The `is_valid` predicate exists in code and would, on a document arriving
from disk, establish invariants 2, 3, 5, 6, and 7 once and for all (the
remaining invariants are about Rom's constancy and selection, which are
not document state). It is not yet wired into the load path. Currently
`save_load::load` deserialises with `bincode` and unwraps the result; a
corrupt or hand-edited save file panics the editor. This is acceptable for
the present personal-use phase and will not remain acceptable once the
editor is shared.

The natural shape of the load is:

1. `bincode` decode → `Document` (panics or `Err` on shape failure today;
   should be `Err` returned to the user).
2. Run `is_valid(&doc, &types)`; on failure, refuse the document with a
   user-facing error.
3. Only then make the document live.

This is light wiring; the predicate is already written.

### 10.2 In-program schema editing

The schema is presently hardcoded as a Rust expression in `src/custom.rs`.
A text format — JSON, RON, or a small bespoke surface syntax — that the
editor reads at startup is the obvious next step; it would eliminate a
recompile per schema change and let users share schemas without sharing
code. Further out, an in-program type editor (the user defines structs,
variants, and fields through the UI) is the eventual goal. Both are
deferred. The validation, integrity check, and operation set are written
so neither requires architectural change to slot in.

### 10.3 Decoupling the root field from the schema

`Types.rim` puts the Rim's root field on the schema. This is fine while
there is one document per schema, and not fine the moment there are
several. Moving the root field onto `Document` is a small refactor whose
cost is paid once and whose benefit shows up the first time the editor
holds two documents in memory.

### 10.4 Multi-document

The editor today opens one document at a time, against one schema. Multiple
documents — tabs, split views, side-by-side — are coherent and would
mainly require the §10.3 cleanup and a window-management story. The data
model already supports it: the document is a value, not a global.

### 10.5 Validation cost

`is_valid` walks the whole document for every copy and move. At the
present scale this is unmeasurable; at much larger scales it would become
visible. Two cheap improvements are available when needed: (a) localise
the walk to the subtree of the change, since most invariants are local;
(b) cache validation results until a relevant change invalidates them.
Neither is necessary now and both are deferrable until the cost is real.

### 10.6 True picture-zoom

The current zoom is implemented as a `UiTransform.scale` on a per-region
pivot node. This is post-layout in principle, but in Bevy 0.18 it
interacts with text rasterisation and `Overflow::clip` in ways that, at
extreme zoom, break the layout of content-sized cells — the visible
breakdown was severe enough that both `Overflow::clip` layers were
removed to recover stability. The cost is that nothing prevents content
from drawing outside its region column visually; in practice the
implementation's geometry makes this rare, but it is real.

A true picture-zoom — content rendered once at base scale to an offscreen
image, the image then scaled and translated to display — would give
unconditionally stable zoom and bounded regions, at the cost of rewiring
cell interaction (clicking through the image instead of directly on UI
nodes). This is deferred work, conditional on the bounded-region issue
ever becoming visible.

### 10.7 Schema sharp edges

A struct declared with zero variants currently panics on instantiation
(`variants[0]` indexes an empty `Vec`). Defaulting and Rom population
both assume at least one variant exists. The schema does not enforce
this; nor does the integrity check (because both are presently absent).
A schema-validation pass that runs after schema construction — at minimum
checking that every struct has at least one variant — is straightforward
work whose absence is acceptable for the present authored-in-code schema
and unacceptable for a future loaded-from-file or edited-in-program
schema.

### 10.8 Cleanups inside the data model

Two pieces of the present data model would be improved by a renaming
pass:

- `Types` (the schema) has a field `types: Vec<StructDef>` and a field
  `rim: FieldDef`. `Types::types` is awkward; `Schema { structs }` is
  clearer.
- `CellValue` (the element-type enum) is misnamed: it is a type
  reference, not a value. `TypeRef` or `ElemType` reads better.

These are clarity wins, not correctness fixes. They are noted here so
they are not lost as the project grows.

### 10.9 Selection enrichment

The single-anchor focus model of §7 is what the implementation has and is
sufficient for the present editor. Two enrichments are coherent
extensions, deferred:

- **Per-region focus.** Each region keeps its own focused cell; cross-
  region picks do not displace the focus of another region. This matters
  if the user begins to use the regions in a side-by-side way (e.g. with
  Ram as a real workspace).
- **Multi-cell selection in Ram.** v4 described this; v5 dropped it. If
  it returns, the natural place is Ram alone (Rom and Rim still want
  single-cell selection per region), and the natural representation is a
  `Vec<CellPath>` of highlighted Ram cells alongside the global focus.

### 10.10 Persistence formats

Two save paths exist in the code:

- `save_load.rs` — `bincode` binary save/load. Working.
- `serialization.rs` — a human-readable / `txt` export, work in progress.

Both are deferred from the design's perspective. The design's only claim
about persistence is that loading any document must, eventually, run the
integrity check (§10.1); the formats themselves are out of scope here.

---

## 11. Rejected alternatives — and what we kept

Each item below was tried at some point in the design's history or
rejected at the model level, with reasoning that survives in v5. The list
also serves as the record of where v5 *inverted* a v4 rejection — those
inversions are tagged.

### 11.1 An `Any` type

There is no `Any` element type, no `Any` value, no cell that "becomes
`Any`." `Any` is the *name for the absence of a validation call* on an
ungoverned cell (§9.5). Giving it a representation reintroduces the
question "what happens when an `Any` value enters a typed field" — a
question that only exists if `Any` is a thing, and which §9 answers
precisely because it is not. **Kept.**

### 11.2 The "two recursions" framing

v4 §3.4 distinguished the *grid axis* (a tree's grid contains cells whose
content may be trees) from the *field axis* (a struct's instance contains
field cells whose content may be structs). It made the depth-1 cap a
statement about the field axis and let flatten walk only the grid axis.
v5 drops the vocabulary: there is one axis, the grid, and struct
instances *live inside grids* (a struct's fields are the cells of the
struct's grid). The depth-1 cap survives — it is enforced by the shape of
`FieldDef` (§3.3) — but the framing is gone. **Dropped from v5.**

The content of the distinction was not wrong; it was just no longer the
clearest way to talk about a model in which the implementation uses a
single `Tree` type for both struct grids and free grids.

### 11.3 An "empty" state for trees

A tree always has a grid of at least one cell; there is no "empty tree."
A tree you might think of as empty is a tree whose one cell (or whose
every cell) is `Empty`. The zero-sized-grid alternative was tried in
earlier designs and removed: it made "empty" a state every traversal had
to branch on, where moving it to the cell level made it a single
enumerated case. **Kept.**

### 11.4 `Empty` as the absence of content

v4 §9 rejected `CellContent::Empty` as a content kind: emptiness was the
*absence* of content at a grid position, with grid cells typed
`Option<CellContent>` and field cells typed `CellContent`. v5 **inverts
this**: emptiness is `Cell::Empty`, an ordinary cell variant; "field
cells are never `Empty`" is a maintained invariant rather than a
structural one.

The cost is one validity-predicate clause; the gain is one cell type,
one rendering path, one set of breadth operations, a free `Default` for
`mem::take` to use during move, and no `Option`-unwrapping at every
traversal. v4's argument that the structural enforcement was preferable
is honest and was lived in for a while; v5's settlement is that the
predicate-defended version is what the implementation actually wants.
**Inverted.**

### 11.5 `Field` cells with slot identity

v4 §9 rejected any "type tag" on cells; struct fields lived in a
`Vec<CellContent>` indexed by field number, with no positional cells, no
empties between fields, and no 2D layout for the struct's contents. v5
**inverts this**: a struct's fields live inside the struct's grid as
`Cell::Field` cells, each carrying its slot identity `(struct_id,
variant_id, field_id)`. Field cells share the cell type, the grid type,
and the rendering path with every other cell; the struct's grid can be
grown or shrunk by the same breadth operations as any other grid, with
`Empty` cells filling the interstitial space.

This is the most consequential v4 rejection that v5 reverses. The
honest framing is in §4.3: v5 chose uniformity of code over the cleaner
conceptual story v4 carried. **Inverted.**

### 11.6 A move operation

v4 §9 rejected move on the argument that, once trees were rigid grids,
move was always reducible to copy plus a clear of the empty form over
the source. v5 **inverts this**, keeping move as a structural operation
peer with copy. The argument for the reversal is in §8.2: with
trial-clone-and-validate doing the legality work uniformly, move is forty
lines and a UI button, while spelling it out as `copy + clear` requires
a synthesized empty-of-the-right-kind source and the same all-or-
nothing wrapping. The cleaner model didn't actually save anything.
**Inverted.**

### 11.7 A type tag on cells

A cell carries content (and, for `Field`, the location it occupies);
neither a `Cell::type` field nor a redundant type-of-content tag should
be added. The Ram-and-Rim-are-one-data-model decision (§4.5) depends on
it. **Kept.**

The slot tag on `Field` cells is acknowledged in §4.3 as walking close
to this line. The defensible reading is that a slot tag records *where*
a cell sits, not *what kind* its content has — extending it to carry
content's type would reintroduce the contradiction this rule exists to
kill.

### 11.8 `Tree<Tree<T>>` as a field type

The depth-1 cap is structural (§3.3): a `FieldDef` has one is-tree flag
and an element type that is never itself a tree. Layout nesting on the
grid axis is *not* a deeper type and is not restricted. **Kept.**

### 11.9 Fallibility framed per region pair

Earlier specs carried a table of region-to-region copies marking some
"infallible" and one "fallible." Fallibility is a property of the
**destination cell** — whether it is governed (§9.1) — not of a region
pair. A copy into any governed cell is checked; a copy into an
ungoverned Ram cell is not. Do not restore the table. **Kept.**

### 11.10 Selection as document state

v4 / impl-v2 §7 argued that selection must live in `Document` so it
survives view rebuilds (which destroy entities). v5 **softens** this
without rejecting it: in the implementation, selection lives in a Bevy
`Resource` separate from the document, which is similarly view-
rebuild-resilient. The design claim — "selection persists across
rebuilds" — is preserved; the storage location is an implementation
choice that impl-spec-v3 decides. **Not a rejection, a relocation.**

---

*This spec is the v5 settlement. The model it describes is what the
implementation does, written down with the rationale that recovering it
again would take. The places it is uncertain — schema cleanup, integrity
check wiring, multi-document, picture zoom — are §10. The places it
inverted v4 — `Empty` and `Field` as cell kinds, move as a peer of copy,
the two-recursions framing — are §11, with the inversions tagged.*
