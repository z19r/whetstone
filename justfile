# Whetstone — run tasks via `just <recipe>`

default:
    @just --list

# Build debug binary
build:
    cargo build

# Build optimized release binary
build-release:
    cargo build --release

# Run all tests
test:
    cargo test

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Build, test, and lint in one shot
check: build test lint

# Run whetstone setup (uses cargo run)
setup *ARGS:
    cargo run -- setup {{ARGS}}

# Run whetstone uninstall
uninstall:
    cargo run -- uninstall

# Bump VERSION: just release patch|minor|major|set [version] [--tag]
release *ARGS:
    cargo run -- release {{ARGS}}

# Bump, commit, tag, and push release in one command
release-publish *ARGS:
    cargo run -- release-publish {{ARGS}}
