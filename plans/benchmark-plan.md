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

The thesis these numbers publish is a general cross-language competitiveness claim: on the same
task, run by the same harness, how does a toylang program measure up against the same task
written in each comparison language -- not merely how toylang's own backends compare to one
another. The ruling (gh:147) settled this framing: toylang is measured against the other
languages as a general competitiveness claim about the language, not the narrower "comparative
color across this project's backends" reading.

The concrete shape stays the same program in toylang and in the comparison languages, run by the
same harness:

- toylang runs across its seven backends from the emitted source, as above.
- The comparison rows are the same task written in jq, Python, Node, Go, Rust, C, and Lua --
  toylang's own backend languages are the obvious baseline set, because they are already
  installed and each is a real, widely used language in its own right. Reading the numbers
  against those rows is a competitiveness claim about toylang, not an internal accounting of
  what an emitted program costs next to its host.
- Every row is timed by the same hyperfine invocation with the same `--warmup`, `--runs`, and
  input, so the only variable is the language the program was written or emitted in.

Carry the CLBG site's own caveat wherever the numbers land, stated as a caveat on the claim
rather than a retreat from it: the game's maintainers call its results "far from realistic" and
not a general performance ranking, so no single row is a general performance ranking of toylang.
The claim is still real -- toylang is measured against the other languages on these tasks -- but
it is bounded by the tasks, the harness, and the machine the numbers were taken on.

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

`fasta` is landed (`benches/programs/fasta.toy`, correctness pinned at
`tests/corpus/fasta_generate.yaml`): ONE repeats an ALU fragment to `2n` bases, TWO draws `3n`
from the IUB ambiguity codes, THREE draws `5n` from a nucleotide frequency table. The language
has no float type and no `Str` indexing, so the weighted pick runs in integer arithmetic -- the
classic `IM=139968 IA=3877 IC=29573` LCG, with the random value compared against cumulative
integer weights -- and each symbol is a literal one-char `Str` chosen by the computed index, so
no `Char -> Str` conversion is needed. The base count is threaded through the generators rather
than re-read from stdin inside a function body, because the emitted code reads `parse(stdin)`
off the one top-level input value on the compiled backends. It deviates from CLBG's byte-exact
output the way binary-trees does: same task, integer arithmetic instead of doubles, our own
pinned corpus output that every backend agrees on.

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

`fannkuch-redux` is landed (`benches/programs/fannkuch-redux.toy`, correctness pinned at
`tests/corpus/fannkuch.yaml`): for N, generate every permutation of [1..N], and for each one
flip the leading block of length equal to its first element until that element is 1, counting
flips; track the maximum and a checksum, each permutation's flips signed by its parity. The
parity is inversion parity, so the sign is order-independent rather than a property of the
enumeration order; that makes the checksum differ from the CLBG site's index-based one, which
this recursive generation (build permutations by inserting the first element at every position
of each permutation of the rest) does not reproduce. Both verify that all N! permutations were
visited, and the maximum flip count, the benchmark's real number, matches CLBG. Every backend
runs it -- none of the seven refuses, since it needs only the `Vec<Int>` slice, reverse, and
index operations every backend already carries.

The second wave (2026-09-18), the three float tasks gh:146 deferred until `Float` existed, is
landed: `n-body` (`benches/programs/n-body.toy`, `tests/corpus/n_body.yaml`), `spectral-norm`
(`spectral-norm.toy`, `spectral_norm.yaml`), and `mandelbrot` (`mandelbrot.toy`,
`mandelbrot.yaml`). All seven backends run all three and agree. Each one works around a gap
in what `Float` can do today, and the workaround is written in toylang rather than faked:

- **No square root.** n-body needs one per body pair per step and spectral-norm one at the
  end. Both carry a `sqrt` that is a Newton iteration: start at the mean of x and 1 (never
  below the root), halve toward the root until a step stops decreasing. Deterministic IEEE
  arithmetic, so every backend prints the same digits, but the last digit is not the
  correctly-rounded one a builtin would give. n-body's energies for N = 1000 agree with
  CLBG's published `-0.169075164` / `-0.169087605` to the nine decimals CLBG prints, and
  spectral-norm's N = 100 result begins with CLBG's `1.274219991`.
- **No `Int -> Float` conversion** (`i64` is the only bridge, and it is Int to Int64).
  spectral-norm's matrix entry and mandelbrot's pixel coordinate are Float formulas over an
  index, so each loop carries the index twice, an Int for indexing or counting and a Float
  for the formula, both stepped by one. mandelbrot needs the grid size as a Float once, and
  `to_float` counts up to it by repeated addition.
- **`sum` refuses `Vec<Float>`**, so spectral-norm's dot products are accumulator recursions.
- **No Bool literal**, so spectral-norm's transpose flag is an Int.
- **No binary output.** mandelbrot writes the plain-text PBM (P1), the same bits CLBG packs
  into P4, one `0`/`1` character per pixel.

n-body prints its two energies through `jsonlines` over a `Vec<Float>`, the nested-Float shape
the jq backend prints in its own notation (`float-jq-nested-in-container`). For these values
the notations coincide, so jq agrees byte for byte; a different step count could land on a
value where they differ, and then jq's line is the known divergence, not a reason to drop it.

Building the wave found two backend bugs, fixed alongside with their corpus cases: the Lua
backend omitted its Float printer when the only Float reached the output through a
`jsonlines` callback (`jsonlines_of_floats.yaml`), and the native backend crashed the compiler
on `v[i]!` over a `Vec<Float>`, handing back the slot pointer instead of decoding the bits
(`vec_float_index_unwrap.yaml`).

Of the ten CLBG tasks, six are landed. `pidigits` needs a bignum type; `reverse-complement`,
`k-nucleotide`, and `regex-redux` need `Str` slicing or a `Char -> Str` builtin, neither of
which exists.

## What this plan leaves to the next person

- A `sqrt` builtin and an `Int -> Float` bridge. Each would let the float benchmarks drop a
  workaround, and `sqrt` would also make n-body's digits CLBG's rather than a few ulps off.
- A dashboard, when there is a baseline and a specific regression it would have caught.
