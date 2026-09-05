# Benchmark plan: the set, the harness, and how the numbers compare

Synthesis of the two spikes: the program set from gh:106
([`benchmark-suite-spike.md`](benchmark-suite-spike.md)) and the harness from gh:107
([`benchmark-tooling-spike.md`](benchmark-tooling-spike.md)). Each spike already settled its half;
this file is where the two halves stop being separate questions and become one runnable shape.
Where the spikes left a decision open, this plan says what it assumes and names what it does not.

## The program set

Adopt the ten task names from the Computer Language Benchmarks Game, license-clean under
BSD-3-Clause, written from each task's plain description in toylang and cited as derived from the
CLBG set (`Derived: Computer Language Benchmarks Game task set, BSD-3-Clause,
https://benchmarksgame-team.pages.debian.net/benchmarksgame/`). Write the programs the way this
repo already treats Project Euler: paraphrase the problem, never copy a reference implementation.

Feasibility today splits by toylang's own shape, not by CLBG's categories:

- **Good fit:** fasta, k-nucleotide, reverse-complement, regex-redux -- text and stream
  processing, the language's design center.
- **Reachable with the recursion work already landed:** fannkuch-redux, binary-trees, pidigits.
- **Open:** n-body and spectral-norm, tight floating-point loops over mutable accumulators. See
  the escalation note below; the plan's default is to keep them named in the set but not blocked
  on them.

That split is the one real judgment call the set spike leaves to this plan, and it is not settled
by the spikes' text, so it is flagged rather than silently decided.

## The harness

hyperfine drives every backend as a real subprocess, using `--parameter-list` to sweep one
command template across the seven backends, `--warmup` to stabilize Go and the JIT-adjacent
interpreters, `--input <file>` to feed corpus fixtures where a case has one, and
`--export-markdown`/`--export-json` for reporting. The arguments for this over criterion and
divan, and why the in-process tools are the right fit only for a future compiler-throughput
benchmark, are in the tooling spike and are not restated here.

The benchmark runner is a thin sibling of `run_on` (`src/lib.rs`), not `run_on` itself. Three
retargetings make the timed number mean the same thing per backend:

- **Go:** `run_go` uses `go run`, which compiles on every sample. Build once with `go build`
  outside the timed loop, then let hyperfine time the binary the way it times Rust and Native.
- **Lua:** `run_lua` runs inside the process through the embedded `mlua::Lua`. hyperfine can only
  time a spawned process, so write the emitted source to a file and run it through the system
  `lua5.4` (already a CI dependency) -- which also makes Lua's number comparable, real spawn plus
  real interpreter startup like every other row.
- **Rust and Native** already compile ahead of the run via `link_rust` and `link` and hand back a
  bare executable path; reuse those two functions directly.

So the runner shares the emit step with `run_on` but splits compile-once from run-many, which is
exactly the boundary `run_on` currently blurs.

## How results compare against other languages

A comparison needs the same program in toylang and in the comparison languages, run by the same
harness. The concrete shape:

- toylang runs across its seven backends from the emitted source, as above.
- The comparison rows are the same task written in jq, Python, Node, Go, Rust, C, and Lua --
  toylang's own backend languages are the obvious baseline set, because they are already
  installed and the comparison asks "what does this emitted program cost next to the language it
  is emitted into?" rather than "how does toylang compare to an arbitrary third party."
- Every row is timed by the same hyperfine invocation with the same `--warmup`, `--runs`, and
  input, so the only variable is the language/backend the program was written or emitted in.

Carry the CLBG site's own caveat wherever the numbers land: the game's maintainers call its
results "far from realistic" and not a general performance ranking. These timings are comparative
color across this project's backends and their host languages, not a claim about toylang in the
abstract.

## CI reporting

Post hyperfine's `--export-markdown` table as a job summary or PR comment -- enough to answer
"did this change make anything slower" by eye on a single run. The regression-dashboard option
(`github-action-benchmark`, requiring a small JSON conversion from `--export-json`) stays a real
option, deliberately not built now: there is no baseline worth protecting until this plan runs
for the first time, and a moving dashboard is only worth its upkeep once a specific regression it
would have caught actually happens.

## Build status (2026-09-03)

The harness landed as designed: `src/bin/bench.rs` (`just bench NAME`) compiles a
`benches/programs/<name>.toy` benchmark once, builds Go/Rust/Native ahead of the timed run,
retargets Lua at the system `lua5.4` (writing the `t_input` global into the script text itself,
since there is no embedding host to set it the way `run_lua` does), and drives `hyperfine`
`--shell=none` over all seven. Results export to `benches/results/<name>.{md,json}` (gitignored).
Today's harness feeds at most one `Int` value from stdin, or nothing -- see the doc comment atop
`bench.rs` for what extending it to a richer input type needs.

`binary-trees` is the one task landed so far (`benches/programs/binary-trees.toy`, correctness
pinned at `tests/corpus/binary_trees_node_count.yaml`): build a perfect binary tree of a given
depth, count its nodes. Simplified from CLBG's actual multi-tree, GC-stress variant (a stretch
tree, a long-lived tree, and a loop of many trees at each depth) to the single build-and-count
core, which is what the language can express today; the loop-of-many-trees shape adds nothing a
recursive count doesn't already exercise, so it was left out rather than force-fit.

**The suite spike's "good fit today" claim for fasta/k-nucleotide/reverse-complement/regex-redux
does not hold against the current builtin set.** `chars(s)` decodes `Str` to `Vec<Char>`, but
`Char` has no wire form and there is no `Char -> Str` builtin and no `Str` slice/index operator
(`docs/reference/types/char.md`; confirmed empirically, not just read off the docs, since the
docs could themselves be stale) -- so a program cannot decode an existing string, transform it
character by character, and print the result. That blocks reverse-complement (decode, complement,
re-encode) and k-nucleotide (decode into k-length windows, print each as a string) outright.
regex-redux additionally has no regex engine to build on. fasta is different: it only *generates*
text from a small fixed alphabet of literal `Str` values chosen by a computed index, never
decodes anything, so it stays buildable -- just not yet built. pidigits needs digits of pi beyond
what a 32-bit or even 64-bit accumulator holds; the spigot algorithm's usual unbounded-bignum
shape has no home in a language with no bignum type, so it is blocked the same way the float
tasks are, on a type the language does not have yet.

Of the eight, `binary-trees` is landed; `fasta` and `fannkuch-redux` (both fully numeric or
literal-driven, no decode needed) are the next real candidates; `mandelbrot` needs float support
most backends don't have yet, the same gate n-body/spectral-norm are already behind; `pidigits`
needs a bignum type; `reverse-complement`, `k-nucleotide`, and `regex-redux` need `Str`
slicing or a `Char -> Str` builtin, neither of which exists.

## What this plan leaves to the next person

- Whether n-body and spectral-norm stay in the initial set, and whether float semantics (the
  `q37-float-semantics` / `float-build` board rows, both still `todo`) gate them.
- A dashboard, when there is a baseline and a specific regression it would have caught.

These are named rather than resolved here because the spikes do not settle them.
