# SPL Compiler — common development commands

# List available recipes
default:
    @just --list

# Build all crates
build:
    cargo build

# Build in release mode
build-release:
    cargo build --release

# Run all checks (clippy + all tests)
check: lint test

# Run clippy with warnings as errors
lint:
    cargo clippy --all-targets -- -D warnings

# Format code (requires nightly)
fmt:
    cargo +nightly fmt

# Check formatting without applying changes
fmt-check:
    cargo +nightly fmt -- --check

# Run all tests (unit + spec, across all crates)
test:
    cargo test

# Run parser unit tests only
parser-tests:
    cargo test -p spl-parser

# Run TOML spec tests
spec-tests:
    cargo test -p spl-test-runner

# Run a specific spec test file (e.g., just spec-file functions)
spec-file name:
    cargo test -p spl-test-runner -- {{name}}

# Run tests with single thread (useful for debugging panics)
test-serial:
    cargo test -- --test-threads=1

# Clean build artifacts
clean:
    cargo clean
