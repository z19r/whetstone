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
| `just run <args>` | Run whetstone with arguments |
| `just build` | Build debug binary |
| `just build-release` | Build optimized release binary |
| `just test` | Run all tests |
| `just test-one <name>` | Run a single test by name |
| `just lint` | Format check + clippy |
| `just fix` | Auto-format + clippy fix |
| `just check` | Check compilation without binaries |
| `just release-check` | Release quality gate (fmt + clippy + test) |
| `just release <level>` | Bump version, create release branch + PR |
| `just release-dry-run <level>` | Preview release without changes |
| `just info` | Show project and toolchain versions |
| `just loc` | Show lines of code |
| `just clean` | Remove build artifacts |
| `just deps` | Show dependency tree |
| `just audit` | Audit dependencies for vulnerabilities |
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
whetstone release patch|minor|major|set X.Y.Z
whetstone release-publish patch|minor|major|set X.Y.Z # Deprecated
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
├── release.rs       # Release preflight, version bump, and PR creation
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

<!-- rtk-instructions v2 -->
# RTK (Rust Token Killer) - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `rtk`**. If RTK has a dedicated filter, it uses it. If not, it passes through unchanged. This means RTK is always safe to use.

**Important**: Even in command chains with `&&`, use `rtk`:
```bash
# ❌ Wrong
git add . && git commit -m "msg" && git push

# ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## RTK Commands by Workflow

### Build & Compile (80-90% savings)
```bash
rtk cargo build         # Cargo build output
rtk cargo check         # Cargo check output
rtk cargo clippy        # Clippy warnings grouped by file (80%)
rtk tsc                 # TypeScript errors grouped by file/code (83%)
rtk lint                # ESLint/Biome violations grouped (84%)
rtk prettier --check    # Files needing format only (70%)
rtk next build          # Next.js build with route metrics (87%)
```

### Test (60-99% savings)
```bash
rtk cargo test          # Cargo test failures only (90%)
rtk go test             # Go test failures only (90%)
rtk jest                # Jest failures only (99.5%)
rtk vitest              # Vitest failures only (99.5%)
rtk playwright test     # Playwright failures only (94%)
rtk pytest              # Python test failures only (90%)
rtk rake test           # Ruby test failures only (90%)
rtk rspec               # RSpec test failures only (60%)
rtk test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
rtk git status          # Compact status
rtk git log             # Compact log (works with all git flags)
rtk git diff            # Compact diff (80%)
rtk git show            # Compact show (80%)
rtk git add             # Ultra-compact confirmations (59%)
rtk git commit          # Ultra-compact confirmations (59%)
rtk git push            # Ultra-compact confirmations
rtk git pull            # Ultra-compact confirmations
rtk git branch          # Compact branch list
rtk git fetch           # Compact fetch
rtk git stash           # Compact stash
rtk git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
rtk gh pr view <num>    # Compact PR view (87%)
rtk gh pr checks        # Compact PR checks (79%)
rtk gh run list         # Compact workflow runs (82%)
rtk gh issue list       # Compact issue list (80%)
rtk gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
rtk pnpm list           # Compact dependency tree (70%)
rtk pnpm outdated       # Compact outdated packages (80%)
rtk pnpm install        # Compact install output (90%)
rtk npm run <script>    # Compact npm script output
rtk npx <cmd>           # Compact npx command output
rtk prisma              # Prisma without ASCII art (88%)
```

### Files & Search (60-75% savings)
```bash
rtk ls <path>           # Tree format, compact (65%)
rtk read <file>         # Code reading with filtering (60%)
rtk grep <pattern>      # Search grouped by file (75%). Format flags (-c, -l, -L, -o, -Z) run raw.
rtk find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
rtk err <cmd>           # Filter errors only from any command
rtk log <file>          # Deduplicated logs with counts
rtk json <file>         # JSON structure without values
rtk deps                # Dependency overview
rtk env                 # Environment variables compact
rtk summary <cmd>       # Smart summary of command output
rtk diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
rtk docker ps           # Compact container list
rtk docker images       # Compact image list
rtk docker logs <c>     # Deduplicated logs
rtk kubectl get         # Compact resource list
rtk kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
rtk curl <url>          # Compact HTTP responses (70%)
rtk wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
rtk gain                # View token savings statistics
rtk gain --history      # View command history with savings
rtk discover            # Analyze Claude Code sessions for missed RTK usage
rtk proxy <cmd>         # Run command without filtering (for debugging)
rtk init                # Add RTK instructions to CLAUDE.md
rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 90-99% |
| Build | next, tsc, lint, prettier | 70-87% |
| Git | status, log, diff, add, commit | 59-80% |
| GitHub | gh pr, gh run, gh issue | 26-87% |
| Package Managers | pnpm, npm, npx | 70-90% |
| Files | ls, read, grep, find | 60-75% |
| Infrastructure | docker, kubectl | 85% |
| Network | curl, wget | 65-70% |

Overall average: **60-90% token reduction** on common development operations.
<!-- /rtk-instructions -->

<!-- icm:start -->
## Persistent memory (ICM) — MANDATORY

This project uses [ICM](https://github.com/rtk-ai/icm) for persistent memory across sessions.
You MUST use it actively. Not optional.

### Recall (before starting work)
```bash
icm recall "query"                        # search memories
icm recall "query" -t "topic-name"        # filter by topic
icm recall-context "query" --limit 5      # formatted for prompt injection
```

### Store — MANDATORY triggers
You MUST call `icm store` when ANY of the following happens:
1. **Error resolved** → `icm store -t errors-resolved -c "description" -i high -k "keyword1,keyword2"`
2. **Architecture/design decision** → `icm store -t decisions-{project} -c "description" -i high`
3. **User preference discovered** → `icm store -t preferences -c "description" -i critical`
4. **Significant task completed** → `icm store -t context-{project} -c "summary of work done" -i high`
5. **Conversation exceeds ~20 tool calls without a store** → store a progress summary

Do this BEFORE responding to the user. Not after. Not later. Immediately.

Do NOT store: trivial details, info already in CLAUDE.md, ephemeral state (build logs, git status).

### Other commands
```bash
icm update <id> -c "updated content"     # edit memory in-place
icm health                                # topic hygiene audit
icm topics                                # list all topics
```
<!-- icm:end -->
