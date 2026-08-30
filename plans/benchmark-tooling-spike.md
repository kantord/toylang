# Spiking the benchmark harness: hyperfine over criterion or divan

Issue #107, the tooling half of the benchmark cluster (`plans/board.yaml`: `benchmark-tooling-spike`,
paired with `benchmark-set-spike` gh:106 for the program set and `benchmark-synthesis` gh:108 to
combine them). This file answers one question only: what runs and times the benchmarks, and how
the numbers reach CI. Which toylang programs to run is gh:106's decision, not this one.

## What a toylang benchmark actually measures

`Backend::ALL` is seven targets (`src/lib.rs`), and six of them are not Rust function calls --
they are separate OS processes in a separate language runtime:

- `jq`, `python3`, `node` interpret the emitted source directly (`run_jq`, `run_py`, `run_node`).
- Go's `run_go` shells out to `go run`, which compiles and executes in one step.
- Rust and Native (LLVM) are compiled ahead of the timed run (`link_rust` via `rustc`, `link` via
  `cc`) and then executed as a plain binary (`run_binary`).
- Lua is the one exception: `run_lua` runs the emitted source through an embedded `mlua::Lua`
  interpreter living inside the same process as the test harness, not a subprocess.

A benchmark harness that only knows how to time a Rust function in-process -- which is what
criterion and divan are -- covers zero of the six subprocess backends and has to invent a way to
reach into the seventh. Timing whole external processes, in a machine that also has to run `go`,
`node`, `python3`, `jq`, and a C-family compiler, is the actual job.

## criterion: dropped exactly this use case

criterion 0.3 removed built-in external-program benchmarking outright; the documented
replacement, `iter_custom`, has the harness pass an iteration count into the child process and
the child loop that many times internally and print the elapsed nanoseconds back on stdout
(bheisler/criterion.rs, `book/user_guide/timing_loops.html`). That protocol has to be built into
whatever the child runs. Here the child is the *emitted program itself* -- a `.go` file, a `.py`
file, a jq filter string -- so making `iter_custom` work would mean adding a loop-N-times-and-report
convention to six independent codegen backends, purely to satisfy the benchmark harness's
calling convention. That is not a benchmarking cost, it is a second CLI protocol for every
backend, permanently.

## divan: same shape, no escape hatch

divan (nvzqz, actively maintained: 0.1.21 as of six months ago, 761k downloads/month) is the
same kind of tool as criterion -- `#[divan::bench] fn foo()` wraps a Rust call and divan measures
it in-process, with allocation profiling and throughput counters layered on top
(docs.rs/divan). It has no equivalent of `iter_custom`: no path for handing it a duration that
was measured somewhere else. It is a good fit for benchmarking Rust functions and a non-fit for
benchmarking `node program.js`, for the same reason criterion is, minus even the workaround.

Both tools remain the right answer for a benchmark that *is* an in-process Rust function -- most
plausibly toylang's own `parse` / `check` / emit passes, which run inside this binary and never
spawn anything. That is a different, narrower benchmark than "how fast does the emitted program
run," and nothing here rules it out later if compiler throughput itself becomes a question worth
tracking. It is out of scope for gh:107 as filed.

## hyperfine: built for the case this project has

hyperfine (sharkdp, already installed in this environment: `hyperfine 1.20.0`) benchmarks
arbitrary commands from outside: spawn, measure wall time (plus user/system CPU time from the
OS), repeat, warm up first, flag outliers, and report mean/stddev/median/min/max plus every
individual run. It does not care what language the command is in, which is exactly the property
six of the seven backends need. Relevant specifics, verified against current docs rather than
assumed:

- `--warmup N` runs the command N times uncounted first, which matters for Go (`go run` may hit
  the build cache differently on the first invocation than the tenth) and for the JIT-adjacent
  warmup effects interpreters can have.
- `--input <file>` feeds a file to the command's stdin, which covers every corpus case that has
  an `input:` fixture -- no shell redirection or `--shell=none` juggling needed for the common
  case, and `--shell=none` is still available for the argv-only cases to remove shell startup
  from the measurement entirely.
- `--parameter-list` sweeps one command template over a list of values -- the natural fit for
  "same benchmark, seven backends," rather than seven separate hyperfine invocations to reconcile
  by hand.
- `--export-json` writes the full per-run data (`times`, plus `mean`/`stddev`/`median`/`min`/`max`/
  `user`/`system`), and `--export-markdown` writes a comparison table in the form GitHub renders
  directly in an issue or PR body. Both from one run, no separate reporting pass.

hyperfine ships no library crate to depend on from `Cargo.toml` -- it is a CLI binary, invoked as
a subprocess of the benchmark runner the same way `run_go`/`run_py`/etc. already invoke their
targets. That is not a gap here: nothing in this project's existing benchmark-adjacent tooling
(`run_subprocess` and friends in `src/lib.rs`) is a Rust API either, and CI already has to install
a toolchain per backend regardless of which timer drives it.

## What the existing backend runners get wrong for timing, on purpose

`src/lib.rs`'s backend runners are built to make correctness-testing convenient, not to isolate
what a benchmark needs excluded:

- `run_go` uses `go run`, which compiles on every invocation. Timing that measures compile time
  plus run time on every sample; a fair per-backend comparison needs `go build` once, outside the
  timed loop, then hyperfine timing the resulting binary the same way it times the Rust and
  Native ones.
- `run_lua` never leaves the process -- it hands the emitted source to an embedded `mlua::Lua`.
  hyperfine can only time something it spawns, so a Lua benchmark needs the emitted source written
  to a file and run through the system's `lua5.4` (already a CI dependency, `.github/workflows/
  ci.yml`) rather than through `mlua`. This also makes Lua's number comparable to the others: real
  process spawn plus real interpreter startup, like every other row.
- Rust and Native already compile ahead of the run (`link_rust`, `link`) and hand back a bare
  executable path, which is exactly the shape hyperfine wants; no change needed there beyond
  reusing those two functions instead of `run_on`.

So the benchmark runner is a thin sibling of `run_on`, not `run_on` itself: same emit step, a
compile-once/run-many split where `run_on` currently compiles-and-runs together, and Lua and Go
retargeted at real subprocesses. This is a detail for gh:108 to design, not a foreclosure of it --
worth flagging now because a synthesis plan that assumes the existing runners can be timed as-is
will get a compile-time-polluted Go number and no Lua number at all.

## CI reporting

hyperfine's `--export-markdown` output is a table GitHub already renders -- posting it as a job
summary or a PR comment costs nothing beyond `--export-markdown bench.md` and an upload/comment
step, and is enough to answer "did this change make anything slower" by eye on a single run.

Tracking a number over time (a regression dashboard, "5% slower than last week") is a separate
commitment: `benchmark-action/github-action-benchmark` does this by diffing a JSON file across
commits and failing the build past a threshold, but it has no hyperfine-native format (open
request, `benchmark-action/github-action-benchmark#22`, unresolved) -- reaching it means mapping
`--export-json`'s `{name, mean, stddev, ...}` entries into the action's `customSmallerIsBetter`
shape (`{name, unit, value, range}`), a small conversion step rather than a supported path.
That is a real option, not a recommendation to build now: a moving dashboard is only worth its
upkeep once there is a baseline worth protecting, and there is no baseline until gh:106 and
gh:108 pick what runs. Start from the markdown table; add the dashboard when a specific
regression it would have caught actually happens.

## Recommendation

hyperfine, driving each backend as a real subprocess (building Go/Rust/Native once per benchmark
run rather than per sample, and running Lua through the system interpreter rather than the
embedded one), with `--export-markdown` in CI now and `--export-json` kept as the on-ramp to a
`github-action-benchmark` dashboard later if the project decides it wants one. criterion and
divan stay the right tools if a purely in-process Rust benchmark -- the compiler's own passes --
ever gets proposed, but neither one can time the six subprocess backends this issue is actually
about.
