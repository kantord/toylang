# Fold order-dependence: how other ecosystems signal the convention

Research row [`fold-order-dependence-convention-research`](board.yaml), spun off from
[`search-and-fold-design`](board.yaml)'s applicative-fold-block-syntax round 5 answer. This is a
research writeup only -- no compiler/toylang code change. It surveys how real libraries
communicate an order-dependence (associativity) constraint on fold/reduce-shaped operations when
that constraint is a documented convention rather than a type-checked property.

## Background: what prompted this

The maintainer's round 5 answer leaned toward "reduce is the generic, order-sensitive primitive;
fold is reduce restricted to operations where order doesn't matter (associative combiners)" --
but flagged the limit of that framing in their own words:

> "i think it's more complex than that because there is also the question of associativity...
> but this definitely sounds like something that is too complex to represent in the type system,
> so i am wondering what is the good way to deal with that."

toylang has no mechanism to statically check that a combiner is associative or commutative, and
the maintainer already assumes that is out of reach for the type system. So the open question is
not "how do we check this" but "how do we *signal* it" -- what do other languages and libraries
do when an order-dependence restriction has to live in naming, documentation, or API shape rather
than in a type. The two precedents below are the verified ones; each is a different answer to the
same question.

## Precedent 1 — Rust's `Iterator::fold`: a documented left-to-right guarantee

Rust's [`Iterator::fold`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.fold) is
the canonical ordered, left-associative reduction. Its signature makes the order load-bearing:

```rust
fn fold<B, F>(self, init: B, f: F) -> B
where
    F: FnMut(B, Self::Item) -> B,
```

The accumulator is `B` and the combiner is `FnMut(B, Self::Item) -> B`, which fixes the shape to
`f(f(f(init, x1), x2), x3)` -- left-associative by construction. The documentation spells the
order guarantee out rather than leaving it implicit:

> Folds every element into an accumulator by applying an operation, returning the final result.
> `fold()` takes two arguments: an initial value, and a closure with two arguments: an
> "accumulator", and an element. The closure returns the value that the accumulator should have
> for the next iteration. **The initial value is the value the accumulator will have on the
> first call.**

"The initial value ... on the first call" is the explicit statement that elements are consumed in
iterator (left-to-right) order. The design then handles the *other* direction with a separate,
distinctly-named method rather than an argument flag: [`rfold`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.rfold)
is documented as "an iterator method that reduces the iterator's elements to a single, final
value, starting from the right, in reverse order" (the docs call the pair "foldr" / "foldl"
analogues). So Rust's answer to "order matters here" is: make the order a named, documented part
of the operation's contract, put it in the API shape (the accumulator-typed combiner), and give
the opposite order its own name. A caller who needs a commutative assumption is told explicitly
that they are not getting one from `fold` itself.

## Precedent 2 — Rayon's `reduce`/`fold`: order deliberately unspecified, op must be associative

Rayon (Rust's data-parallel library) is the same language but a different contract. Its
[`ParallelIterator::reduce`](https://docs.rs/rayon/latest/rayon/iter/trait.ParallelIterator.html#method.reduce)
and [`ParallelIterator::fold`](https://docs.rs/rayon/latest/rayon/iter/trait.ParallelIterator.html#method.fold)
cannot promise the sequential left-to-right order, because the work is subdivided across threads
and recombined. Instead of pretending otherwise, the documentation states the loss of order
guarantee explicitly and hands the responsibility to the caller:

> ... the order in which `op` will be applied to reduce the result is not fully specified. So
> `op` should be associative or else the results will be non-deterministic.

The point is that Rayon does *not* try to enforce this in the type system (Rust's `Fn` types
cannot express "is associative" either), and it does not rename `reduce`/`fold` to something
scarier. It keeps the same fold/reduce vocabulary and instead:

- states in the doc comment that the application order is unspecified;
- tells the caller the exact property required (`op` associative) to get deterministic results;
- names the failure mode ("non-deterministic") so the consequence of violating the convention is
  concrete, not vague.

Where Rust's `Iterator::fold` documents a *guarantee* (left-to-right), Rayon's `reduce`/`fold`
document a *non-guarantee* (order not fully specified) plus the condition that restores
well-defined behavior. Both are conventions carried by prose in the API reference, because neither
could be expressed in the type system.

## Implications for toylang (descriptive, no new syntax)

The two precedents bracket the design space for a fold-shaped builtin in a language that cannot
check associativity:

- **Name the order into the operation, or name the assumption into the caller's responsibility.**
  Rust's `Iterator::fold` puts the order guarantee in the name+docs and the API shape (an
  accumulator-typed combiner), and gives the reverse order its own name (`rfold`). Rayon keeps
  the fold/reduce names but attaches the order caveat and the associativity requirement to them in
  the documentation. Both treat the order-dependence property as part of the operation's
  documented contract rather than as something the type system can be expected to verify.

- **Documentation is the mechanism both real ecosystems rely on.** Neither precedent type-checks
  associativity; each says the required property in prose and gives the caller a concrete failure
  mode (wrong results in Rayon's case) if the property is violated. This is the pattern toylang
  would be following if it also keeps the constraint out of the type system.

- **The naming choice (fold vs reduce) and the order guarantee are separable axes.** Rust shows
  an ordered `fold` can exist with a fully specified order; Rayon shows an unordered `reduce` can
  exist with an explicitly unspecified order. A language could pick either spelling and still have
  to answer the same documentation question -- "does this operation promise a particular order,
  and is the caller told what to assume?" -- which is exactly the associativity question the
  maintainer flagged.

None of the above is a proposal. This section only records what the two verified precedents do and
why: they make the order/associativity convention explicit in the API's documented contract,
because no type system in the survey can check it. What toylang should name or document is the
next grilling round's decision, not this research's.
