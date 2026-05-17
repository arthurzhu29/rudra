# Project Specification

**Status:** Design spec. The core design — the type system, the cell model, the
three regions, and the movement rules — is **closed**. What remains open is
collected in §10: verb *naming*, a few small UI/mechanics calls, the
type-*representation* strategy, and one provisional invariant (§5.4).
**Implementation target:** Bevy 0.18 (covered in a separate document, not here).

This document is meant to be self-contained: read on a blank slate, it should
convey everything needed to understand the project. Rationale is included
inline wherever a decision is non-obvious or was reached by ruling out
alternatives, because the *reasons* are load-bearing — several rules only stay
consistent if the reasoning behind them is preserved.

---

## 1. What this project is

A visual editor for structured data. The user defines their own data types and
builds instances of them on a canvas. The application *is* the editor.

Both the type definitions and the instances are **runtime data**. Types can be
created while the program runs (see §4.1); whether the editor also exposes a
dedicated in-app type creator/editor UI is an open question (§10).

Background: the project originated as a block-wise editor for Rust code, which
is why a region's root can be configured to a specific Struct (§5.3).

---

## 2. Terminology

These names are used precisely throughout. Several of them exist specifically
to avoid collisions, so they matter.

- **Symbol** — the only primitive type. A Symbol is a text box / text input.
  Its value is a string.
- **Struct** — a user-defined type. Every Struct is *simultaneously an enum*:
  it is a named sum type. From here on, "Struct" always means struct-enum;
  there is no separate "enum" concept. (Earlier drafts distinguished them; they
  were unified.)
- **Variant** — one named alternative of a Struct. Each variant carries its own
  text and its own set of fields. A Struct with 0 variants may be *defined* but
  can never be validly *instantiated* (there is no variant to pick). This is
  permitted but currently has no known use.
- **Field** — a named slot inside a variant. A field is **typed** by the
  schema, and that type never changes (unless the schema itself is edited). A
  field's type is exactly `T` or `Tree<T>` (see §3).
- **Tree** — the editor's name for the list/collection type and for the
  runtime cell structure that realizes it. **The word `Vec` is deliberately
  avoided** because it collides three ways: it was the original name for this
  type constructor, Rust's `Vec` is the storage primitive, and the runtime
  structure is tree-shaped. So: the *type* is written `Tree<T>`; the runtime
  thing is a **Cell** / a **cell tree**; Rust's `Vec` keeps its own meaning.
  Note that *semantically* a Tree is a flat list only (see §4.4).
- **Cell** — a runtime value-holder. A Cell is **not typed** — it holds a
  value; it does not carry a type. Every Cell in a tree is itself a Tree (see
  §4). This uniformity is intentional.
- **Cell tree** — the recursive structure a `Tree<T>` instance forms (§4).
- **Rom / Ram / Rim** — the three regions of the editor (§5).
- **`Any`** — *not a type*. `Any` is the name for "this is not being
  type-checked." It describes Ram's behavior, nothing more. It is never a type
  a Cell holds, acquires, or propagates. See §6 — this point is load-bearing.

---

## 3. The type system

There are exactly three ingredients:

1. **Symbol** — the sole primitive (a string).
2. **Struct** — the sole way to build compound types. A Struct is a named sum
   type; each variant is a named product (its fields).
3. **`Tree<T>`** — the sole type constructor. Mechanically it is *not* a
   stackable wrapper but a boolean toggle on a field (see §3.2) — which is why
   it is capped at **depth 1**: `Tree<T>` is allowed, `Tree<Tree<T>>` is **not**
   a valid type. Calling it a "constructor" is a fine loose description; the
   toggle framing is the one that explains the cap.

A field accepts **exactly one type**, plus a boolean "is-tree" toggle. So a
field's type is `T` or `Tree<T>` — nothing else.

### 3.1 Why fields are monomorphic

Earlier drafts gave each slot the combined power of an enum *and* a tree —
multiple accepted types per slot. That was dropped. Because a Struct is itself
an enum, polymorphism belongs *in the Struct*, not in the field: a field that
should accept "A or B" simply takes a Struct whose variants are A and B. With
that, multi-type fields are redundant. The result is a minimal algebra:
one primitive, one compound builder, one (depth-1) constructor.

### 3.2 Why no nested Tree type

`Tree<Tree<T>>` is not so much *forbidden* as **unexpressible**: a field
carries one type-reference and one is-tree bool, so there is simply nowhere to
write a second `Tree`. The depth-1 cap falls directly out of the toggle being a
bool rather than a stackable wrapper.

This does **not** forbid nested *layout* — see §4.2. The distinction (nesting
as type vs. nesting as layout) is the single most important idea in the cell
model.

Note: you can simulate the semantics of `Tree<Tree<T>>` by using `Tree<E>`
where `E` is an enum resolving to `T` or `Tree<E>`.

---

## 4. Schema vs. instance, and the cell tree

### 4.1 Schema is runtime data

Because Structs can be created at runtime, the **default** representation of a
Struct is a runtime record, not a Rust type. The schema is a runtime registry
of records: each Struct has a name and a list of variants; each variant has a
name and a list of fields; each field has a name, a type-reference, and the
is-tree bool.

In this default mode `bevy_reflect` does **not** apply — it reflects Rust
types, and runtime-created types are dynamic data — so the registry and its
structural validation are hand-written.

(A compile-time *variant*, in which pre-known types are real Rust structs, is a
possible alternative route and is the one context where `bevy_reflect` would be
relevant — see §10. The default build is runtime types.)

An **instance** is a value validated against a schema record: pick a variant,
fill its fields. Instances are trees of values.

### 4.2 The cell tree

A `Tree<T>` instance is a **Cell**. Every Cell is itself a Tree — there are no
special "leaf cells" that behave differently. A leaf is just a Cell that
currently holds a value instead of a grid. This uniformity is the point: every
Cell supports the same operations (§7) because every Cell is the same kind of
thing.

A Cell's content is one of exactly two cases:

- **`T`** — a single value of the Cell's designated element type, or
- **Grid** — a 2D grid whose cells are again Cells (each itself a Tree). The
  grid may be empty.

So the model is **`T | Grid`**. Recursion bottoms out at the `T` case.

"Empty" is not a third state. An empty tree is the `Grid` case with no cells; a
grid that loses its last cell is just an empty grid. (An alternative — modeling
content as `Leaf(Option<T>)` — was considered and rejected; `T | Grid` was
chosen as logically equivalent and preferred.)

### 4.3 The grid is 2D, per-Cell, and is layout only

A Cell's grid is a **2D rectangular matrix** of Cells — `m` rows by `n`
columns, with a Cell at every position (that Cell may itself be empty). From
any Cell in the grid, the user can add a column to its left/right or a row
above/below; each such operation inserts or removes a *whole* row or a *whole*
column (§7.2).

Because breadth only ever changes by a whole row or a whole column, the grid is
**always rectangular** — it is never ragged, and it is never assembled one
dimension at a time. A grid is a single 2D thing, not a list of rows that split
independently.

This builds a **2D structure per Cell**. There is no "horizontal tree" vs.
"vertical tree" distinction — 2D-ness is a property of an individual grid, not
of a type. That this 2D shape is layout only — carrying no semantic weight — is
the subject of §4.4.

### 4.4 The flatten invariant — semantics vs. storage

The whole cell tree **flattens, in reading order, to a flat sequence of `T`**.
That flattened sequence is what satisfies the type `Tree<T>`. No matter how
deep or how 2D the tree is, its *type* is the flat `Tree<T>`.

**Semantically, only the flattened sequence matters.** The tree shape carries
no meaning. Rationale, and why this is a deliberate constraint: if the tree
shape *had* to carry meaning, the user would not build this structure at all —
they would just use a `Tree<E>` of an enum `E = T | Tree<E>`. The tree exists
*only* so the canvas can remember 2D layout; it is layout memory, nothing more.
Two cell trees with the same flattening are semantically equal.

Consequently: **the cell tree is the source of truth for storage and identity**
(the editor must remember the exact tree to redraw the canvas), but **the
flattened sequence is the source of truth for semantics**. Many distinct trees
flatten to the same sequence; that is fine and expected.

---

## 5. The three regions: Rom, Ram, Rim

All three regions are **grids of Cells**. Equivalently — since every Cell is a
Tree and a Tree's content can be a Grid — a region can be seen as a single Cell
whose content is a grid of Cells.

### 5.1 Rom — read-only source / palette

Rom is read-only. It contains:

- one instance of **each defined Struct**,
- one instance of **`Symbol`**,
- one **bare Tree** instance.

The bare Tree in Rom is an **empty container box**. It is untyped until it is
placed into a typed field in Ram/Rim, at which point it takes on that field's
element type. (Rom does **not** contain pre-made `Tree<T>` versions of each
Struct — just the one generic empty container.)

Rom cannot be depleted: moving a Cell *out* of Rom behaves as a **copy**. You
**cannot** copy or move Cells *within* Rom.

### 5.2 Ram — untyped scratch storage and the transfer bus

Ram is random-access scratch storage, itself a grid of Cells — Ram is itself a
Tree. Each Ram cell can hold **any value**, including a whole cell tree (moving
a tree into a Ram cell is therefore never a problem).

Ram is special in two ways:

- **Selection:** multiple Ram cells can be selected, in order to duplicate data
  across them. To persistently select a cell, select it, then click it again.
  Ram is also the one region where it is valid for **zero** cells to be
  selected (see §8.2 — zero-selection is used for deletion).
- **No type checking:** Ram performs no type-checking on its contents. This is
  what `Any` refers to. See §6.

Ram is the **only** path between Rom and Rim — Rom and Rim never exchange Cells
directly.

### 5.3 Rim — the canvas

Rim is the canvas and is also a grid of Cells. Rim is where instances are
actually built and laid out; "designated cell" language throughout assumes
Rim's grid.

Rim's root Cell is generally an instance of a Struct, and a Struct's fields are
typed by the schema — that is where Rim's typing comes from. Rim fields are
therefore **typed**.

### 5.4 Always-selected invariant

A Cell is **always** selected in Rom and in Rim — so at least those two
selections exist at all times. Ram is the exception: Ram may have zero selected
cells. (So the minimum is two guaranteed selections — Rom and Rim — plus
0-or-more in Ram.)

This invariant is **provisional** and may be reworked — see §10.

---

## 6. Typing, `Any`, and heterogeneity — the load-bearing section

This section states the rule that keeps the whole design consistent. It was
reached by ruling out several tempting alternatives; the rejected options are
listed because re-introducing any of them rebuilds a contradiction.

### 6.1 The rule: fields are typed, Cells are not

**Typing is a property of fields, not of Cells.** A Cell does not have a type.
A Cell holds a value. A value is **checked against a type only at the moment it
is placed into a typed field.**

- **Rim fields are typed** (by the Struct schema).
- **Rom is the typed source.**
- **Ram fields are not typed** — Ram is scratch.

### 6.2 What `Any` means

`Any` is **not a type**. `Any` is the name for *the absence of checking*.
"A tree in Ram is `Tree<Any>`" means exactly: **Ram does not type-check it.**
It does **not** mean a Cell has acquired a type called `Any`, and it does
**not** mean anything propagates down the tree. There is **no conversion
event** anywhere — nothing is typed in the first place, so nothing converts.

This holds **uniformly at every depth**: a child Cell inside a Ram tree is a
Cell, in Ram, therefore unchecked — exactly like the root. Children are not
"converted to `Tree<Any>`"; they are simply cells in Ram.

### 6.3 Heterogeneous trees are a consequence, not a mechanism

Because Ram does not check, a tree in Ram may be **heterogeneous** — its cells
may hold values of different shapes. This is real and intended, and it is
**Ram-only**: Rim, being typed, cannot hold a heterogeneous tree. But
heterogeneity needs **no new machinery** — it falls directly out of "Ram does
not check." `Any` stays a description of Ram's behavior; it never becomes a
per-Cell type that spreads.

> Rejected alternatives (do not re-introduce — each rebuilds a contradiction):
> - "The Ram cell *becomes* `Tree<T>`" — Cells are not typed; there is nothing
>   to become.
> - "The grid *becomes* `Tree<Any>` and is therefore unmovable" — there is no
>   type `Tree<Any>`; the tree is movable to any Rim field whose declared type
>   its actual contents satisfy.
> - "Allow `Tree<Any>` in Rim" — no; Rim fields stay strictly schema-typed.
> - "Make all Cells `Any`" — this directly violates the design's purpose.

### 6.4 Validation happens on the way into Rim

Type enforcement happens **only** on entry into a typed (Rim) field. The check
is **structural**: walk the entire candidate tree — root and every child Cell —
and confirm every leaf value is a valid `T` for the destination field's element
type. A homogeneous tree matching the target passes; a heterogeneous tree (or
one of the wrong `T`) fails. The check inspects *values*, not Cell-types —
Cells never needed types for this to work.

This is also exactly what "a `Tree<Any>` converts back to `Tree<T>` on the way
to Rim" means: not a mutation of the data, but a *check* of it against the
destination field's `Tree<T>`.

---

## 7. Operations on Cells / trees

### 7.1 The "move" operation

Every Cell has a **move** button. (Whether to also offer a **copy** button, or
to settle on one of the two as a general matter, is an open question — §10.)

### 7.2 Tree (grid) editing — rows and columns

From a Cell selection inside a grid, the user can create new rows and columns:

- **row above**, **row below**
- **column left**, **column right**

And delete neighboring rows/columns:

- **delete + (above / below / left / right)** — deletes a neighboring row or
  column.

A Cell currently **cannot delete its own row or column** (it cannot delete
itself). Whether to lift this restriction is an open question (§10): empty
trees are valid (§4.2), so allowing it would not be incoherent — it is purely
an aesthetic/UX call.

These row/column verbs change the **breadth** of one grid level. They do
**not** change nesting **depth**.

### 7.3 Depth changes — "self" vs "contents", no within/without verb

Changing nesting depth is necessary, but there is **no dedicated
within/without verb**. Depth change is handled by two pairs of operations plus
the Ram round-trip:

- **move self / copy self** — operate on *the Cell itself* (the whole subtree
  rooted at it).
- **move contents / copy contents** — operate on *what the Cell contains*.

Rationale for not having a within/without verb (this is an explicit, accepted
**aesthetic decision** by the project architect, and it is logically sound):

- **Depth decrease** (e.g. delete a parent but keep this Cell; or flatten):
  achieved with the Ram round-trip. Delete-parent-keep-this = move self → Ram,
  delete parent, move self back. Flatten = move contents.
- **Depth increase / wrapping** (e.g. "create a new parent around the root"):
  "move self" on a **root** Cell is unambiguous — at the root there is no
  parent to move *within*, so the only coherent reading is that the Cell leaves
  and **a fresh root Cell is created to fill the hole**. Depth increases as a
  *consequence* of that single operation, not as a hidden second meaning. One
  operation; its effect at the root happens to include a new root.

That an explicit within/without verb is thereby unnecessary is considered
something a competent user will infer; the provided verbs are to be used as the
architect intends. The behavior is "fun," is intended, and is
**manual-worthy** — it should be documented in the user manual rather than
treated as a quirk.

### 7.4 "move contents" of a grid is well-defined

The "contents" of a *grid* is multiple cells. This raises no special case: a
Ram cell holds a whole **tree**, not a single value, and multiple cells already
*form* a grid, which is itself a tree. So "move contents" of a grid moves that
grid as one tree into one Ram cell — no carrier-wrapping rule is needed,
because every Ram cell is already a tree. (An earlier draft treated this as an
open caveat; it is resolved.)

---

## 8. Movement rules between regions

Cells flow **Rom → Ram → Rim**. Rom and Rim never exchange directly.

| From → To      | Behavior                                                                 |
|----------------|--------------------------------------------------------------------------|
| Rom → out      | Always a **copy** (Rom cannot be depleted).                              |
| Rom → Ram      | Copy. Always succeeds.                                                   |
| Rom → Rim      | **Forbidden.** Must go via Ram.                                          |
| within Rom     | **Forbidden** — no copy and no move within Rom.                          |
| within Ram     | Allowed (move and copy). Always succeeds. Multi-select duplicates data.  |
| Rim → Ram      | Move. Always succeeds.                                                   |
| Ram → Rim      | Move. **Fallible** — see §8.3.                                           |
| within Rim     | No direct copy and no direct move — see §8.1.                            |

### 8.1 Rim has no direct internal copy or move

- **Copy within Rim** = duplicate via Ram: Rim → Ram (use Ram's multi-select to
  duplicate) → Rim.
- **Move within Rim** = round-trip via Ram: Rim → Ram → Rim, back to the
  designated cell.

(An earlier draft mistakenly said "via Rom"; the correct region is **Ram** in
both cases. Rom is read-only and Rom→Rim is forbidden, so Rom could not serve
this role.)

### 8.2 Deletion

Deletion = moving a Cell **from Rim to Ram while 0 cells are selected in Ram**.
(Zero-selection is valid only in Ram — §5.2, §5.4.)

### 8.3 The fallible-move rule

**Only Ram → Rim can fail.** Rom→Ram, within-Ram, and Rim→Ram never fail.
Rom→Rim is forbidden outright (so not "fallible," just disallowed). A Ram→Rim
move is validated by the structural tree-walk of §6.4 against the destination
field's type; a heterogeneous or wrongly-typed tree is rejected **at the moment
of the move**, against a specific destination, with a clear cause. The UI must
express this failure — e.g. reject the drop and/or highlight the offending
leaf cells.

---

## 9. Read-only meaning of Rom

"Rom is read-only" means: other regions cannot **deplete** it — moving a Cell
out of Rom copies instead. It also means no copy/move *within* Rom (§5.1, §8).
Rom's contents are the fixed palette: one instance per Struct, one `Symbol`,
one bare empty Tree.

---

## 10. Open questions

The type system, the cell model, and the movement rules are **closed**. The
regions are closed in structure, with one provisional point (item 5). What
remains open:

1. **Verb naming.** The *set* of operations is settled (move; row
   above/below, column left/right; delete + direction; move/copy self;
   move/copy contents — and explicitly **no** within/without verb). Open: exact
   button labels / UI wording; whether to offer **move**, **copy**, or both as
   a general matter (§7.1); and the possibility of further verbs being added
   later.

2. **Deleting your own row/column.** A Cell currently cannot delete the row or
   column it occupies (§7.2). Since empty trees are valid (§4.2), lifting this
   would not be incoherent — it is purely an aesthetic/UX call, left open.

3. **In-app type editor.** Types are runtime data and can be created from
   program code; whether the editor *also* exposes a UI for defining/editing
   Structs at runtime is undecided.

4. **Type-representation strategy.** The default build represents Structs as
   runtime records (§4.1). An optional compile-time variant — pre-known types
   as real Rust structs with `bevy_reflect`, selected via `#[cfg]`, with
   identical frontend behavior — could let users define types in Rust and
   compile them in, turning the project into a library. Decision: proceed with
   runtime types first; keep this route open.

5. **The always-selected invariant.** §5.4 (a Cell is always selected in Rom
   and Rim; Ram may have zero) is provisional and may be reworked.

---

## 11. Implementation note

The project is to be built in **Bevy 0.18** (current latest). Bevy provides the
ECS and the UI for the *editor*; the *document* (schema registry + cell trees)
is its own data structure, validated by hand-written structural checks. In the
default runtime-types build, `bevy_reflect` is not used for the schema; the
optional compile-time variant (§10) is the only context in which it would be.
Implementation detail belongs in a separate document; this spec stays
implementation-agnostic.
