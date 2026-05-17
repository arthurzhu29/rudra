# Rudra — Design Specification

**Version:** 4. Supersedes `spec-v3.md` in full.

**Status:** Design spec. It answers *what the editor is*. A separate
implementation specification answers *how it is built* and is downstream of
this document — where the two ever disagree, this document wins.

This version is a rewrite, not a patch. The data model, the operation set, and
the selection model all changed at once, and the change is a *simplification*:
the move/nest/contents family collapsed to a single `copy`, "empty" stopped
being a state a tree can be in, and structs stopped having half-built states.
Rationale is kept inline wherever a decision is non-obvious, because several
rules below only stay consistent if the reasoning behind them is preserved.

---

## 1. What the editor is

Rudra is an editor for **typed, tree-structured data**. The user assembles a
document by copying pieces from a palette and from a scratch area into a typed
canvas, and by reshaping the 2D layout of those pieces.

There are three regions, always visible:

- **Rom** — a read-only palette. It holds one ready-made instance of every
  primitive the user can build from: the empty form of each defined struct, a
  symbol, and a bare tree. Rom never changes.
- **Ram** — an untyped scratch area. Anything may be assembled here in any
  shape; nothing is type-checked. Ram is where heterogeneous, in-progress, or
  experimental structure lives before it is committed.
- **Rim** — the typed canvas. This is the actual document. Every cell in Rim
  has a declared type, and nothing enters Rim without being checked against it.

The single structural operation is **copy**: the user picks a destination,
presses copy, and picks a source; the source's content is cloned into the
destination. Everything else — clearing a cell, changing a struct's variant,
typing into a symbol, growing or shrinking a grid — is either a copy or one of
a small set of grid-shape operations (§7).

The data the editor manipulates is a **cell tree** (§3). The defining idea is
that **a value's *type* is a property of the slot it sits in, never of the
value itself** (§3.5). That single decision is what makes Ram (untyped) and Rim
(typed) the *same* data model with the type checks simply turned off in one of
them.

---

## 2. Invariants

These are the statements that are true of every valid document at every
quiescent moment — between operations, never mid-operation. Everything in the
rest of this spec is, in effect, a description of how an operation takes a
document satisfying all of these to another document satisfying all of these.
If an operation cannot preserve an invariant, the operation is rejected.

Some invariants are **structural** — guaranteed by the shape of the data model,
so they cannot be violated even in principle. Others are **maintained** — the
data model *can* express a violation, and it is the operations (and a one-time
integrity check, §8.4) that keep it from happening. The distinction is marked
below because the maintained ones are the ones that require ongoing care.

1. **Rom is constant.** No cell in Rom ever changes — not its content, not a
   struct's variant, not a symbol's text. Rom is never the destination of a
   copy. *(Maintained — by the operation rules of §7.)*

2. **Index integrity.** In any live document, every struct identifier, every
   variant index, and every struct-typed field's referent resolves against the
   schema; every grid is well-formed (invariant 3). This is established once,
   at load, and re-checked after any schema edit (§8.4); thereafter every
   operation may assume it. *(Maintained — by §8.4; assumed everywhere else.)*

3. **Grids are rectangular and non-empty.** Every tree's grid has width ≥ 1 and
   height ≥ 1, and holds exactly width × height cells. There is no ragged grid
   and no zero-sized grid. *(Maintained — by the breadth operations of §7.3,
   which are the only operations that change a grid's dimensions, and which add
   or remove only whole rows and whole columns.)*

4. **Grid structure is stable under copy.** Only the four breadth operations
   change any grid's width or height. Copy, variant selection, and symbol
   editing never do. *(Maintained — by the operation definitions of §7.)*

5. **Fields are never empty.** A field declares a type that is either a value
   type or a tree type; in both cases it holds *something*. Emptiness is a
   property only of a *grid cell* (§3.3). A struct field, and the Rim root, are
   never empty. *(Structural — a field's slot has no empty case to express; see
   §3.3.)*

6. **Content matches its declared type.** A non-tree field holds a value of its
   element type; a tree field holds a tree. The Rim root cell conforms to the
   root field. This is exactly what validation (§8) checks on every copy into a
   typed destination. *(Maintained — by validation; §8.)*

7. **Struct instances are well-formed.** Every struct instance is at a valid
   variant and carries exactly that variant's fields, in order. Every struct
   definition has, as variant 0, a nameless variant with no fields (§3.2).
   *(Maintained — by §8.4 at load, and by the variant-selection operation,
   §7.4, thereafter.)*

8. **There is always exactly one superhighlighted cell** — the most recently
   selected cell, in whichever region (§6). *(Maintained — by the selection
   rules of §6.)*

9. **Rom and Rim each have exactly one highlighted cell; Ram has zero or
   more.** After a successful copy whose destinations were in Ram, Ram's
   highlighted set becomes empty. *(Maintained — by §6 and §7.2.)*

10. **The Ram and Rom root trees are ungoverned.** Their cells have no declared
    type, so a copy into such a cell is checked against nothing and cannot
    fail. ("A tree in Ram is a `Tree<Any>`" is a *nickname* for this situation;
    `Any` is not a type — see §8.3.) *(Structural — these roots have no
    governing field; see §5 and §8.1.)*

The maintained invariants are the editor's real surface area. §8.4 is what
establishes them for a document arriving from disk; §7 is what preserves them
for a document being edited. Neither is optional.

---

## 3. The type model

### 3.1 The schema

The set of struct types the user can build with is the **schema** — a registry
of **struct definitions**. A struct definition has a name and an ordered list
of **variants**; a variant has a name and an ordered list of **fields**; a
field has a name, an **element type**, and an **is-tree** flag.

The element type is either **Symbol** or **a struct** (named by its identifier
in the schema). The is-tree flag is a plain boolean. A field is therefore one
of exactly four shapes: a symbol, a struct, a tree of symbols, or a tree of
structs.

Whether the schema is fixed or user-editable in-program is an open question
(§10); this spec assumes a fixed schema and is written so that an in-program
type editor slots in without disturbing anything else.

### 3.2 Variant 0 — the nameless empty variant

Every struct definition has, as its **variant 0**, a variant with **no name and
no fields**. It is a real variant: it appears in the variant selector, the user
can select it, and an instance of it is an ordinary struct instance carrying an
empty field list. It is the struct equivalent of the empty string.

Its purpose is to remove the partial state that earlier versions had. A struct
can always be instantiated — there is always at least one variant to be at — so
there is never a struct "without a variant yet." A struct *defaults* to variant
0. Clearing a struct-typed field means copying a variant-0 instance over it
(§7.5).

### 3.3 Cells, content, and trees

The unit the user sees, selects, and copies is a **cell**. A cell holds
**content**, and content is one of exactly two things:

- a **value** — a Symbol (a string) or a Struct (an instance of a schema struct
  at a chosen variant, carrying one cell per field of that variant); or
- a **tree** — a rectangular grid of cells.

Write the model as **`content = value | tree`**, with **`value = Symbol |
Struct`**.

A **tree** is a rectangular grid (§invariant 3) whose every position is again a
cell. This is the recursion: a cell's content may be a tree, whose cells' content
may be trees, and so on. The recursion bottoms out at values.

Two kinds of cell exist, and they differ in exactly one way — **whether the
cell may be empty**:

- A **grid cell** — a position inside a tree's grid — may be **empty**, or may
  hold content. Emptiness *is* one of its states; a grid cell is "empty, a
  value, or a tree."
- A **field cell** — the slot of a struct field, and the Rim root cell — holds
  content and is **never empty** (invariant 5). It is "a value or a tree."

This asymmetry is the whole of invariant 5, and it is structural: a field cell
simply has no empty state to be in. "Empty" is not a third kind of content
competing with values and trees; it is the absence of content at a grid
position. (The implementation realizes this directly — a grid cell is an
*optional* content, a field cell is a content — so the asymmetry is enforced by
the type system, not by a runtime check.)

A consequence worth stating: a **tree always contains a grid, and that grid
always has at least one cell** (invariant 3). There is no "empty tree." A tree
that you might think of as empty is a tree whose one cell (or whose every cell)
is *empty* — the emptiness is at the cells, never at the tree. This is what
makes a 1×1 tree exactly as rigid as any larger tree: its single cell can be
emptied or overwritten, but the tree, and its grid, remain.

### 3.4 The depth-1 cap, and the two recursions

A field's type can be `Tree<T>` but never `Tree<Tree<T>>`. This is enforced by
the *shape* of a field definition: a field has one is-tree boolean and one
element type, and the element type is `Symbol` or a struct — never a tree.
There is simply nowhere in a field definition to write a second `Tree`.

This does **not** forbid a tree's grid cell from holding another tree. It does
— that is how 2D layout works. But a tree nested inside a tree's grid is the
**grid axis** recursing: it carries the *same* element type as the tree it sits
in, and it is layout, not a deeper type. There are two distinct recursions in
this model and they must never be conflated:

- The **grid axis** — a tree's grid contains cells, whose content may be trees.
  This is the recursion that 2D layout is made of. Flatten (§4) walks this axis.
- The **field axis** — a struct instance contains field cells, whose content
  may be structs. This is a different recursion entirely.

`Tree<Tree<T>>` being unexpressible is a statement about *field types*, on the
field axis. Layout nesting, on the grid axis, is unrestricted and is not a type.

### 3.5 Cells carry no type

A cell has content and nothing else. It has no type tag. A type is a property
of a **field** — of a *slot* — and lives in the schema. A value is checked
against a type only at the moment it is copied into a typed slot (§8).

This is the load-bearing decision of the whole model. Because cells are
untyped, the *same* cell tree can sit in Ram (where no slot governs it, so no
check ever runs) or in Rim (where a slot governs it, so a check ran when it
entered). Ram and Rim are not two data models; they are one data model with
the checks turned off in one of them. Do not add a type to a cell "for
convenience" — it reintroduces the contradiction this decision exists to kill
(§9).

---

## 4. Flatten and semantic equality

A tree's **flatten** is the sequence of values obtained by walking its grid in
row-major reading order and bottoming out at values:

- a value contributes itself, as one element;
- a tree contributes its cells' flattens, in row-major order, concatenated;
- an empty grid cell contributes nothing.

Flatten walks the **grid axis only**. It does **not** descend into a struct's
fields: a struct is **one element** of the flattened sequence, however much
internal field structure it has. (Struct internals are compared on the field
axis, separately — see below.)

The flattened sequence is the **semantic identity** of a tree; the grid shape
is **layout identity** only. Two trees are *semantically equal* exactly when
their flattens are equal. Two values are equal when: two symbols have equal
strings; two structs have equal identifier, equal variant, and — recursing the
field axis — pairwise semantically-equal field cells.

The editor keeps the full grid shape (it must, to draw the canvas), but **no
piece of meaning may depend on layout**. This is why a 1×1 tree holding a value
and that bare value are interchangeable as *meaning* while remaining distinct
as *layout* — and it is why copy needs no separate "nesting" mode (§9): nesting
would only ever change layout, never meaning.

---

## 5. The three regions

Each region is, at its root, a single cell. The three root cells are the
entry points to the three cell trees the document consists of.

### 5.1 Rom — the read-only palette

Rom's root is an **ungoverned tree** (§8.1) whose grid holds:

- one cell per defined struct, each holding that struct as a **variant-0
  instance** — the empty form of the struct;
- one cell holding a **Symbol** with the empty string;
- one cell holding a **default tree** — a bare 1×1 tree whose single cell is
  empty.

Rom is **read-only in the strongest sense**: its cells cannot be edited, cannot
have their variant changed, cannot be typed into, and are never the destination
of a copy (invariant 1). Rom is only ever a copy *source*.

Rom's contents are exactly the things the user needs as copy sources to build
and to *clear*: copying a variant-0 struct over a struct-typed cell clears it;
copying the empty Symbol clears a symbol cell; copying the empty grid cell that
lives inside Rom's default tree clears a grid cell to empty (§7.5). Because Rom
is constant, these canonical sources are always available and always pristine.

### 5.2 Ram — the untyped scratch area

Ram's root is an **ungoverned tree**. It begins as a default tree (a 1×1 grid
with one empty cell). Nothing in Ram's root tree is type-checked: a copy into
an ungoverned grid cell is infallible (invariant 10, §8.1).

Ram is where heterogeneous and in-progress structure is assembled. Because a
typed field needs a *whole* well-formed tree to be copied into it, and copy has
no assembling or nesting mode, Ram is the place that assembling happens: the
user builds a tree cell by cell in Ram, where no check interferes, and then
performs one validated copy of the finished tree into Rim.

Note that "Ram is untyped" applies only to Ram's **root tree**. The moment a
Ram cell holds a *struct*, that struct's fields are governed by the schema
exactly as they would be anywhere — typing is a property of the field, not of
the region (§3.5, §8.1). Ram being untyped means only that Ram's root tree
imposes no element type.

### 5.3 Rim — the typed canvas

Rim's root is a single cell — the document being built. Its type is given by a
**root field**, chosen by the user when the document is created. The root field
is an ordinary field definition (an element type plus an is-tree flag); the
only thing unusual about it is that it is attached to the document rather than
to a struct.

Once chosen, the root field is **fixed for the document's lifetime**. Changing
it is comparable in weight to editing the schema, and is deferred to the same
open question (§10).

Because the Rim root cell is governed by the root field, invariant 6 holds at
the root just as at every other Rim cell: the Rim root cell always conforms to
the root field. Consequently **Rim has no untyped location** — every cell in
Rim is governed (by the root field, or by a struct field, or by the element
type of an enclosing typed tree), so every copy into Rim is validated (§8).

---

## 6. Selection and highlight

The user works by selecting cells. Selection is described by three independent
per-cell states. They are orthogonal: a cell may be in any combination of them
at once, and the user interface must render all eight combinations
distinguishably.

- **Highlighted** — the cell is a member of the current selection set.
  Highlight is *persistent*: it survives until something explicitly changes it.
- **Superhighlighted** — the cell is the *anchor*: the single most recently
  selected cell. There is **always exactly one** superhighlighted cell in the
  whole editor (invariant 8).
- **Red** — the cell was the destination of a copy that failed validation
  (§7.2). Red is *transient*.

### 6.1 Highlight, per region

- **Rom** and **Rim** each have **exactly one** highlighted cell at all times.
  Selecting a different cell *within* that region moves the highlight to it —
  the previous cell loses it. The count is always one.
- **Ram** has **zero or more** highlighted cells. Clicking an unhighlighted Ram
  cell highlights it; clicking a highlighted Ram cell un-highlights it (a
  toggle). The user can thus build up a multi-cell Ram selection, or clear it.

Highlight is persistent in all three regions: selecting a cell in one region
does **not** disturb the highlights of another. (Ram's highlighted set is
cleared only by the one event named in §7.2 — a successful copy into Ram — not
by activity in another region.)

### 6.2 Superhighlight

Superhighlight tracks **focus**: it marks the single cell most recently brought
into focus by a selection. It moves only when a cell is *selected* — a pick in
Rom or Rim, or a toggle-*on* in Ram. It does **not** move on a Ram toggle-*off*
(de-selecting a cell is not selecting one).

Crucially, superhighlight **persists on its cell even if that cell loses its
highlight**. In Rom and Rim this never separates the two states — there is
always exactly one highlighted cell and selecting it is what made it
superhighlighted, so in those regions *superhighlighted implies highlighted*.
In Ram the two come apart: after a successful Ram copy (§7.2) the anchor cell
loses its highlight but keeps its superhighlight, and a Ram cell toggled off
while still the most-recent selection is likewise superhighlighted-but-not-
highlighted. This separation is not a quirk; it is the mechanism by which the
*next* operation knows where Ram's focus is even though Ram's selection set is
empty.

### 6.3 Red

A cell becomes red when it was a destination of a failed copy (§7.2). Red marks
the **whole cell**, never a part of it. It is cleared when the user next makes
any selection, anywhere. There is deliberately no keyboard shortcut to dismiss
red on its own; to clear it, select something.

### 6.4 Starting state

When a document is created the user first chooses the root field's type (§5.3).
The editor then opens with:

- the **Rom root cell** highlighted **and** superhighlighted;
- the **Rim root cell** highlighted;
- Ram with no highlighted cell.

This satisfies every selection invariant immediately: one superhighlighted cell
in total, one highlighted in Rom, one in Rim, zero in Ram.

---

## 7. Operations

The editor has one **structural** operation — copy — together with four
**breadth** operations that reshape grids, and the two ordinary content edits
(choosing a struct's variant, typing into a symbol). "One operation" is true of
the move-family only: the editor still has several buttons, and §7.3–§7.4 are
load-bearing. What collapsed to one is *moving data between cells*.

### 7.1 Copy — the gesture

Copy is a three-part gesture: **select the destination, press copy, select the
source.**

The **destination** is whatever is selected when copy is pressed, and which
cells those are is read from the **superhighlighted** cell's region (invariant
8 guarantees there is one):

- superhighlighted cell is in **Rim** → the destination is that single cell.
- superhighlighted cell is in **Ram** → the destination is **every highlighted
  Ram cell**. (If none are highlighted, the destination set is empty and copy
  does nothing.)
- superhighlighted cell is in **Rom** → Rom is read-only and cannot be a
  destination; copy does nothing.

Pressing copy puts the editor in a brief **copy mode**: the next cell the user
clicks is taken as the **source** and triggers the copy. The source may be in
any region, including Rom and including Rim. The source click is consumed by
the copy — it does not change highlights or superhighlight the way an ordinary
selection click would.

### 7.2 Copy — effect

The source cell's **content is cloned** into each destination cell. Copy is a
pure overwrite: the destination cell keeps its position and its identity, and
its content becomes a clone of the source's. It is *content* that is copied;
the destination's place in its grid or struct is unchanged (invariant 4).

Every copy into a **governed** destination is validated against that
destination's type (§8). A copy into an **ungoverned** destination — a cell on
the Ram root tree's grid axis — is checked against nothing and cannot fail
(invariant 10).

A copy with more than one destination (the Ram case) is **all-or-nothing**: the
source is validated against every destination first; only if *all* succeed is
*any* content written. This keeps the editor from a half-applied state.

- **On success**, the destinations now hold clones of the source content.
  - Rim destination: the cell stays highlighted and superhighlighted.
  - Ram destinations: every highlighted Ram cell is de-highlighted (invariant
    9); the superhighlighted cell stays superhighlighted, now without highlight
    (§6.2).
- **On failure** (any destination's check fails), nothing is written, and every
  cell that was a destination turns **red** (§6.3). The red persists until the
  next selection. Because red marks whole cells, the editor needs only a
  per-destination pass/fail from validation — never a sub-cell location — which
  keeps error handling simple.

A cell copied onto itself (the source is also among the destinations) is a
no-op for that cell and does not, by itself, make the copy fail.

### 7.3 Breadth operations — reshaping grids

From a selected cell inside a tree's grid, the user can:

- **add a row above / add a row below / add a column left / add a column
  right** — insert a whole row (one cell per column) or whole column (one cell
  per row) of empty cells, adjacent to the selected cell, into that grid;
- **delete the row above / below / column left / right** — remove a whole
  *neighbouring* row or column.

These are the **only** operations that change a grid's dimensions (invariant 4).
Because they add and remove only whole rows and whole columns, a grid stays
rectangular (invariant 3).

A cell **cannot delete its own row or column** — only a neighbouring one. This
is not a usability nicety; it is the mechanism that guarantees a grid never
loses its last cell. With only one row, there is no neighbouring row to delete;
likewise for columns. Therefore every grid keeps at least one cell, and "an
empty tree" never arises (invariant 3, §3.3). Earlier drafts left "deleting your
own row" as an open question; in this model it is a settled, load-bearing
prohibition.

### 7.4 Variant selection and symbol editing

A **struct** cell carries a variant selector. Choosing a variant replaces the
struct's field list with the chosen variant's fields, each initialized to its
declared type's default (§7.6). Variant 0 — the nameless empty variant (§3.2) —
is shown in the selector and is selectable like any other; the interface gives
it a placeholder label (it has no name of its own).

A **symbol** cell is a text input; editing it changes the symbol's string. (How
finely such edits group for undo is an implementation concern, not a design
one.)

### 7.5 Clearing — two distinct mechanisms

There is no "delete cell" operation, and the user should not look for one.
There are instead two different kinds of deletion, and they are different
*because* they affect different things:

- **Clearing a cell's content** is a copy. To clear a cell, copy a canonical
  empty over it: a variant-0 struct or the empty Symbol from Rom for a value
  cell, or the empty grid cell from inside Rom's default tree for a grid cell.
  The cell remains; its content becomes empty/default.
- **Deleting structure** is a breadth operation (§7.3): deleting a row or
  column removes cells outright.

A field cell can be *cleared* (copied a default over) but never *deleted* — it
is part of a struct (invariant 5). A grid cell can be cleared to empty, and a
whole row or column of grid cells can be deleted.

### 7.6 Depth and defaults are emergent

There is no operation that "increases depth" or "decreases depth." Depth change
is simply what happens when a copy puts content of one shape where content of
another shape was: copy a tree-bearing cell's content into a cell that held a
value, and that cell is now deeper; copy a value where a tree was, and it is
shallower. The constraint, as always, is the destination's type (§8): a
non-tree field will not accept a tree, and a tree field will not accept a bare
value.

Defaults are likewise uniform: a Symbol defaults to the empty string, a Struct
defaults to its variant-0 instance, a tree defaults to a 1×1 grid holding an
empty cell. Initializing a freshly-variant-selected struct's fields (§7.4) and
initializing the Rim root from the root field (§5.3) both use these defaults.

---

## 8. Validation

### 8.1 Governed and ungoverned cells

A cell is **governed** if some field definition declares its type, and
**ungoverned** otherwise. A cell is governed when it is:

- a **struct field cell** — typed by that field, in any region, Ram included;
- the **Rim root cell** — typed by the root field;
- a **grid cell of a governed tree** — typed by the element type of the field
  whose value that tree is.

A cell is ungoverned exactly when it is a grid cell of an **ungoverned tree**,
and the only ungoverned trees are the **Ram and Rom root trees** (§5). So the
ungoverned cells are precisely the cells reachable from the Ram or Rom root by
grid steps alone, never crossing into a struct.

Validation runs on a copy **into a governed destination**. A copy into an
ungoverned destination is checked against nothing and is infallible (invariant
10). Since Rim is governed everywhere (§5.3) and Rom is never a destination,
the only infallible copy is one into an ungoverned Ram cell.

### 8.2 The rule

A field's type is an element type `E` (a Symbol or a struct) plus an is-tree
flag. A copy of source content `C` into a destination governed by that field is
valid exactly when:

- **the field is not a tree** → `C` is a **value** whose kind matches `E`: a
  Symbol if `E` is Symbol; a struct with the matching identifier if `E` is a
  struct. A tree, or an empty source, is rejected.
- **the field is a tree** → `C` is a **tree**, and **every value reached by
  flattening it on the grid axis (§4) has a kind matching `E`**. A bare value,
  or an empty source, is rejected. (An empty grid cell within the tree
  contributes no value and so is always acceptable.)

That is the entire check. It is structural — it inspects the *kinds* of values,
never any cell-type tag, because there are none (§3.5).

### 8.3 Why structs are not recursed, and what `Any` means

When the check reaches a **struct** — as the source value of a non-tree field,
or as a leaf while flattening a tree — it confirms the **identifier** and stops.
It does **not** recurse into the struct's fields. It does not need to: every
struct in a live document is already well-formed and well-typed by invariants
6 and 7, because a struct's fields are *always* governed (a field is governed in
every region — §8.1) and so were validated when they were filled. The check
only ever needs to confirm the *top-level* compatibility of the thing being
copied; its interior is an invariant, not a thing to re-verify.

A **tree**, by contrast, must be walked, because an *ungoverned* tree carries no
such guarantee — its cells were never checked, so its leaves may be of any
kind. Validating a copy of an ungoverned tree into a typed tree field is exactly
the walk that confirms that heterogeneous scratch content is, in fact,
homogeneous enough for its destination.

This is all that **`Any`** ever meant. There is no `Any` type, no `Any` value,
nothing in the data that "is `Any`." "A tree in Ram is a `Tree<Any>`" is a
*nickname* for "the Ram root tree is ungoverned, so no validation runs on it."
`Any` names the *absence of a check*. Do not give it a representation (§9).

### 8.4 Establishing the invariants — the integrity check

A document arriving from disk has the right *shape* — it deserializes into
cells, trees, and struct instances — but deserialization does not verify
*indices*. A corrupt, hand-edited, or version-mismatched file may carry a
struct identifier, a variant index, or a struct-typed field referent that does
not resolve, or a grid whose cell count contradicts its dimensions.

Loading is therefore two steps: deserialize, then run an **integrity check**
that walks the schema and all three regions and confirms invariants 2, 3, 5, 6,
and 7 — every identifier and variant index resolves, every grid is rectangular
and non-empty, every field cell is non-empty and matches its declared type,
every struct instance is well-formed. A document that fails is rejected with a
user-facing error and never becomes live.

This check is what *earns* the rest of the system its right to assume the
invariants — in particular it is why the validation of §8.2 may confirm a
struct's identifier and stop (§8.3), and why operations may resolve schema
indices without re-checking them. It must also run after any in-program schema
edit, should that ever exist (§10), since a schema edit is the other way a live
document's indices could be invalidated. The integrity check is not an
implementation afterthought; it is the load-bearing guarantee behind every
"assumed" in §2.

---

## 9. Rejected alternatives — do not reintroduce

Each of these was tried, in this design's history or its predecessors, and
removed. Each one, reintroduced, rebuilds a contradiction the current model was
shaped to avoid.

> **An "empty" state for trees, or an empty content kind.** Content is `value |
> tree` — two kinds, not three. Emptiness belongs to *grid cells* and lives
> there as the absence of content (§3.3). Do not add an `Empty` content kind,
> and do not let a tree be empty by giving it a zero-sized grid: a tree always
> has a grid of at least one cell (invariant 3). The earlier zero-sized-grid
> and `Empty`-variant models both made "empty" a state the editor had to branch
> on everywhere; making it the absence of content at a cell removes the branch.

> **An `Any` type.** There is no `Any` element type, no `Any` value, no cell
> that "becomes `Any`." `Any` is the *name for the absence of a validation
> call* on an ungoverned cell (§8.3). Giving it a representation reintroduces
> the question "what happens when an `Any` value enters a typed field" — a
> question that only exists if `Any` is a thing, and which §8 answers precisely
> *because* it is not.

> **A move operation, a "nest" mode, a "contents" payload, or a self/children
> source axis.** The structural operation is **copy**, and only copy. Earlier
> designs distinguished move from copy, distinguished replacing a cell from
> placing inside it, distinguished moving a cell from moving its contents or its
> children. Once a tree is always a rigid grid (§3.3) every one of those
> distinctions collapsed — they produced either the same result or an
> ill-formed one. Do not reintroduce them; their disappearance is a theorem of
> the model, not an oversight.

> **A type tag on cells.** A cell carries content and nothing else. Typing is a
> property of fields, in the schema (§3.5). A `Cell::type` field "for
> convenience" makes Ram and Rim two data models instead of one and reintroduces
> the contradiction §3.5 exists to kill.

> **`Tree<Tree<T>>`.** The depth-1 cap is structural (§3.4): a field has one
> is-tree flag and an element type that is never itself a tree. Layout nesting
> on the grid axis is *not* a deeper type and is not restricted.

> **Fallibility framed per region pair.** Earlier specs carried a table of
> region-to-region moves marking some "infallible" and one "fallible."
> Fallibility is a property of the **destination cell** — whether it is
> governed (§8.1) — not of a region pair. A copy into *any* governed cell is
> checked; a copy into an ungoverned Ram cell is not. Do not restore the table.

---

## 10. Open questions

These are genuine, deliberately unresolved. The spec is written so that any
resolution slots in without disturbing what is settled.

1. **In-program schema editing.** This spec assumes a fixed schema. Allowing the
   user to define and edit struct types in-program is a coherent extension; it
   would make the integrity check (§8.4) a thing that also runs after a schema
   edit, and it would interact with undo. Deferred.

2. **Re-selecting the Rim root field.** Changing a document's root type after
   creation is comparable in weight to schema editing, and is deferred with it
   (§5.3). For now the root field is fixed once chosen.

3. **Operation labels.** The user-facing names of the breadth operations, and
   of copy, are not fixed here. Keep the label text in one place so renaming is
   cheap.

4. **The variant-0 placeholder label.** Variant 0 has no name (§3.2); the
   variant selector must show it as *something*. The exact placeholder is a UI
   detail left open.

5. **Abandoning copy mode.** Pressing copy enters a mode awaiting a source click
   (§7.1). Whether — and how — the user can abandon that mode without copying
   (a second press of copy, an escape key, a click on empty space) is left
   open. Note that the escape key is deliberately *not* a way to clear a red
   destination (§6.3); to clear red, make a selection.

6. **Canvas export of a variant-0 struct.** A variant-0 struct instance has no
   fields; on a semantic export it is expected to emit just its variant marker
   (an object recording variant 0, with no field entries). The exact export
   shape for the empty variant is left to the export spec.


*** changes ***

selecting a cell outside of ram will deselect / unhilight all previously
hilighted cells in ram.