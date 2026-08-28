# List available recipes.
default:
    @just --list

# Run the test suite.
test:
    cargo nextest run

# Run clippy with the repo's lint set, the same surface the Stop hook checks.
clippy:
    cargo clippy --workspace --all-targets
