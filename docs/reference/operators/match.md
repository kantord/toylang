# Match

A match is a chain of produce-or-decline arms over the subject `.`: the subject arrives
through a pipe, arms compose with `or`, and the first arm that matches wins. An arm's left
side is a variant pattern of an [enum](../types/enum.md) subject, or a Bool guard over any
subject.

```case
enum_match
```

Four arm shapes:

- `point -> ...`: a unit variant, by name.
- `circle{r} -> r * r`: a payload variant with a record pattern, binding fields fresh.
- `text -> .body`: a bare payload arm; `.` rebinds to the payload, so a scalar payload is
  used whole and a record payload is projected into.
- `. % 3 == 0 -> "Fizz"`: a guard arm, matching when the Bool is true; `.` stays the
  subject in both the guard and the body.

The chain's final element may be a bare expression, the default; `any() -> ...` is the
default spelled as an arm, standing in for every variant nothing named:

```case
enum_match_default
```

A guard chain with a bare default is the conditional chain in another notation -- FizzBuzz
barely changes shape written either way:

```case
fizzbuzz_arms
```

Totality is a hybrid. A chain with variant patterns is closed-world: every variant handled,
or a default at the end, and missing one is a compile error naming it (the
[enum page](../types/enum.md) shows it). Guards do not count toward that coverage. A pure
guard chain is open-world and may be honestly partial: with no default, declining every arm
yields `Opt` -- the same answer indexing gives to reaching past what's there:

```case
match_partial_guards
```

A declared `Opt<T>` return reaches the arms as `T`: the chain's own partiality is what
supplies the `Opt`, so an arm never has to spell it. An arm whose body is itself `Opt`-typed
doubles the wrapping instead of colliding with it -- `none` (no arm matched) and `some(none)`
(matched, and found nothing) stay distinct values, even though both print `null`.

`or` is also Bool disjunction, and which one a given `or` is depends on where it sits: an
arm's left side is still being read, so `or` there joins guard clauses, while an arm's body is
finished, so `or` there ends it and starts the next arm. The
[boolean operators page](boolean.md) has both readings side by side; the only rule that costs
anything is that a Bool `or` written directly in an arm body needs parens.

Matching is decoding, not control flow bolted on: a match is how a value whose shape is
unknowable until runtime becomes typed data again.
