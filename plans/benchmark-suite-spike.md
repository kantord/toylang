# Benchmark suite spike (issue #106)

Maintainer asked for a standard set of programs to rewrite in toylang and compare across its
seven backends, shaped like the Computer Language Benchmarks Game, with a hard constraint: no
license problems in whatever set gets adopted.

## Candidates

**Computer Language Benchmarks Game (CLBG).** BSD-3-Clause on the whole collection, copyright
Brent Fulgham (2004-2008) and Isaac Gouy (2005-2025), per the project's own license page
(<https://benchmarksgame-team.pages.debian.net/benchmarksgame/license.html>). BSD-3 requires only
attribution and a disclaimer on redistributed source or binaries; it does not forbid describing
the tasks in our own words or writing our own implementations, which is what AGENTS.md's "never
copy source text from other repositories" rule requires anyway regardless of license terms. Ten
tasks make up the current site (<https://benchmarksgame-team.pages.debian.net/benchmarksgame/index.html>):
fannkuch-redux, n-body, spectral-norm, mandelbrot, fasta, k-nucleotide, reverse-complement,
binary-trees, pidigits, regex-redux.

**Rosetta Code.** GFDL 1.2, later restated as GFDL 1.3
(<https://rosettacode.org/wiki/Rosetta_Code_talk:Copyrights>) -- a document license with
invariant-sections and full-license-text obligations built for prose, not source code. The site's
own creator has said he regrets not choosing a Creative Commons license for exactly this reason.
Workable in principle but the wrong tool: every obligation it carries is aimed at redistributing
documents, not at "we looked at a task description and wrote a program."

**SPEC CPU / SPECint / SPECfp.** Commercial, paid license, no public redistribution of the
workloads (<https://www.spec.org/order.html>: the benchmarks are ordered/purchased, not
downloaded). Ruled out on the license constraint alone.

**Phoronix Test Suite.** GPL v3 on the harness, but it wraps third-party benchmarks under a mix
of licenses it does not itself clear, and it is a test-runner, not a set of small programs meant
to be rewritten in a new language. Wrong shape as well as unclear license on the payload.

**TechEmpower Web Framework Benchmarks.** Clean license (repo is BSD-3-Clause, per its own
`LICENSE` file: <https://github.com/TechEmpower/FrameworkBenchmarks/blob/master/LICENSE>), but
the whole suite is HTTP round-trip / JSON serialization / DB-query shaped, aimed at comparing web
frameworks. toylang has no HTTP or database story; nothing in this suite exercises what the
language does.

## Recommendation

Adopt the ten CLBG task names as the program set, license-clean under BSD-3-Clause, and
well-known enough that the comparison means something to a reader who already knows the game.
Write each program from the task's plain description (the numerical/algorithmic definition of
"n-body simulation" or "Mandelbrot set membership" is not anyone's copyrightable expression) and
from scratch in toylang, the same way this repo already treats Project Euler problems: paraphrase
the problem, do not copy the reference implementation, cite the source
(`Derived: Computer Language Benchmarks Game task set, BSD-3-Clause,
https://benchmarksgame-team.pages.debian.net/benchmarksgame/`).

Feasibility split by toylang's actual shape (streams and pipes over structured data, recursive
`fn`s, no mutable loop variables) rather than CLBG's own categories:

- **Good fit today:** fasta, k-nucleotide, reverse-complement, regex-redux -- all text/stream
  processing over sequences, which is the language's design center.
- **Needs recursion, not loops, but plausible:** fannkuch-redux (permutations), binary-trees
  (recursive structure), pidigits (digit-by-digit generation) -- issue-76's recursive-enum and
  mutual-recursion work suggests these are reachable without a design change.
- **Open question, not this spike's job to settle:** n-body and spectral-norm are tight
  floating-point loops over mutable accumulators, the shape furthest from how toylang currently
  expresses computation. Whether they're worth forcing into the language's functional style, or
  worth dropping from the adopted set, is a design decision for whoever picks this up next, not a
  licensing one.

The CLBG site itself cautions that its numbers are "far from realistic" and not a general
performance ranking; worth carrying that caveat into wherever these results get published, since
the game's own maintainers already warn against overselling toy-program timings as more than
comparative color across this project's own seven backends.
