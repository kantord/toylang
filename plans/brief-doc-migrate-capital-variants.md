Board row `doc-migrate-capital-variants`: migrate 6 doc pages' variant-declaration examples
from lowercase-leading names to capital-first-type + lowercase-constructor, matching the gh:156
casing rule the checker now enforces, then remove the gate that currently lets `just test` pass
around them.

## The rule, verified today

```toylang
enum Shape { Point, Circle{r: Int} }
circle{r: 1}
```
```
{"Circle":{"r":1}}
```

Declared variant names must start with a capital letter (`Point`, `Circle`); the constructor
used to build/match a value is the same name lowercased (`circle{...}`, or a bare lowercase name
for a unit variant). A declaration that still starts lowercase is refused, verified today:

```toylang
enum Shape { point, circle{r: Int} }
circle{r: 1}
```
```
toylang: a variant name starts with a capital letter; declare `point` of `Shape` as `Point`,
and build its value with the lowercase constructor `point` (at byte 13)
```

## What's gating `just test` green right now

`tests/docs.rs`'s `capital_variant_gate` (around line 193) lists 11 fragments across 6 pages
that still declare lowercase variant names, and pins them to being refused for exactly the
casing reason instead of asserting their (now-stale) `output`/`refuses`/`error` fence:

```rust
fn capital_variant_gate(at: &str) -> bool {
    matches!(
        at,
        "docs/guides/enums.md:13"
            | "docs/guides/matching.md:54"
            | "docs/guides/matching.md:66"
            | "docs/reference/operators/comparison.md:28"
            | "docs/reference/types/enum.md:7"
            | "docs/reference/types/enum.md:34"
            | "docs/reference/types/enum.md:67"
            | "docs/tutorial/04-enums.md:6"
            | "docs/tutorial/04-enums.md:20"
            | "docs/tutorial/04-enums.md:40"
            | "docs/tutorial/06-matching.md:67"
    )
}
```

For each of those 6 pages:

1. Capitalize every variant name in the `enum` declaration (e.g. `enum Msg { ping, quit,
   text{body: Str} }` -> `enum Msg { Ping, Quit, Text{body: Str} }`).
2. Leave constructor/match usage as the lowercased form -- `text({body: "hi"})`,
   `m | ping -> ... or text -> ...` stay exactly as written; only the declaration's names
   change case.
3. Re-run each fragment (`target/release/toylang run`) and fix its `output`/`error` fence to
   match real output -- a payload variant's JSON key is now capitalized (`{"Circle":{"r":1}}`,
   not `{"circle":{"r":1}}`), which changes any fragment whose fence shows that JSON shape
   directly rather than through a function that already unwraps it.
4. Once a page's fragments no longer trip the casing refusal, delete that page's entries from
   `capital_variant_gate`'s list. When the list is empty, delete the gate function and the
   `gated_failures`/`GATED` machinery in `tests/docs.rs` entirely (see the comment at
   `tests/docs.rs:20-24` for what to remove) -- don't leave a dead gate with an empty match arm.

Also check for lowercase-leading variant declarations in `README.md` and any `tests/corpus/*`
fixtures not already covered by the gate list (the board row's broader scope is "corpus,
examples, README"); the gate list above is the confirmed minimum, not necessarily the complete
set.

Done-gate: `just test` passes with the capital-variant gate machinery removed from `tests/docs.rs`
entirely, and every migrated page's fragments assert their real (capitalized) output.
