# List available recipes.
default:
    @just --list

# The final gate: the full test suite. Landing requires this green.
test:
    cargo nextest run --workspace

# The fast inner loop: skips the docs mega-test, the suite's ~137s long pole. Gate on `just test`.
check:
    cargo nextest run --workspace -E 'not test(every_fragment_is_a_real_program)'

# Opt-in: fetch this machine's own copies of the Euler 8/11/13/18/22 puzzle data from
# projecteuler.net (CC BY-NC-SA 4.0, see scripts/fetch_euler_data.py) into a local, gitignored
# cache. Re-run any time; a file already cached is left alone unless --force is passed.
euler-fetch DIR=".euler-data" *ARGS="":
    python3 scripts/fetch_euler_data.py {{DIR}} {{ARGS}}

# Opt-in: run the skipped Euler 8/11/13/18/22 programs against real puzzle data in DIR (fetched
# automatically if not already cached there), outside `just test`. Fails loudly, never skips.
euler-data DIR=".euler-data": (euler-fetch DIR)
    EULER_DATA={{DIR}} cargo nextest run --run-ignored ignored-only -E 'test(euler_real_data)'

# Re-run the slow-fragment tier: the same suite, but `slow` fragments are executed on every
# backend rather than only type-checked and emitted. The tier exists so `just test` stays fast;
# this is where the deferred execution claim is re-verified.
slow-test:
    TOYLANG_SLOW=1 cargo nextest run

# Run clippy with the repo's lint set, the same surface the Stop hook checks.
clippy:
    cargo clippy --workspace --all-targets

# Time one benchmark (a name under benches/programs/) across every backend with hyperfine.
# Design: plans/benchmark-plan.md. Results land in benches/results/<name>.{md,json}.
bench NAME:
    cargo run -q --bin bench -- {{NAME}}

# Regenerate the syntax-highlighting grammar (syntax/, editors/vscode/) from src/parse.rs's own
# token vocabulary. Run after adding/removing/renaming a keyword or operator in the lexer;
# tests/syntax_grammar.rs fails `just test` until you do.
gen-syntax:
    cargo run -q --bin gen_syntax

# Formatter check over every .toy file from the repo root down (exit 1 on drift).
fmt:
    cargo run -q --bin toylang -- fmt

# Rewrite drifted files in place (same exit code as the check).
fmt-write:
    cargo run -q --bin toylang -- fmt --write

# The repo's mechanical checks, the same surface the Stop hook runs.
checks:
    bash .claude/checks/run.sh

# The autonomous drive loop: a tick every 600s, live colorized output, Ctrl-C to stop. Run ONE.
drive:
    uv run --project .claude/scripts .claude/scripts/drive_loop.py

# Fire one coordinator tick right now (zero tokens if there is nothing to do).
tick:
    uv run --project .claude/scripts .claude/scripts/drive_tick.py

# Watch the current coordinator tick live (detaches with Ctrl-C, tick untouched).
peek:
    uv run --project .claude/scripts .claude/scripts/tick_peek.py
