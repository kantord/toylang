# Escalation: whether n-body and spectral-norm stay in the initial benchmark set

Task gh:108 asked me to combine the benchmark-set spike (gh:106) and the tooling spike (gh:107)
into the benchmark plan. Both spikes settled their halves firmly, but the set spike explicitly
leaves one design decision to "whoever picks this up next" rather than settling it itself:

> n-body and spectral-norm are tight floating-point loops over mutable accumulators, the shape
> furthest from how toylang currently expresses computation. Whether they're worth forcing into
> the language's functional style, or worth dropping from the adopted set, is a design decision
> for whoever picks this up next, not a licensing one.

The brief did not settle this, and it is a real design decision (more than one reasonable option,
not forced by a constraint), so I am recording it rather than silently choosing.

## What I assumed while the decision is open

The most conservative continuation, taken in `plans/benchmark-plan.md`:

- Keep all ten CLBG task names in the adopted set, including n-body and spectral-norm, so the
  set stays the well-known ten and nothing is dropped on the strength of a guess.
- Do not gate the plan (or the harness work) on them. The harness, the other seven tasks, and the
  comparison method are all decidable without the float programs, so those proceed.
- Flag that float semantics (board rows `q37-float-semantics` and `float-build`, both still
  `todo`) plausibly gate the two float programs, and leave that link for whoever decides.

## To close

Decide either way: (a) keep n-body and spectral-norm in the initial set, treating float work as
a prerequisite for them; or (b) defer them to a second wave and let the initial set be the other
eight tasks. Then update `plans/benchmark-plan.md`'s "The program set" and "What this plan leaves
to the next person" sections to match. Nothing else in the plan changes.
