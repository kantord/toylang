# List available recipes.
default:
    @just --list

# Run the test suite.
test:
    cargo nextest run
