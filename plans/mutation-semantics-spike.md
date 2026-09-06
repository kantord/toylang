# Provably-one-reference analysis for mutation-as-optimization: the spike

Spike for the "Mutation as an optimization" TODO in draft.md (the
prelude-partials-and-mutation ruling): can a backend prove that a Vec/Str-typed
name has exactly one reference, so that a consuming use may mutate it in place
instead of copying? Three questions, answered from the current TIR (`src/tir.rs`)
against the two backends named in the commission, `src/emit_lua.rs` and
`src/emit_rs.rs`.

**Status: spike only, nothing implemented.** The claim is that the conservative
rule below is decidable from the current TIR, and that both backends already
carry the copy that rule would remove. The gap is the consuming/observing
classification, which the TIR does not carry.

## (1) Where the copy happens today

The two backends copy at different places, because they make opposite ownership
assumptions, and the difference is the whole argument for the analysis.

**emit_rs.rs copies on every read.** Values are owned; every expression produces
an owned value, so any re-read of a name must copy it. That is the dominant
pattern in `expr`: `Kind::Var` emits `{name}.clone()` (emit_rs.rs:1177),
`Kind::Local` emits `{local}.clone()` (:1178), and `Kind::Input`/`Kind::Inputs`
emit `.clone()` (:1179-1180). The mutating builtins then take `&[T]` and copy
again before they are allowed to touch anything:

- `tl_sort` (:127) is `v.to_vec()` followed by an in-place `sort()` on the copy.
- `tl_reverse` (:134) is the same shape.
- `tl_flatten` (:116) builds a fresh output by `.cloned()`.
- `tl_slice` (:87), `tl_tail` (:107), and `tl_at` (:73) copy the slice or the
  extracted element.
- `concat` (:1536) on a `Vec` emits `[{l}, {r}].concat()`, copying both operands.

The fused loop adds two more: `let t_line: String = line.clone()` (:1094) to get
the element out of the reused read buffer, and a `{local}.clone()` after a
`Select` stage (:1128) to hand the surviving element onward as owned. `distribute`
(:1163) and `show` (:1229, :1440) clone at each iteration and print layer.

So in Rust the copy cost is spread over the whole tree, twice over at a mutating
site: once for the read, once inside the helper.

**emit_lua.rs copies only at the mutating builtins.** Lua tables and strings are
reference types, so reads, indexing, and argument passing are free. The emitted
Lua contains no value clones at all -- the only `.clone()` hits in the file are
Rust-side `Type` clones inside the emitter (:403, :423), not emitted code. The
one place a copy is forced is where the runtime must *not* clobber its input:
`tl_sort` (:152) and `tl_reverse` (:161) build an `out` table element by element
and only then sort/reverse it (because `table.sort` mutates in place), and
`tl_flatten` (:140) builds fresh. String concat makes a new string natively.

The two copy sites coincide exactly with the consuming builtins. That is the
finding for question 1: the sites a "provably one reference" rule would let both
backends drop are the same set -- Rust's `tl_sort`/`tl_reverse`/`tl_flatten` and
Vec `+` (which would become a push instead of a read-plus-concat), and Lua's
`tl_sort`/`tl_reverse` (which would mutate the input table directly instead of
building `out`). Lua never pays the per-read tax because it has no ownership to
enforce, so its only copies are the ones the analysis is aimed at.

## (2) Is the rule decidable from the current TIR?

The narrow rule -- "this name has exactly one use, that use is consuming, and no
use follows it" -- is decidable from the current TIR without a borrow checker.
This is structural, not a lucky accident:

- **The TIR is a pure tree.** There are no first-class references, no closures,
  and no higher-order functions: functions are unary and called by name, and the
  emitters inline a callee's body rather than constructing a closure value. A
  name exists only as a binding node plus the `Kind::Local(id)` / `Kind::Var(name)`
  reads inside its scope. There is no construct that shares one value between two
  scopes; two `Bind`s of the same value expression re-evaluate it, producing two
  materializations, not two references to one. The stdin sources (`Inputs`,
  `Lines`, `Dsv`) never alias either, since each tree node is its own read.
- **Occurrence counting is a scope walk.** For a `Bind { local, value, body }`,
  count `Kind::Local(local)` in `body`; for `Map`/`Select`/`OptMap`, count the
  param in `body`/`pred`; for `Match`, count the payload local in the arm `body`;
  for a function, count `Var(param)` in its body. "Exactly one use" is a walk,
  not an analysis.
- **Evaluation order is tree order.** The mutating builtin's `arg` is a subtree;
  the reads that precede it in the emitted program are exactly the subtrees the
  emitters walk first. So "the consuming use is the last use" is orderable from
  the tree with no liveness lattice.

What the TIR does **not** carry, and what the pass must supply:

1. **A consuming/observing classification.** `Kind::Local(id)` is the same node
   whether it is `sort(x)`'s argument (consuming: today copies-then-mutates) or
   `.x[0]`'s base (observing: reads without replacing). The TIR does not tag this;
   the pass infers it from the parent. Consuming sites today are the mutating
   builtin args (`Sort`, `Reverse`, `Flatten`) and the `Concat` operands on a
   `Vec`; everything else -- `Index` base, `Field` base, `Compare` operands,
   `Builtin::Length`, printer input -- is observing. This is the one judgment
   call, and the natural place to be conservative.
2. **Function boundaries.** A `Call` is inlined at emit time, so the pass must
   descend into the callee body to learn whether it observes its parameter after
   the mutation -- the "composite function call" provenance the draft flags
   (draft.md:2108). Decidable, because `Program` carries every `Func`, but it is
   the only genuinely involved part, and it is where a first implementation
   should refuse rather than guess.
3. **Nothing runtime.** The draft rules out refcounting (draft.md:2123) and this
   needs none: no liveness lattice, no region inference, no lifetimes. The value
   graph is tree-shaped with lexical scopes, which is why a full borrow checker
   is overkill.

One honest non-goal: the fused loop's `line.clone()` (emit_rs.rs:1094) is not a
removable copy. The buffer is reused across iterations by design, so the element
must leave it; that is a property of the buffering scheme, not of reference
counts, and the analysis should not claim it.

## (3) Sketch of the check, with example programs

**Rule v1 (what the spike would ship):** a Vec/Str-typed name may be mutated in
place at a consuming use iff it has exactly one use in its scope and that use is
the consuming one. Everything else keeps today's semantics (Rust: copy on every
read, copy again in the helper; Lua: copy at the mutating builtin).

**Rule v2 (the draft's lazy-copy promise, sketch only):** allow an in-place
consuming use even when earlier observing uses exist, provided no observing use
follows it; when a later observing use does exist, take the copy at the mutation
point (draft.md:2102-2104). Because order is tree order, "later" is decidable.
v2 is strictly more permissive than v1 and is the version that pays off the
draft's shadowing idiom.

The accept/reject line, in toylang syntax:

```
# ACCEPT (v1 and v2). v has exactly one use, the concat.
fn append_one(v: Vec<Int>) -> Vec<Int> = v + [1]
append_one([4, 5])
```
Rust lowers to a `push` on the taken-owned `v` instead of `[v.clone(), [1]].concat()`;
Lua may `table.insert` on the argument directly. The literal `[4, 5]` at the call
site is born fresh, so its reference is unique by construction.

```
# ACCEPT (v1 and v2). v has exactly one use, the consuming sort.
fn sorted(v: Vec<Int>) -> Vec<Int> = sort(v)
sorted([3, 1, 2])
```
Rust takes ownership and calls `v.sort()` instead of `tl_sort`'s `to_vec` copy;
Lua sorts the argument table in place instead of building `out`.

```
# ACCEPT under v2 only (two uses, first observing, the consuming one last).
fn norm(v: Vec<Int>) -> Vec<Int> = [length(v)] + sort(v)
```
`v` is read by `length` (observing) and then by `sort` (consuming, and the last
use). v1 refuses because there are two uses; v2 accepts because the observing
read produced an `Int` before the sort ran, so nothing observes `v` afterward.
This is the lazy-copy promise: the sort may mutate in place, and the `length` is
already done.

```
# REJECT (both rules). The observing read comes after the consuming sort.
fn stats(v: Vec<Int>) -> Vec<Int> = sort(v) + [v[0]!]
```
`sort(v)` consumes `v`, then `v[0]!` observes it afterward. v1 refuses on the
two uses; v2 refuses because a later observing use exists, so the sort must keep
its copy. This is the exact "no re-read after a mutating op" case the commission
names, and it is the program that keeps the rule honest.

```
# REJECT (both rules). Two uses, neither one unique.
fn double(v: Vec<Int>) -> Vec<Int> = v + v
```
`v` appears twice, both in consuming positions of a concat. Not unique; both
sides must keep copies.

## What landing it would look like (not done here)

A `consuming(t: &Tir) -> bool` over the node kinds, a scope walk counting
`Kind::Local`/`Kind::Var` occurrences per binding, and a per-backend switch: at
an accepted site, emit `v.sort()`/`v.push(...)` (Rust) or mutate the table
(Lua) instead of the current copy-then-mutate. The function-boundary descent is
the part to leave refusing on a first pass. Corpus coverage would be exactly the
programs above, verified by their `snapshot` pinning on `rs` and `lua`.
