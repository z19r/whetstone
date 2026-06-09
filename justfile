# Whetstone — project command runner
# https://github.com/casey/just

set dotenv-load := false

version := `grep '^version' Cargo.toml | head -1 | cut -d'"' -f2`

# ─── Default ──────────────────────────────────────────────────────

default:
    @just --list --unsorted

# ─── Development ──────────────────────────────────────────────────

# Build debug binary
build:
    cargo build

# Build optimized release binary
build-release:
    cargo build --release

# Install whetstone to ~/.cargo/bin
install:
    cargo install --path .

# Run whetstone with arguments (e.g. `just run -- setup --full`)
run *ARGS:
    cargo run -- {{ ARGS }}

# Watch for changes and rebuild on save
watch:
    cargo watch -x check -x 'test -- --nocapture'

# Check compilation without producing binaries
check:
    cargo check --all-targets --all-features

# Open whetstone in Cursor classic editor (full LSP)
edit-classic:
    cursor --classic {{ justfile_directory() }}

# Diagnose rust-analyzer / Cursor LSP wiring
ra-doctor:
    #!/usr/bin/env bash
    set -euo pipefail
    BUNDLED="/home/zrk/.cursor/extensions/rust-lang.rust-analyzer-0.3.2921-linux-x64/server/rust-analyzer"
    echo "=== rust-analyzer doctor ==="
    echo "Extension server: $BUNDLED"
    if [[ -x "$BUNDLED" ]]; then
        "$BUNDLED" --version
    else
        echo "MISSING: install rust-analyzer extension in Cursor"
        exit 1
    fi
    echo
    echo "Running analysis-stats (proves RA can index this crate)..."
    timeout 60 "$BUNDLED" analysis-stats . | tail -5
    echo
    echo
    echo "=== Cursor Agents window (likely root cause) ==="
    echo "Go-to-definition does NOT work in Agent layout file tabs."
    echo "rust-analyzer can be healthy while F12 does nothing there."
    echo "Fix: open this repo in the classic editor instead:"
    echo "  just edit-classic"
    echo "Or double-click a file tab to pop it into the main editor."
    echo
    echo "If classic editor still fails:"
    echo "  1. Cmd Palette -> Developer: Restart Extension Host"
    echo "  2. Cmd Palette -> Rust Analyzer: Restart server"
    echo "  3. Cmd Palette -> Developer: Reload Window"

# Kill bundled rust-analyzer zombies; reload Cursor window after
ra-restart:
    #!/usr/bin/env bash
    set -euo pipefail
    pkill -f       '/home/zrk/.cursor/extensions/rust-lang.rust-analyzer-.*/server/rust-analyzer'       2>/dev/null || true
    echo "Bundled rust-analyzer stopped (if any)."
    echo "In Cursor: Rust Analyzer: Restart server, then Reload Window."

# ─── Whetstone Commands ──────────────────────────────────────────

# Run whetstone setup
setup *ARGS:
    cargo run -- setup {{ ARGS }}

# Run whetstone uninstall
uninstall:
    cargo run -- uninstall

# Show whetstone version info
version:
    cargo run -- version

# ─── Testing ─────────────────────────────────────────────────────

# Run all tests
test:
    cargo test

# Run a single test by name
test-one NAME:
    cargo test {{ NAME }} -- --nocapture

# Run tests with stdout visible
test-verbose:
    cargo test -- --nocapture

# Generate HTML coverage report via tarpaulin
test-coverage:
    cargo tarpaulin --out Html --output-directory coverage
    @echo "Report: coverage/tarpaulin-report.html"

# Generate coverage and fail if below threshold
test-coverage-check THRESHOLD="80":
    cargo tarpaulin --fail-under {{ THRESHOLD }}

# ─── Linting & Formatting ───────────────────────────────────────

# Format all source files
fmt:
    cargo fmt --all

# Check formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

# Run clippy with warnings-as-errors
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run clippy and auto-fix what it can
clippy-fix:
    cargo clippy --all-targets --all-features --fix --allow-dirty -- -D warnings

# Lint everything (format check + clippy)
lint: fmt-check clippy

# Fix everything (format + clippy auto-fix)
fix: fmt clippy-fix

# ─── Security & Dependencies ────────────────────────────────────

# Audit dependencies for known vulnerabilities
audit:
    cargo audit

# Show outdated dependencies
outdated:
    cargo outdated

# Show the full dependency tree
deps:
    cargo tree

# Show duplicate dependencies
deps-dupes:
    cargo tree -d

# Update all dependencies to latest compatible versions
deps-update:
    cargo update

# ─── Pre-commit & CI ────────────────────────────────────────────

# Quick pre-commit checks (format + clippy + check)
pre-commit: fmt clippy check

# Full local CI pipeline (lint + all tests)
ci-local: lint test

# Set up git hooks (run once after clone)
init:
    git config core.hooksPath .githooks

# ─── Release ─────────────────────────────────────────────────────

# Release quality gate (fmt + clippy + test)
release-check:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test

# Preview what a release would do without changing anything
release-dry-run LEVEL:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! "{{ LEVEL }}" =~ ^(patch|minor|major)$ ]]; then
        echo "Usage: just release-dry-run patch|minor|major"; exit 1
    fi
    CURRENT=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
    echo "Current version: $CURRENT"
    echo "Bump level: {{ LEVEL }}"
    just release-check
    echo ""
    echo "All checks passed. Run: just release {{ LEVEL }}"

# Bump version, create release branch + PR (requires: cargo-set-version, gh)
release LEVEL: release-check
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! "{{ LEVEL }}" =~ ^(patch|minor|major)$ ]]; then
        echo "Usage: just release patch|minor|major"; exit 1
    fi
    if [[ -n "$(git status --porcelain)" ]]; then
        echo "Error: dirty working tree"; exit 1
    fi
    BRANCH=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$BRANCH" != "main" ]]; then
        echo "Error: must be on main (currently on $BRANCH)"; exit 1
    fi
    git pull --ff-only origin main
    OLD_VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
    cargo set-version --bump {{ LEVEL }}
    cargo check --quiet
    VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
    echo "$VERSION" > VERSION
    # Regenerate the marketing site's version constant from VERSION.
    # site/src/version.js is the single source of truth that
    # Releases.jsx and InstallTerminal.jsx read at runtime.
    printf '// Single source of truth — regenerated by `just release`.\nwindow.WHETSTONE_VERSION = "%s";\n' \
      "$VERSION" > site/src/version.js
    # Promote the CHANGELOG's [Unreleased] section to [VERSION] - YYYY-MM-DD
    # and reinsert an empty [Unreleased] header above it.
    TODAY=$(date -u +%Y-%m-%d)
    sed -i "s|^## \[Unreleased\]|## [Unreleased]\n\n## [${VERSION}] - ${TODAY}|" CHANGELOG.md
    # Regenerate site/src/changelog.js from the now-promoted CHANGELOG.md
    # so the marketing site's Releases section always matches what shipped.
    cargo run --quiet -- changelog-sync
    # Regenerate site/src/metadata.js — all dynamic site values that JSX
    # components read at runtime (release date, SHA, tagline, asset counts).
    SHA=$(git rev-parse --short HEAD)
    HUMAN_DATE=$(date -u +"%b %d %Y" | tr -s ' ' | tr '[:lower:]' '[:upper:]')
    TAGLINE=$(sed -n '/^## \[/,/^## \[/{/^- /{ s/^- //; s/\*\*//g; p; q; }}' CHANGELOG.md | sed 's/"/\\"/g')
    : "${TAGLINE:=whetstone v${VERSION}}"
    CMD_COUNT=$(find assets/commands -type f 2>/dev/null | wc -l | tr -d ' ')
    HEADROOM_MIN=$(grep -m1 'HEADROOM_MIN' src/headroom.rs | grep -oP '"[^"]+"' | tr -d '"')
    RTK_MIN=$(grep -m1 'RTK_MIN' src/rtk.rs | grep -oP '"[^"]+"' | tr -d '"')
    MEMORY_CMD=$(grep -m1 'icm init' src/setup.rs | grep -oP -- '--mode \S+' || echo '--mode standard')
    printf '// Dynamic site metadata — regenerated by `just release`.\nwindow.WHETSTONE_META = {\n  releaseDate: "%s",\n  releaseDateHuman: "%s",\n  sha: "%s",\n  tagline: "%s",\n  assets: { commands: %s },\n  modules: [\n    { id: "headroom", label: "HEADROOM PROXY", sub: "Context compression" },\n    { id: "headroom-mcp", label: "HEADROOM MCP", sub: "In-context compress tool" },\n    { id: "rtk-hooks", label: "RTK HOOKS", sub: "Pre/Before tool-call rewrite" },\n    { id: "rtk-scope", label: "RTK SCOPE", sub: "Global vs per-project install" },\n    { id: "icm-memory", label: "ICM MEMORY", sub: "Persistent project memory via icm init" },\n    { id: "icm-hooks", label: "ICM HOOKS", sub: "SessionStart / Stop / recall / store" },\n    { id: "provider", label: "PROVIDER", sub: "ICM via icm init" },\n  ],\n  install: {\n    assetLine: "%s commands → .claude/commands/",\n    hooksLine: "rtk init + icm init → ~/.claude/settings.json",\n  },\n};\n' \
      "$TODAY" "$HUMAN_DATE" "$SHA" "$TAGLINE" "${CMD_COUNT:-0}" "${CMD_COUNT:-0}" \
      > site/src/metadata.js
    git checkout -b "release/v${VERSION}"
    git add Cargo.toml Cargo.lock VERSION site/ CHANGELOG.md
    git commit -m "release: v${VERSION}"
    git push -u origin "release/v${VERSION}"
    gh pr create \
        --title "release: v${VERSION}" \
        --body "Bump to v${VERSION} ({{ LEVEL }} release)" \
        --base main

    echo "Waiting for CI checks to appear..."
    for i in $(seq 1 30); do
        if gh pr checks --json name 2>/dev/null | grep -q name; then break; fi
        sleep 2
    done
    echo "Watching CI checks..."
    gh pr checks --watch --fail-fast

    echo "CI passed. Merging..."
    gh pr merge --squash --delete-branch

    git checkout main
    git pull --ff-only origin main

    echo "Watching release workflow..."
    gh run watch

    echo ""
    echo "Release v${VERSION} complete."

# ─── Cleanup ─────────────────────────────────────────────────────

# Remove build artifacts
clean:
    cargo clean

# Remove build artifacts and coverage reports
clean-all: clean
    rm -rf coverage dist

# ─── Project Info ────────────────────────────────────────────────

# Show project and toolchain versions
info:
    @echo "Whetstone v{{ version }}"
    @echo ""
    @echo "Toolchain"
    @echo "  rustc:  $(rustc --version)"
    @echo "  cargo:  $(cargo --version)"
    @echo "  just:   $(just --version)"
    @echo ""
    @echo "Dev Tools"
    @echo "  cargo-set-version: $(cargo set-version --version 2>/dev/null || echo 'not installed')"
    @echo "  cargo-audit:       $(cargo audit --version 2>/dev/null || echo 'not installed')"
    @echo "  cargo-outdated:    $(cargo outdated --version 2>/dev/null || echo 'not installed')"
    @echo "  cargo-watch:       $(cargo watch --version 2>/dev/null || echo 'not installed')"
    @echo "  cargo-tarpaulin:   $(cargo tarpaulin --version 2>/dev/null || echo 'not installed')"

# Show lines of code
loc:
    @echo "Source:"
    @find src -name '*.rs' | xargs wc -l | tail -1
    @echo "Assets:"
    @find assets -type f | wc -l | xargs -I{} echo "  {} files"

# Install all development tools
install-tools:
    cargo install cargo-set-version cargo-audit cargo-outdated cargo-watch
    cargo install cargo-tarpaulin --version 0.32.8

# Show disk usage of build artifacts and caches
cache-status:
    @echo "Disk Usage"
    @echo "  target/:           $(du -sh target 2>/dev/null | cut -f1 || echo 'n/a')"
    @echo "  coverage/:         $(du -sh coverage 2>/dev/null | cut -f1 || echo 'n/a')"
    @echo "  ~/.cargo/registry: $(du -sh ~/.cargo/registry 2>/dev/null | cut -f1 || echo 'n/a')"
