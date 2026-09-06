# Dense tensor design: research before Q17-Q19 are re-asked

Groundwork for gh:175. The round (`docs/.grill/dense-tensor-type.round.yaml`) is retired and
gone from disk and history, and the issue body names but does not quote its three questions,
so this works from the two surviving sources: [the dense tensor sketch](../draft.md#a-dense-tensor-type)
(draft.md:1072-1093) and Q17-Q19 in [questions.md](questions.md#q17-is-there-a-dense-tensor-type-constructed-explicitly).
The three things the issue asks for, in order: whether a dense tensor value kind is justified
at all; each of Q17-Q19 worked through realistic multi-step pipelines rather than the round's
one-liners; and on nulls, the "trivial-conversion" idea weighed against the bitmask,
sentinel, and hard-fail tradeoff. Two language facts bear on everything below: `Float` does
not exist yet (`3.14` is still notation, draft.md:2266), and the only numbers implemented are
`Int` and `Int64`. The pipeline syntax here is illustrative, as the draft's is (draft.md:4).

## Whether a dense tensor value kind is justified at all

Two different decisions are tangled together in "is there a dense tensor type", and they have
separate customers.

The first is **shape safety**: rectangular data, known extent per dimension, and a check that
you actually have it. This needs no new value kind. The design already names it twice,
`as_tensor : Vec<Vec<Num>> -> Result<Tensor<Num, [n, m]>, ShapeError>` (draft.md:240 and
draft.md:468), and [every dimension gets a spec](../draft.md#proposal-every-dimension-gets-a-spec)
already handles rectangular data without a second scheme: "a tensor is not a second scheme
with its own syntax; it is the same scheme over a type whose extents happen to be uniform",
and "rectangularity becomes a refinement rather than a gate". On the access side the tensor is
a done deal, and it is free.

The second is **a layout change**: a flat, row-major, unit-stride buffer instead of
`Vec<Vec<Num>>`'s array of pointers to separate allocations. This is the part that is a new
value kind, and it is only worth a seventh kind if a pipeline needs one of the three things
only a flat buffer delivers:

- **Vectorization across the whole region.** `Vec<Vec<Num>>` has no single stride: element
  `(i, j)` costs a load of the inner pointer and then the element, inner rows are not
  contiguous, and only individual rows can vectorize as a region. The legality table at
  draft.md:1015 lists "unknown or non-unit stride" as a blocker, and the row that removes it is
  "a dense buffer has unit stride known at compile time". A 2D region vectorizes only if the
  language can hand the vectorizer a flat buffer and a shape.
- **Broadcasting arithmetic.** draft.md:1095 confines elementwise arithmetic on tensors to the
  new kind precisely because plain JSON arrays error on arithmetic (elementwise ops over two
  `Vec`s are still an open question in the arithmetic reference). `$m * 2` as a matrix scalar
  has no spelling over `Vec<Vec<Num>>`.
- **Zero-copy interop.** A flat buffer plus shape is the Arrow layout; nested `Vec`s are not
  (draft.md:1090-1093).

So the question the round should re-ask is not "does the language need rectangularity" (it
already has it) but "does a realistic pipeline need the flat buffer". The test is a matrix
consumed by several elementwise stages and reduced, with interop on the boundary:

```
.readings            # Vec<Vec<Num>>: 3 days x 1024 samples
  | @f32             # narrow: hard-fails on hetero/null/nested
  | reshape(3; 1024)
  | . * 2            # broadcast, elementwise
  | map(fold(add; 0))   # per-day totals
```

The middle two stages are the tell. `reshape` and `map(fold(...))` work fine over
`Vec<Vec<Num>>` plus the existing `flatten` and `sum`; the `. * 2` does not, and neither does
reliable 2D vectorization or an Arrow hand-off. If a dogfood pipeline stops at select, map,
and filter over rows, `Vec<Vec<Num>>` plus the stdlib covers it and a seventh kind is dead
weight. The moment the pipeline touches an elementwise whole-region op, a flat buffer, or a
columnar boundary, the value kind earns its construction sites.

One option the round did not list: a **tensor as a flat `Vec<Num>` plus a shape**, the same
"one access model" as the spec section but with a distinct rectangular type and a unit-stride
buffer, broadcasting as an operator extension over that buffer, shape promised by the type
rather than by `@f32`'s width. This honors the spec section's "one access model, not two"
while still delivering the two things that need a flat buffer. It separates the shape promise
(a type concern) from the width (`f32`, a number concern), which `@f32` conflates.

Leaning: the tensor **type** is justified (the `as_tensor` signature already commits to it);
the tensor **value kind** is justified only by the flat-buffer customer, and since `Float`
does not exist yet and `@f32` presupposes it, the width should not be bundled into the
construction decision. Flagging that last point as inference, not a settled design fact.

## Q17. Construction syntax

The options:

- **A. `@f32` then `reshape`.** `.readings | @f32 | reshape(1024; 3)` (draft.md:1078). The
  constructor narrows (hard-fails on heterogeneous input, nulls, or nested strings at the
  constructor rather than three stages later) and `reshape` attaches shape as a second stage.
- **B. Named `as_tensor`.** `as_tensor(v) -> Result<Tensor<Num, [n,m]>, ShapeError>`
  (draft.md:240, 468). Shape and homogeneity in one call; failure is a `Result` value the
  program must handle. This is parse-don't-validate applied to shape.
- **C. Single constructor that takes the shape.** `tensor(3; 1024)` doing narrow plus reshape
  in one step, failing loud on any mismatch.
- **D. Shape from the type.** A declared type supplies the shape; the constructor is just
  `tensor(...)` or `@f32`, with no `reshape`, because the signature says what the shape is.

Working the three-day sensor pipeline through each.

**A.**

```
.readings | @f32 | reshape(3; 1024) | . * 2 | map(fold(add; 0))
```

Two stages to read. The failure surfaces at `@f32` if any reading is null, a string, or
ragged, which is exactly the narrowing the sketch promises. The cost: `@f32` commits the
buffer to `f32` before `Float` exists, and "second number type, deliberate lossy exit from
the f64 commitment" (draft.md:1085) is a claim about the eventual float, not about shape.
`reshape` as a separate stage finds a mismatched length one operator later than the
constructor, which is the "three stages later" failure the design wanted to avoid, just moved
by one.

**B.**

```
fn as_tensor(...) -> Result<Tensor<Num, [3, 1024]>, ShapeError>
...
as_tensor(.readings) | map(. * 2) | map(sum(.))
```

Honest about failure: the program names the shape in the signature and the `Result` forces it
to decide what a shape error means. The cost: `Result` handling in the middle of a pipeline is
noise, and the shape lives in the type while the construction still has to get the buffer
there. This is the most verbose of the four.

**C.**

```
.readings | tensor(3; 1024) | . * 2 | map(fold(add; 0))
```

One operator, one failure point, no width bundled in; `tensor` can carry whatever the eventual
number type is. This is A with the `@f32` width commitment dropped and the shape moved into
the constructor. It is the shortest spelling that still narrows and shapes in one place.

**D.**

```
fn norm(m: Tensor<Num, [3, 1024]>) -> Tensor<Num, [3, 1024]> = m * 2
norm(tensor(.readings))
```

Shape is written once, in the type; the constructor just has to produce a rectangular buffer.
Failure is a type error at `tensor(.readings)` against `Tensor<Num, [3, 1024]>`, the loudest
and earliest possible. The cost: the shape must be expressible at the construction site, and a
constructor that infers shape from the target type is type-directed inference of exactly the
kind the draft's "constructed explicitly, never inferred" line (draft.md:1074) warns against.

All four agree on the important part: shape is earned by a check, not assumed. They differ on
whether width travels with construction (A does, C and D do not) and whether failure is a
`Result` (B) or a loud constructor. The round's real decision is A versus C, whether `@f32`'s
width promise is wanted, and that hangs on whether the dense kind is the `f32` story or a
type-carrying one.

## Q18. What `.[]` does on a rank-2 tensor

The round's phrasing is jq's, but the language dissolved `.[]` into index specs before Q18 was
asked, so the question has to be restated: there is no `.[]` stream, only `[]` meaning "keep a
dimension at full extent" ([index specs](../docs/reference/operators/specs.md)). Under
[every dimension gets a spec](../draft.md#proposal-every-dimension-gets-a-spec), a type has an
ordered list of dimensions and an access gives each one a spec, left to right. So on a rank-2
tensor `v[]` keeps dimension 0 and yields rank-1 rows, and `v[][0]` keeps dimension 0 and
collapses dimension 1 to scalars. "Rows" is not a choice the spec model leaves open; it is the
forced consequence of one spec per dimension operating on the leftmost dimension. The "scalars"
answer would require one `[]` to collapse two dimensions at once, which the model forbids.

Rank-polymorphism falls out without new machinery: `map` already applies its body to each entry
of a `Vec`, so on a rank-2 tensor `map(f)` applies `f` to each row. Per-row sums are
`map(sum(.))`, and full linearization is `flatten`, which already exists as `Vec<Vec<T>> -> Vec<T>`
(the flatten builtin), the "separate flattening view" the round anticipated.

A confusion matrix worked through:

```
.counts                        # Tensor<Num, [4, 4]>  (predicted x actual)
  | map(sum(.))                # per-row (per-class) totals: Vec<Num>, length 4
  | sum(.)                     # grand total
```
```
.counts
  | flatten                    # one level: 16 scalars, row-major
  | sum(.)                     # same grand total, without the intermediate Vec
```

Both totals come out the same; the difference is whether the program wants the per-class
vector on the way. Under "scalars" semantics the per-row step would need a reshape back to
rank-2 first, an extra stage that buys nothing. The case rows-iteration does not give directly
is a per-column reduction (along dimension 0): rows-iteration reduces per row, and a column
total needs a transpose or an explicit gather. A transpose view is the real gap, and it is a
separate question from rows-versus-scalars.

So Q18 as posed is already answered by the access model (rows), and the sharper re-ask is
whether full linearization is `flatten` (it exists) and whether a transpose/column-access view
is wanted at all. The round's one-line `map(fold(add; 0))` also assumes a `fold` that is not a
builtin today; `map(sum(.))` is the spelling that works now.

## Q19. How nulls are carried

The round's three options, with the draft's position: **NaN sentinel** is already rejected
(draft.md:1089, it collides with genuine NaN); **Arrow validity bitmask** solves the collision
and buys zero-copy interop; **hard-fail** is what `@f32` does at construction (draft.md:1081,
fails on nulls). The issue adds a fourth: a tensor-specific type with a near-trivial (single
call) conversion to and from a plain tensor, floated by the maintainer as possibly sidestepping
the whole tradeoff.

The trivial-conversion idea does not sidestep the representation tradeoff; it relocates the
default. A dense tensor is a no-null buffer, and its constructor hard-fails on nulls, which is
the hard-fail option unchanged. The moment nulls must be representable, some layout has to hold
them, and of the three the sentinel is already dead, leaving the bitmask. So the conversion is
not a fourth representation; it is a wrapper that makes hard-fail the default and the bitmask
opt-in. That is an ergonomics decision (which is the common case) rather than a representation
decision, and it is a genuinely useful one: most tensor pipelines are dense, and a
`masked(...)` / `dense(...)` pair lets nulls appear only where the data forces them.

It also mirrors what the language already did for `Opt`. Absence is tagged, and on the jq
backend no in-memory value is ever JSON null ([borrowing the host's null](../research-log/borrowing-the-hosts-null-borrowed-its-conflations.md)).
A nullable tensor is a rectangular `Vec<Opt<Num>>`; a dense tensor is the all-present case. The
"conversion" is really "resolve the nulls", fill or drop, a decision the program should make
explicitly, the same way "reification is where allocation becomes visible" (draft.md:874) makes
materialization explicit.

A sensor pipeline with gaps:

```
.samples                       # Vec<Opt<Num>>, gaps where a sensor dropped out
```

**Hard-fail only:**

```
.samples | @f32                # refuses: there is a null
```

The program must first resolve: drop with a filter, or fill each gap with a default via a
`some`/`none` match, e.g. `map(. | some(x) -> x or none -> 0.0)`. The tensor that results is
clean and every downstream op is total. This is the draft's leaning and the cheapest.

**Bitmask (nulls preserved):**

```
.samples | masked(3; 1024)     # nulls in, validity bitmask beside the buffer
  | mean_per_sample            # must define what a masked entry contributes
```

Nulls stay first-class and Arrow interop is real, but every consuming op now has a partial
answer to give. This is the layout you pay for only when the raw gaps must survive to the
boundary.

**Trivial conversion:**

```
.samples | dense(3; 1024)      # hard-fail; a "masked"/"dense" pair
.samples | masked(3; 1024) | dense(3; 1024)   # back, after resolving the gaps
```

One call each way. It makes hard-fail the default (matches draft.md:1081 and the
reification-visible principle), reserves the bitmask for the nullable form where interop
matters, and never touches the sentinel. Its honest cost: it is not a shortcut past the
tradeoff, the nullable form still needs the bitmask; it is a decision about which is the
default, and "single call" is an ergonomics claim to test, not a settled fact.

Leaning: hard-fail as the default for the dense kind, a `masked`/`dense` conversion pair for
the rare nullable case, the Arrow bitmask behind `masked` where interop wants it, and the NaN
sentinel left rejected. The single-call convenience should be measured against the pipeline
above, where the interesting work (resolving the gaps) happens before the conversion either
way.

## What would settle each

Whether the value kind is justified: a single realistic pipeline that must do an elementwise
whole-region op or an Arrow hand-off, run under `Vec<Vec<Num>>` plus the stdlib first. If the
stdlib version is acceptable, the seventh kind is not.

Construction syntax (A vs C): whether the dense kind is committed to `f32` or is type-carrying,
which is downstream of whether `Float` and the f32 exit are decisions the tensor should be
tied to.

Iteration: whether a transpose/column-access view is wanted at all; rows-versus-scalars is
already settled by the access model.

Nulls: whether a nullable tensor has a customer once gaps can be resolved before the dense
constructor. If not, hard-fail alone is enough and the bitmask is YAGNI.
