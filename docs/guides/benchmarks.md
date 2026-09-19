# Benchmarking toylang against other languages

The benchmark thesis this repo publishes is a general cross-language competitiveness claim: on
the same task, run by the same harness, toylang is measured directly against the same task in
the languages it competes with -- jq, Python, Node, Go, Rust, C, and Lua. These are not
internal accounting of how toylang's own backends compare to one another, and the numbers are
not a retreat into "comparative color across this project's backends." They are a claim about
how toylang competes, in the abstract, with other languages on these tasks.

**Caveat, in the game's own words, near the top because it belongs on every reading of the
numbers:** the tasks come from the Computer Language Benchmarks Game, whose maintainers call
their own results "far from realistic" and explicitly not a general performance ranking. That
caveat attaches to this claim too. Each row below is a single task, run once on one machine,
and no row is a general performance ranking of toylang. The claim is still real -- toylang is
measured against the other languages on these tasks -- it is just bounded by the tasks, the
harness, and the machine the numbers were taken on.

The program set, the harness, and the comparison framing live in the
[benchmark plan](../../plans/benchmark-plan.md). Each benchmark is a program under
`benches/programs/`; `just bench <name>` compiles it once, builds the compiled backends ahead of
the timed run, drives `hyperfine` across all seven, and writes
`benches/results/<name>.md` and `benches/results/<name>.json`.

## binary-trees

Build a perfect binary tree of the given depth and count its nodes -- CLBG's
[binary-trees](https://benchmarksgame-team.pages.debian.net/benchmarksgame/description/binarytrees.html)
task, simplified to the single build-and-count core. Timed with `hyperfine`, depth 14, one
`Int` read from stdin. Every row is the same program; the only variable is the language the
program was written or emitted in:

```text
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `lua` | 24.5 ± 1.8 | 22.2 | 31.8 | 17.60 ± 4.15 |
| `js` | 82.4 ± 3.8 | 76.4 | 92.6 | 59.27 ± 13.51 |
| `native` | 3.1 ± 0.6 | 2.5 | 5.4 | 2.24 ± 0.65 |
| `jq` | 81.3 ± 2.0 | 78.4 | 87.5 | 58.46 ± 13.13 |
| `go` | 1.4 ± 0.3 | 1.0 | 3.3 | 1.00 |
| `py` | 30.7 ± 1.6 | 28.6 | 37.5 | 22.11 ± 5.07 |
| `rust` | 58.4 ± 3.8 | 54.6 | 79.1 | 42.01 ± 9.78 |
```

The rows are toylang's emitted program on each of its seven backends: `lua` (lua5.4), `js`
(node), `native`, `jq`, `go`, `py` (python3), and `rust` (rustc). Read the same way the plan
does -- toylang measured against the language each backend emits into as a general
competitiveness claim, with the CLBG caveat above applied to every number.

More tasks land as the language grows (see the plan's build-status notes and its "what this
plan leaves to the next person" list); each one publishes its own table here the same way.
