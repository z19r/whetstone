# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Whetstone is a Rust CLI that installs and configures three token optimization tools for Claude Code:

- **Headroom** — HTTP proxy between Claude Code and the Anthropic API (50-90% context compression)
- **RTK** — Hook that rewrites CLI commands to compress output before entering context (60-90% savings)
- **Memory** — Persistent memory via ICM (embedded SQLite) or AutoMem (graph memory), with bundled skills and session hooks

Single binary distribution. Users run `whetstone setup` from inside a git project. Global tools (Headroom, RTK) install once; skills, rules, and memory provider are configured per-project.

**Bundled assets** in this repo:
- `assets/skills/` — 20 skill directories (copied to project's `.claude/skills/`)
- `assets/hooks/` — 5 hook `.sh` scripts (copied to `~/.claude/hooks/`)
- `assets/rules/` — 8 rule `.md` files (copied to project's `.claude/rules/`)
- `assets/commands/` — 2 command `.md` files (copied to project's `.claude/commands/`)
- `assets/db/schema.sql` — SQLite schema for session database

## Commands

<!-- AUTO-GENERATED: commands -->
| Command | Description |
|---------|-------------|
| `cargo build` | Build the whetstone binary |
| `cargo test` | Run all tests (11 tests) |
| `cargo clippy` | Run lints |
| `cargo fmt` | Format Rust code |
| `just build` | Build release binary |
| `just test` | Run tests |
| `just release <bump>` | Bump VERSION and optionally tag |
| `just release-publish <bump>` | Bump, commit, tag, and push |
<!-- AUTO-GENERATED: end -->

## CLI Reference

<!-- AUTO-GENERATED: cli -->
```
whetstone                              # Default: headroom wrap claude --model claude-opus-4-6
whetstone setup [--full] [--headroom-extras EXTRAS]
whetstone uninstall
whetstone claude [args...]
whetstone code [args...]               # Alias for claude
whetstone proxy [args...]
whetstone rtk [args...]
whetstone version
whetstone update [--full]
whetstone release patch|minor|major|set X.Y.Z [--tag]
whetstone release-publish patch|minor|major|set X.Y.Z [--tag]
whetstone db init|add-session|add-insight|search|get-sessions|...
```
<!-- AUTO-GENERATED: end -->

`--headroom-extras` accepts: `all` (default = `proxy,code,mcp`), `none`, or comma-separated like `proxy,code`.

## Architecture

```
User → Claude Code
         ├── Bash calls → [RTK Hook] → rtk <cmd> → compressed output
         ├── Context    → [Headroom Proxy :8787] → Anthropic API
         └── Memory     → [ICM or AutoMem] → persistent context
```

**Setup flow** (`whetstone setup`, orchestrated by `src/setup.rs`):
1. Preflight: verify Python 3.10+, git, curl, uv; confirm inside git repo
2. Install Headroom via `uv tool install "headroom-ai[EXTRAS]"` (extras configurable)
3. Install RTK from GitHub (detects name collision with Rust Type Kit)
4. Configure RTK hook globally + set `ANTHROPIC_BASE_URL` in shell profile
5. Self-install binary to `~/.local/bin/whetstone`
6. Prompt for memory provider (ICM, AutoMem, or Skip)
7. Copy skills, rules, commands, MEMSTACK.md; create config.local.json
8. Install and configure chosen memory provider
9. Copy hook scripts to `~/.claude/hooks/`; merge into `~/.claude/settings.json` (backed up with timestamp)
10. Generate `STACK-SETUP.md`

**Hook system** — registered in `~/.claude/settings.json`:

| Event | What Fires | Source |
|-------|-----------|--------|
| PreToolUse (Bash) | RTK rewrites command | RTK |
| PreToolUse (Write/Edit/Bash) | TTS notification | whetstone |
| PreToolUse (Bash, git push) | Build check + secrets scan | whetstone |
| PostToolUse (git commit) | Debug artifact scan | whetstone |
| SessionStart | Headroom auto-start + indexing | whetstone |
| Stop | Session reporting | whetstone |

## Source Layout

<!-- AUTO-GENERATED: source-layout -->
```
src/
├── main.rs          # Entry: parse CLI, dispatch subcommands
├── cli.rs           # clap derive structs for all subcommands
├── setup.rs         # whetstone setup orchestrator (8 steps)
├── uninstall.rs     # Interactive component removal
├── wrapper.rs       # claude/proxy/rtk exec wrappers
├── update.rs        # 12h-cached remote version check
├── release.rs       # Version bump, tag, publish
├── db.rs            # SQLite ops for session/memory database
├── memory.rs        # MemoryProvider enum (ICM, AutoMem, Skip)
├── hooks.rs         # Hook script copy + settings.json merge
├── config.rs        # Typed structs for config.local.json
├── shell.rs         # Shell profile detection, env var injection
├── preflight.rs     # Dependency checks (python, git, curl, uv)
├── headroom.rs      # Headroom install/upgrade (extras configurable)
├── rtk.rs           # RTK install/upgrade + collision detection
├── version.rs       # Semver parse, compare, bump
└── ui.rs            # Colored output, interactive prompts
```
<!-- AUTO-GENERATED: end -->

## Key Design Decisions

- **Single Rust binary**: replaces ~1200 lines Bash + ~460 lines Python
- **Idempotent**: setup skips already-installed components; safe to rerun
- **Absolute paths in hooks**: avoids PATH/shell-state issues
- **Global tools, per-project config**: RTK/Headroom installed globally; memory provider and config are per-project
- **Backup before modify**: `settings.json` backed up with timestamp before any merge
- **No jq dependency**: serde_json replaces jq for settings.json manipulation
- **rusqlite bundled**: statically links SQLite, no system dependency
- **Asset resolution**: `WHETSTONE_ASSETS` env → `<binary_dir>/../assets/` → `~/.whetstone/assets/`

## Rust Conventions

- `anyhow::Result` for error propagation with context
- `ui::fail()` for fatal errors (calls `process::exit(1)`)
- Unix `CommandExt::exec` for wrapper commands (replaces process)
- Non-interactive fallback: `dialoguer::Confirm` with TTY detection
- `console::style` for colored output

<!-- headroom:learn:start -->
## Headroom Learned Patterns
*Auto-generated by `headroom learn` on 2026-03-31 — do not edit manually*

### Repository Layout — Bundled Assets
*~4,000 tokens/session saved*
- `assets/skills/` contains ONLY skill files (flat, no subdirectories from external repos)
- `assets/hooks/`, `assets/rules/`, `assets/commands/` contain runtime files
- These directories are **static/vendored** — do NOT clone or pull external repos into them at install time; files are shipped with whetstone and should only change on a new whetstone release

### Install Constraints
*~3,000 tokens/session saved*
- `src/setup.rs` copies skills flat into `.claude/skills/` via `copy_dir_recursive` (no nested repo structure)
- Never use `git clone` or `git submodule` for skills during install; copy bundled files only
- Verify with `cargo clippy` and `cargo test` after any edits

### Available Commands
*~500 tokens/session saved*
- Use `cargo build && cargo test && cargo clippy` to verify changes
- `just` is the task runner (see `justfile` in repo root)

<!-- headroom:learn:end -->
