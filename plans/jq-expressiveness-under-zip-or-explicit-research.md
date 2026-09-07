# jq expressiveness under a zip or explicit-only default: what the cartesian product actually carries

Research spike for [Q2](questions.md#q2-binary-operators-over-two-multi-valued-expressions-cartesian-zip-or-explicit)
in [binary-op-multiplicity-design](board.yaml), requested by the round (multiplicity-and-offload-round2, question
binary-op-default-multiplicity): the maintainer leans B (zip/broadcast) or C (no default, explicit
`zip`/`cross`) for what toylang's `Vec op Vec` should mean, but wants to know how much of jq's
expressive power is lost either way before choosing. Everything below was checked against a real
jq 1.8.2 binary rather than taken from a single web source; the jq snippets run as printed.

## What toylang does today

`Vec op Vec` is refused for every operator except `+`, which concatenates two `Vec`s of the
same element type (`src/check/mod.rs:3089`, the Q2 guard with the add-trait ruling, kantord/toylang#97). A comparison on any type that carries a `Vec` is refused for the same reason
(`src/ty.rs:241`, contains_vec),so `==` on two `Vec`s is part of the same open question, not a
separate one. There is no `zip` or `cross` builtin, no variable binding (`as`/`let`),and no
closure that could capture an outer list (functions take one named parameter, and lambdas are
checked-only forms with no capture machinery -- [functions](../docs/reference/syntax/functions.md),
and the closure question is still an open thread, closures-first-class-functions-research). An
index-spec `[]` must be followed by a field, index, or `!`, so `.a[]` is not a standalone
expression either ([index specs](../docs/reference/operators/specs.md)). The net: jq's
`.a[] * .b[]` has no toylang spelling today, at any level of verbosity -- it is refused,
not merely verbose.



## What jq's cartesian actually is

jq does **not** overload operators over arrays. `[1,2] * [3,4]` errors: "array and array
cannot be multiplied". The cartesian behavior lives in the *effect layer*: an operator whose
operands both produce multiple results runs over every pair, with the left varying slowest. The
manual states it for `|`: "If the one on the left produces multiple results, the one on the right
will be run for each of those results. ... This is a cartesian product, which can be
surprising",and for binary operators the same rule: "Operations that combine two filters,
like addition, generally feed the same input to both and combine the results."

So the canonical idiom is the `[]` iterator feeding both sides of an operator:

```
$ echo '{"a":[2,3],"b":[10,20]}' | jq -c '[.a[] * .b[]]'
[20,30,40,60]
```

which is sugar for the nested-iteration form, again per the manual: "`exp as $x | ...` means:
for each value of `exp`, run the rest of the pipeline with the entire original input,and with `$x`
set to that value":

```
$ echo '{"a":[2,3],"b":[10,20]}' | jq -c '.a[] as $a | .b[] | $a * .'
20
40
30
60
```

zip, by contrast, has **no** operator default in jq. It is spelled explicitly: `transpose`
pairs rows of an array of arrays, and the cookbook's "Zip column headers with their rows"
recipe binds `$headers` and indexes per row:

```
(.columnHeaders | map(.name)) as $headers
| .rows
| map(with_entries({key: $headers[.key], value}))
```

So in jq, the two operations have opposite idiom economies: cartesian rides the bare
operator; zip is always a written-down builtin or an `as`-bound index. That asymmetry is
exactly what a B-default would invert.



## The idioms, and what happens to them

### `.a[] * .b[]`: the outer product / multiplication table

The direct use is generating every pair combination -- multiplication tables, outer
products, coordinate grids:

```
$ echo '{}' | jq -c '[range(1;5) as $a | range(1;5) | $a * .]'
[1,2,3,4,2,4,6,8,3,6,9,12,4,8,12,16]
```

**Under A** this becomes toylang's first free spelling of the pattern. `.a * .b` is
legal and cartesian -- one expression, no new machinery:

```
.a * .b
```

For a nested-iteration-without-an-operator version (each pair stays a record),A does not
help: that is the `as`-form below, which needs binding that toylang lacks. A only rescues
the *operator* form.



**Under B**, `.a * .b` now zips, and `.a * .b` on the same-length pair above silently
gives the diagonal: `[20,60]` instead of `[20,30,40,60]` -- no error, no signal
(see below). To get jq's result you must call a `cross` builtin that does not exist yet:

```
cross(.a; .b; *)
```

The exact signature is unverified: Q2's implementation would define it;the shape of the
cost is what matters here. `cross` is not free either way -- it is new vocabulary, and every
call site pays for it.



**Under C**, `.a * .b` stays refused, unchanged from today. The same explicit `cross`
call is required, and now nothing silently produces a wrong answer: the only spellings are
the explicit one and the refusal.



### `[.prefix[] + .suffix[]]`: string cross-join

A common real pattern is generating every concatenation of two lists -- product codes,
URL paths, row/col label pairs:

```
$ echo '{"prefix":["x","y"],"suffix":["p","q"]}' | jq -c '[.prefix[] + .suffix[]]'
["xp","yp","xq","yq"]
```

**This one is blocked under every option**, because `+` is the one operator toylang already
settled: `Vec + Vec` concatenates ([arithmetic](../docs/reference/operators/arithmetic.md),
Q2's add-trait reading). A cartesian `+` cannot ride the operator default; it would
collide with concat. So even under A, the string cross-join needs an explicit `cross`
(`cross(.prefix; .suffix; +)`, unverified spelling), exactly as under B and C. The add-trait
ruling already removed the `+`-half of Q2 from the multiplicity question -- the round should
know that a whole family of real jq idioms (string cross-join) is on the wrong side of that
settlement regardless of A/B/C. (jq has no such collision: its `+` on streams concats
per pair, because its arrays never appear as raw operands at all. The collision is toylang's
own doing -- the value-layer `Vec + Vec` concat decision.,and it is settled.)



### The join: `.a[] as $x | .b[] | select(...)`

The general cross-product-with-a-predicate form is jq's stand-in for an unindexed join: pair up two lists and keep the pairs that match:

```
$ echo '{"a":[{"id":1,"v":"x"},{"id":2,"v":"y"}],"b":[{"id":2,"w":"p"},{"id":1,"w":"q"},{"id":1,"w":"r"}]}' | jq -c '[.a[] as $x | .b[] | select(.id == $x.id) | {a: $x.v, b: .w}]'
[{"a":"x","b":"q"},{"a":"x","b":"r"},{"a":"y","b":"p"}]
```

**This is unaffected by A/B/C**, because it never uses the operator default -- it is explicit
nested iteration already. But it needs `as`-binding, and toylang has none; named functions
cannot see both lists from inside a `map` body. So this idiom is inexpressible in toylang
today, under every option, until binding or closure machinery lands. That gap is
independent of Q2: choosing A does not buy it,and choosing C does not cost it beyond
what A already does. It is the *real* carrier of jq's cartesian power (the operator form is
the sugar;the join is the substance,and it survives the A/B/C choice entirely.)



### Scalar times a Vec: `.a[] * 2`

jq distributes a scalar over the stream:

```
$ echo '{"a":[2,3]}' | jq -c '[.a[] * 2]'
[4,6]
```

This is not part of Q2's scope:Q2 is about two multi-valued operands, and a scalar-Vec
pair is the broadcast/tensor axis (already flagged as separate in the dense-tensor research,
plans/dense-tensor-design-research.md). In toylang today the spelling is `map(. * 2)`,
and every option keeps that spelling, because none of them touches scalar-Vec. So this
idiom does not differentiate A from B from C -- worth saying once, so it is not
misremembered as an A-vs-B point.





## Where B silently diverges from A

The brief asks specifically for the case where broadcast/zip produces a *different result*
rather than an error. There is exactly one such case,and it is the load-bearing one: the two
operands have the same length.



**Same length:** cartesian gives `N*N` values;zip gives `N` (the diagonal;
silently, both typecheck, both look like plausible output:

```
$ echo '{"a":[2,3],"b":[10,20]}' | jq -c '[.a[] * .b[]]'       # A's intent
[20,30,40,60]
# B, same program:   [20,60]      # diagonal only, no error
```

Real programs hit this whenever the two lists naturally have equal lengths -- the multiplication
table over two equal ranges, row/col label grids, and coefficient lists combined for
polynomial terms. In each, B's result is a *plausible-looking subset* of the cartesian one: the
same type, the same element kind, half the values, each one a real pair product. A
program written against A, run under B, does not crash -- it silently produces a result
that looks like an undercount. That is the worst possible failure mode for a silent default:
worse than an error, because nothing points at the operator;the fix ("why are half the rows
missing?") is a debugging hunt, not a type error.



**Length one vs length N:** the two agree. jq's cartesian pairs the single element with every
element of the other ([2] * [10,20,30]` -> `[20,40,60]), which is exactly what broadcasting
the length-1 side does. So B's broadcast wing matches A here; the boundary where B
broadcast stops diverging is length exactly one, which is also the boundary where the two
interpretations coincide. Good to know, so a future B does not add a length-1 special case
that actually changes nothing.



**Mismatched lengths (neither 1 nor equal):** A gives the full `N*M` product;
B has no zip equivalent at all ([2,3] * [10,20,30] -> six values). If B refuses on
length mismatch,that is a loud difference from A, not a silent one -- but it is still a real
loss:jq idioms over ragged inputs (rows of different widths, etc. are everyday, and
cartesian handles them with no ceremony. If B truncates to the shorter instead of refusing,
that is a *second* silent divergence, the same shape as the equal-length case. So B's
zip rule needs the equal-length-handling decision stated explicitly, whichever way it lands:
refuse, or truncate -- because "zip" alone does not say which, and one of the two answers is a
silent wrong-answer machine for the most common jq idiom.





## What each option actually costs

- **A (cartesian default).** Makes `.a * .b` legal and cartesian with zero new
  machinery; the operator form of the outer-product idiom becomes free, matching jq. It
  does not rescue the `+` cross-join (settled as concat)nor the join idiom (needs
  binding regardless. Its cost: a silent-wrong-answer risk is inverted -- now a
  future B-style reader has to know that `.a * .b` is all pairs, not pairwise;but that is
  exactly jq's contract,and the round already has jq as the compatibility target.

- **B (zip/broadcast default)..** Makes `.a * .b` mean what jq spells explicitly
  (transpose / `as`-bound index),and requires a new `cross` builtin for what jq means by the
  bare operator. On same-length operands it silently produces a different result than A
  rather than refusing -- the undercount failure above. It also forces the zip-length-rule
  decision (refuse or truncate on mismatch)to be made,where one answer reintroduces
  the silent divergence for ragged data. What it buys: `.a * .b` on equal-length Vecs
  gets a free pairwise spelling that toylang also lacks today -- a real gain, but for an
  operation jq itself never gives the bare operator to.



- **C (no default, explicit `zip`/`cross`)..** Keeps today's refusals unchanged,and
  adds `zip` and `cross` as explicit builtins. No silent divergence is possible, because
  there is no default to misread. The cost is verbosity:every outer product, every
  multiplication table, every cross-join writes `cross(...)`, and the language never gains
  the free spelling A offers. The join idiom still needs binding regardless, exactly as
  under A. C is "A minus the free spelling", plus immunity from B's silent-divergence
  family -- and the verbosity is constant, not per-backend or per-length:it is one
  wrapper on every call site that A would have spelled bare.


## Answering Q2

The maintainer's stated trade is "how much of jq's magic would be lost" under B versus C.
The survey says:the magic is almost entirely the *cartesian* default. jq's bare-operator
idioms are cartesian;its zip is already explicit. So:

- A preserves the operator form of the outer-product family (`.a[] * .b[]`)with zero
  new machinery,and the only silent-divergence risk it carries is the one the round already
  has (jq's own contract, which a toylang program would be written against).
- B keeps a cartesian story alive, but moves it to a builtin that must be invented,and makes
  the bare operator mean the operation jq treats as explicit -- with the same-length undercount
  as the silent trap,and a length-mismatch rule to invent on top. It is the option that
  actively misreads the surveyed jq corpus.
- C is B without the silent trap:same new `cross` builtin, no default to misfire, and
  the verbosity is the honest price of not having a default.

The empirical case for leaning **A** is that jq's cartesian is not an incidental default:it is
how the operator vocabulary works at all -- there is no jq program that writes `.a[] * .b[]`
and means pairwise. B and C both require a new `cross` builtin to say the thing A says for
free,and B additionally manufactures a silent failure mode out of the most common shapes of
the idiom. The two things no option rescues -- the `+` string cross-join (dead by the
add-trait ruling)and the join idiom (needs binding/closures,out of Q2's scope)-- should
be recorded as the real jq-expressiveness gaps,so the round is not seduced into thinking
the A/B/C choice is where jq's power is decided. It decides the operator shorthand;the
power lives in iteration-and-binding,which toylang does not have and Q2 does not give it.



Sources:
- [jq 1.8 Manual](https://jqlang.github.io/jq/manual/v1.8/) (`|`, "This is a cartesian
  product", `as`, `combinations`, `transpose`, array-addition semantics)
- [jq Cookbook](https://github.com/stedolan/jq/wiki/Cookbook) ("Zip column headers with
  their rows", the explicit-zip idiom)
- jq 1.8.2 binary, run directly for every snippet above (outputs as printed).
- toylang: [arithmetic](../docs/reference/operators/arithmetic.md),
  [index specs](../docs/reference/operators/specs.md),
  [functions](../docs/reference/syntax/functions.md),
  `src/check/mod.rs:3089`, `src/ty.rs:241`, and Q2 in
  [questions.md](questions.md#q2-binary-operators-over-two-multi-valued-expressions-cartesian-zip-or-explicit).

Derived: the jq behaviors (verified empirically, jq 1.8.2,and from the cited manual/cookbook
passages)and their A/B/C consequences, from the option definitions and toylang's existing rulings.

Agent-invented: the recommendation to lean A on the round -- the comparison constrains it,
but preferring the free spelling over the silent-immunity trade is an agent's call.