Board row `match-type-wrapper-experiment` (gh:122): experimentally build a `Match<T>`
wrapper.

Design: `.` entering a `|` chain wraps the scrutinee as `Match<T>`; traits like `%` get
generic impls over `Match<T>` per underlying `T`, applying only past a passing boolean
guard; an arm's aliases become named fields on a result struct the match produces. This is
a spike to see what goes wrong before ratifying the design (gh:122 ruling) -- read the
existing match implementation and trait-impl machinery first (look for how `|` chains and
trait dispatch are currently implemented in the compiler) before writing code.

Build the experiment, run `just check`, and write up what worked and what broke (type
inference gaps, trait resolution ambiguity, parser conflicts, whatever surfaces) to
`plans/match-type-wrapper-experiment-findings.md`, committed alongside any experimental
code. This is exploratory -- a working experiment plus a clear writeup of the failure
modes is the done-gate, not a production-ready feature.
