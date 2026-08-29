---
type: Lesson
calendar:
  - 2026-08-29
title: Borrowing the host's null borrowed its conflations
description: Making absence tagged (#62) required changing only the four backends that had mapped Opt onto a host null-ish value; the three that had built their own representation were already tagged and needed nothing.
tags:
  - backends
  - representation
timestamp: 2026-08-29T00:00:00Z
---

# Borrowing the host's null borrowed its conflations

When #62 moved `Opt` from untagged value-or-null to a tagged prelude enum, the expected cost
was seven backend encodings. The actual cost was four. The split was exact: every backend
that had *borrowed* a host value for absence had to move, and every backend that had *built*
a representation was already correct.

Borrowed, and conflating levels (Symbol/None/sentinel/null at two depths is one value):
JavaScript's `Symbol("none")`, Python's `None`, Lua's `tl_none` table, jq's actual `null`.
Built, and already tagged without anyone intending it: Rust's `Option<T>` (`Some(None)` is
not `None`), Go's `tlOpt[T]{ok, v}` (`{false}` is not `{true, {false}}`), and the native
runtime's boxed slot (a box holding `NULL` is not `NULL`). `runtime/toylang.c` was untouched
by the migration, which nobody would have predicted from the issue's phrasing.

The lesson is about where invariants live. A borrowed representation imports the host's
equivalences along with its convenience: the host's null has no levels, so neither does
yours. A constructed representation only ever has the structure you gave it, and structure
composes -- the Go struct nests because structs nest. This is the same shape as
[the backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md),
from the other direction: there the host imposed rules the design did not have, here it
erased distinctions the design turned out to need.

The four that moved now carry the enum's own runtime shape (`{"some": v}` / `"none"`), which
buys a second invariant for free in jq: no in-memory value is ever JSON null, so jq's
out-of-range `.[i]` yielding null is unambiguously "was not there".

Open: native's Opt layout (NULL-or-one-slot-box) is now a different shape from the general
enum's two-slot tag box -- legal while the checker refuses matching an Opt by variant, and
another entry in the pile
[vec-of-enum fell into](vec-of-enum-falls-into-the-boxed-default-nobody-chose.md): a layout
nobody chose, held in place by a refusal. If the matcher-totality round gives Opt ordinary
arms, either the match lowering special-cases Opt on native or the layouts reconcile.

Still open after #66:
[reorder found a route around the layout it did not reconcile](reorder-found-a-route-around-the-layout-it-did-not-reconcile.md)
needed to reach inside an Opt payload before that round runs, and turned out not to need this
resolved -- rebuilding a present value is a smaller question than matching on the variant.
