# List available recipes.
default:
    @just --list

# The final gate: the full test suite. Landing requires this green.
test:
    cargo nextest run

# The fast inner loop: skips the docs mega-test, the suite's ~137s long pole. Gate on `just test`.
check:
    cargo nextest run -E 'not test(every_fragment_is_a_real_program)'

# Opt-in: run the skipped Euler 8/11/13/18 programs against real puzzle data in DIR, outside
# `just test`. DIR holds your own copies of the raw data texts; fails loudly, never skips.
euler-data DIR:
    EULER_DATA={{DIR}} cargo nextest run --run-ignored ignored-only -E 'test(euler_real_data)'

# Run clippy with the repo's lint set, the same surface the Stop hook checks.
clippy:
    cargo clippy --workspace --all-targets

# Formatter check over every .toy file from the repo root down (exit 1 on drift).
fmt:
    cargo run -q -- fmt

# Rewrite drifted files in place (same exit code as the check).
fmt-write:
    cargo run -q -- fmt --write

# The repo's mechanical checks, the same surface the Stop hook runs.
checks:
    bash .claude/checks/run.sh

# The autonomous drive loop: a tick every 600s, live colorized output, Ctrl-C to stop. Run ONE.
drive:
    .claude/scripts/drive-loop.sh

# Fire one coordinator tick right now (zero tokens if there is nothing to do).
tick:
    .claude/scripts/drive-tick.sh

# Watch the current coordinator tick live (detaches with Ctrl-C, tick untouched).
peek:
    .claude/scripts/tick-peek.sh
