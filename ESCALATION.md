# ESCALATION: building gh:136 before the one-stdin-source redesign

The issue itself flags the gating: a parameterized `dsv(delim)` source breaks the
sources-are-nullary rule, so it "lands alongside or after the one-stdin-source redesign"
(board rows `stdin-syntax-design`, a decide, and `stdin-redesign-build`). Both are still
`todo` on the live board, and the redesign's syntax is explicitly unsettled ("gets its own
session once type-flow lands", draft.md). This lane was dispatched as a build anyway. The
questions below are what the issue and gh:88's ruling do not settle; each was answered with
the most conservative reading that still delivers the ratified surface (`dsv(delim)` plus
`csv`/`tsv` partials), so nothing here waits on a human.

## Q1: does this lane also do the redesign?

The one-stdin-source redesign retires `input`/`inputs`/`lines` for one parameterized source
with parsing as ordinary steps. Its syntax is deliberately unsettled and it is a decide
row. Doing it here would mean inventing that syntax, which is the session's job. Not done.

Alternative considered: implement the redesign as part of this. Cost: invents an unsettled
syntax, rewrites every corpus program and every source reference page, and destabilizes the
stream/linearity core -- the exact thing the decide exists to avoid. Rejected.

## Q2: cardinality of `dsv(delim)`

`dsv(delim)` could be a `Stream<Vec<Str>>` source (sibling of `lines`/`inputs`) or an eager
`Vec<Vec<Str>>` value (the shape `inputs` shipped as before it became a stream, draft.md's
"inputs, eager" decision).

Chosen: eager `Vec<Vec<Str>>`. One source of ambiguity in `tir::Source`/fusion is avoided
across all seven backends, and the redesign session is the right place to decide whether a
delimited source streams. Cost if wrong: `collect(csv)` refuses (it is not a stream), and a
follow-up would add fusion support. `input` is already an eager value source, so this is a
familiar shape.

## Q3: how do `csv`/`tsv` become "prelude partials"?

The prelude is toylang source (`prelude.toy`), and a `pub fn csv() = dsv(",")` body is
refused today: `source_in_fn` rejects stream sources in function bodies, and allowing a
source as a nullary function's body with correct single-use/exclusivity semantics is the
redesign's hard part (single-use becomes reachability-based, not AST-based).

Chosen: `csv`/`tsv` are compiler-predefined names that lower to `dsv(",")`/`dsv("\t")` in
the parser, available to every program like the builtins are. This honors the intent (fix
the delimiter once, as a partial application of the parameterized source) without the
source-in-fn-body extension. Cost if wrong: they are not literally prelude.toy definitions;
a program could not shadow them, and a redefinition is a parse conflict. Making them real
prelude functions is listed as a candidate for the redesign build.

## Q4: what does the split do?

Raw line-by-line split, the sense "DSV" carries distinct from CSV's quoting rules: no
quoting, no escaping, no header handling. A line split on a delimiter that is absent yields
one field (the whole line). Blank lines are kept, like `lines` keeps them, so a blank line
is one empty field; a `\r` before the `\n` is preserved, again matching `lines`, so CRLF
input leaves a `\r` on the final field. Each of these follows from matching the closest
existing source (`lines`) rather than inventing a new convention.

## What would settle each

- Q1/Q2: `stdin-syntax-design` decide and `stdin-redesign-build` -- whether dsv becomes the
  redesign's first parameterized stream source.
- Q3: a maintainer call on whether "prelude partial" must mean `prelude.toy`, or whether
  compiler-predefined is acceptable.
- Q4: any CSV-vs-DSV calling convention the maintainer prefers over raw split.
