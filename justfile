# List available recipes.
default:
    @just --list

# The final gate: the full test suite. Landing requires this green.
test:
    cargo nextest run

# The fast inner loop: skips the docs mega-test, the suite's ~137s long pole. Gate on `just test`.
check:
    cargo nextest run -E 'not test(every_fragment_is_a_real_program)'

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
