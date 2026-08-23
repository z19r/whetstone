# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Whetstone is a Rust CLI that installs and configures three token optimization tools for Claude Code:

- **Headroom** — HTTP proxy between Claude Code and the Anthropic API (50-90% context compression)
- **RTK** — Hook that rewrites CLI commands to compress output before entering context (60-90% savings)
- **Memory** — Persistent project memory via ICM (embedded SQLite). ICM owns its own skills, hooks, and CLI, installed by `icm init --mode standard`.

Single binary distribution. Users run `whetstone setup` from inside a git project. Global tools (Headroom, RTK) install once; the memory provider and version-pinned manifest are configured per-project.

**Bundled assets** in this repo (v3 ships only slash commands and the DB schema — skills, rules, and hook scripts are no longer vendored):
- `assets/commands/` — slash command `.md` files (copied to project's `.claude/commands/`)
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
whetstone                              # Default: headroom wrap claude (auto-selects newest available Sonnet; falls back to claude-opus-4-6)
whetstone setup [--full] [--headroom-extras EXTRAS]
whetstone project uninstall            # Remove whetstone files from this project
whetstone global uninstall             # Remove global binaries, RTK, Headroom
whetstone claude [args...]
whetstone code [args...]               # Alias for claude
whetstone proxy [args...]
whetstone rtk [args...]
whetstone version
whetstone dashboard                    # TUI: installed tool versions vs pinned floors
whetstone settings                     # Interactive layered settings (global/project)
whetstone doctor [--fix]               # Check dependencies + ~/.claude/settings.json, report drift
whetstone install-tools [--force]      # Install/repair headroom, rtk, claude code, memory provider
whetstone migrate [--dry-run] [-y] [--rollback ID]   # v2 → v3 migration (reversible)
whetstone stats                        # Token savings across RTK + Headroom
whetstone update [--full]
whetstone release patch|minor|major|set X.Y.Z
whetstone release-publish patch|minor|major|set X.Y.Z # Deprecated
whetstone changelog-sync [--input F] [--output F] [--limit N]  # regen site/src/changelog.js
whetstone db init|add-session|add-insight|search|get-sessions|...
whetstone memory consolidate [--dry-run]   # drain stray project-local .headroom stores into ~/.headroom
```
<!-- AUTO-GENERATED: end -->

`--headroom-extras` accepts: `all` (default = `proxy,code,mcp`), `none`, or comma-separated like `proxy,code`.

`--memory` is a global flag (e.g. `whetstone --memory`, `whetstone --memory claude`). It enables Headroom persistent cross-session memory by passing `--memory` to the proxy whetstone spawns. Memory on/off is part of the per-config fingerprint (see Architecture below), so a session that wants memory never fights one that doesn't — each gets its own proxy, and there is no restart/replace prompt to resolve the conflict. It can also be set persistently per-project or globally via `whetstone settings` (Headroom Memory).

`whetstone doctor` checks the managed dependencies (headroom, rtk, claude
code, and the project's memory provider) before it looks at hooks: a tool that
is missing or no longer runnable is offered for reinstall in interactive runs,
reported with a pointer to `whetstone install-tools` otherwise, and reinstalled
without prompting under `--fix`. A repaired tool's own `init` is re-run so its
hooks come back before the settings checks look for them.
`whetstone install-tools` drives the same repair path directly, also installing
`uv` if it went missing, restoring the `~/.local/bin/whetstone` symlink, and
resyncing `tool_versions` in the project manifest.

Doctor also runs a **startup check**: if a proxy is already answering on
`127.0.0.1:8787` that counts as proof, otherwise whetstone spawns a throwaway
`headroom proxy` on a free port (same args/env as a real launch, killed after a
6s grace window) and reports headroom's own error if it exits. This catches the
case every other check misses — headroom installed, current, and complete, but
dead on arrival because its own `~/.headroom/settings.json` asks for a flag the
current rollout channel rejects. When the error names such a flag, doctor maps
it back to the settings key and offers to remove it (prompt when interactive,
automatic under `--fix`, backing the file up first), then re-runs the check.
Note the fast path's blind spot: a proxy still running from before a settings
change reports OK even though the next start would fail.

Headroom is checked by *extras* as well as version: whetstone reads uv's
receipt (`$UV_TOOL_DIR`/`~/.local/share/uv/tools/headroom-ai/uv-receipt.toml`)
and treats an install missing any requested extra as needing repair, so a bare
`headroom-ai` can't pass as healthy while `headroom proxy`/`headroom mcp` are
absent. Both commands default to `--headroom-extras all`
(`headroom-ai[proxy,code,mcp]`); pass the same value you set up with if you
deliberately installed a smaller set. An install uv didn't record (e.g. pip)
reports unknown extras and is left alone.

`whetstone settings` also exposes **Anthropic API URL** — a custom upstream Anthropic API URL for the Headroom proxy. When set (per-project or globally), whetstone exports it as `ANTHROPIC_TARGET_API_URL` before launching Headroom, so the whetstone-spawned proxy targets that upstream (mirrors Headroom's `proxy --anthropic-api-url` flag). An externally-set `ANTHROPIC_TARGET_API_URL` env var takes precedence over the stored setting.

`whetstone settings` also exposes **Edit Mode** — the Claude Code `--permission-mode` (`acceptEdits`, `default`, `plan`, `bypassPermissions`) whetstone injects into `headroom wrap claude`. Stored per-project (`permission_mode` in `.claude/whetstone.json`) or globally, project-over-global. When unset (**Off**), whetstone injects no flag and Claude Code uses its own default; an explicit `--permission-mode` on the command line always wins.

`whetstone settings` also exposes **Headroom Savings Profile** — the Headroom compression profile (`coding`, `agent-90`, `balanced`, `general`) whetstone exports as `HEADROOM_SAVINGS_PROFILE` before launching Headroom, so the spawned proxy and the exec'd `headroom wrap claude` read the same profile (keeping them in agreement and part of the per-config proxy fingerprint). Stored per-project (`savings_profile` in `.claude/whetstone.json`) or globally, project-over-global. When unset (**Off**/None), whetstone exports nothing and Headroom falls back to its own default (`agent-90`); an externally-set `HEADROOM_SAVINGS_PROFILE` env var takes precedence over the stored setting.

On launch, whetstone (`src/model_update.rs`) checks the 12h-cached Anthropic models list for a model newer than the one this project runs — or a brand-new model family — and shows a full-screen modal offering to pin it as the project default (`api_model`), use it for one session, or dismiss it permanently (recorded in the manifest's top-level `dismissed_models`). The prompt is skipped when non-interactive, offline, not a v3 project, or when `--model` was passed explicitly.

whetstone also supports `headroom_env` — a map of any Headroom launch knobs in
`.claude/whetstone.json` (project) or `~/.whetstone/settings.json` (global),
with precedence: external `HEADROOM_*` env vars > project map > global map >
whetstone defaults (`HEADROOM_PORT` reserved, wins always).

## Architecture

```
User → Claude Code
         ├── Bash calls → [RTK Hook] → rtk <cmd> → compressed output
         ├── Context    → [Headroom Proxy :8787] → Anthropic API
         └── Memory     → [ICM, embedded SQLite] → persistent context
```

Whetstone runs one Headroom proxy per distinct resolved config (savings profile, telemetry, memory, upstream URL, and the `HEADROOM_*` apply set — see the "Global Headroom memory root" bullet below), reusing a live proxy only when a session's config fingerprint matches exactly. There is no memory-conflict prompt: memory on/off is part of that fingerprint, so a session that wants memory and one that doesn't simply get separate proxies instead of racing to replace each other's.

**Setup flow** (`whetstone setup`, orchestrated by `src/setup.rs`). Setup first self-updates and offers v2→v3 migration; if the terminal is interactive it runs `src/wizard.rs`, otherwise the headless sequence below:
1. Resolve assets; preflight-check dependencies (python, git, curl, uv)
2. Install Headroom via `uv tool install "headroom-ai[EXTRAS]"` (extras configurable)
3. Install RTK from GitHub (detects name collision with Rust Type Kit)
4. Shell profile: set `ANTHROPIC_BASE_URL` + ensure `~/.local/bin` on PATH
5. Self-install binary to `~/.local/bin/whetstone`
6. Prompt for memory provider (ICM or Skip)
7. If a provider was chosen, `complete_setup`: copy slash commands, install the provider binary, run tool integrations (`src/integrations.rs`), run `doctor`, write the `.claude/whetstone.json` manifest, generate `STACK-SETUP.md`

**Hook system** — v3 no longer hand-writes `~/.claude/settings.json`. Whetstone delegates to each tool's own installer (`src/integrations.rs`), which writes its own hook entries; `whetstone doctor` reports drift.

| What Fires | Installed by |
|-----------|--------------|
| PreToolUse (Bash) — RTK rewrites the command | `rtk init --auto-patch` |
| ICM slash commands, CLAUDE.md block, session hooks | `icm init --mode standard` |

## Source Layout

<!-- AUTO-GENERATED: source-layout -->
```
src/
├── main.rs          # Entry: parse CLI, dispatch subcommands
├── cli.rs           # clap derive structs for all subcommands
├── setup.rs         # whetstone setup orchestrator (headless path)
├── wizard.rs        # Interactive setup wizard
├── uninstall.rs     # Interactive component removal
├── wrapper.rs       # claude/proxy/rtk exec wrappers; proxy + model resolution
├── claude_code.rs   # Claude Code launch/detection helpers
├── integrations.rs  # Delegates to `rtk init` / `icm init` (tool-managed hooks)
├── migrate.rs       # v2 → v3 migration + reversible rollback
├── doctor.rs        # Check dependencies + settings.json, repair/report drift
├── dashboard.rs     # TUI: installed tool versions vs pinned floors
├── settings.rs      # Interactive layered global/project settings TUI
├── stats.rs         # Token-savings summary (RTK + Headroom)
├── tools.rs         # Managed-dependency inventory: presence checks, repair, install-tools
├── update.rs        # 12h-cached remote version check + self-update
├── release.rs       # Release preflight, version bump, and PR creation
├── changelog.rs     # CHANGELOG.md parsing / site changelog sync
├── db.rs            # SQLite ops for session/memory database
├── memory.rs        # MemoryProvider enum (ICM, Skip)
├── memory_consolidate.rs # Drain stray project-local .headroom stores into ~/.headroom
├── config.rs        # ProjectSettings/GlobalSettings + .claude/whetstone.json manifest
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
- **Global tools, per-project config**: RTK/Headroom installed globally; memory provider and version-pinned manifest are per-project
- **Per-config proxy registry**: whetstone runs one proxy per distinct resolved config, keyed by a fingerprint (savings profile, telemetry, memory, upstream URL, and the non-memory-path `HEADROOM_*` apply set) over entries in `~/.whetstone/proxies.json` (`src/proxy_registry.rs`). Two sessions that would spawn a byte-identical proxy share it; anything that differs — including memory on vs. off — gets its own. Port `8787` is only a best-effort launch-order anchor so a bare `claude` launch and doctor's fast path still find a proxy there when nothing else has claimed it; it is not a fixed single-proxy port. The memory DB path itself stays out of the fingerprint (see below), so it can't fracture reuse on its own.
- **Global Headroom memory root**: whetstone pins `HEADROOM_MEMORY_DB_PATH` to `~/.headroom/memory.db` so per-project memory DBs live under `~/.headroom/memories/projects/` instead of accumulating in whichever project launched the proxy — unaffected by per-config proxy reuse above. `wrap_claude` auto-consolidates any stray project-local `.headroom` store into that root (never overwriting global data; seed DBs migrate all-or-nothing), and `whetstone memory consolidate [--dry-run]` runs it explicitly (`src/memory_consolidate.rs`)
- **Tool-managed hooks**: v3 delegates `~/.claude/settings.json` hook entries to `rtk init` / `icm init`; whetstone never hand-merges them (`doctor` reports drift, `migrate` archives before touching state)
- **Layered settings**: `GlobalSettings` (`~/.whetstone/settings.json`) and per-project `ProjectSettings` (in `.claude/whetstone.json`) resolve with project-over-global precedence
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
- v3 vendors **only** `assets/commands/` (slash commands) and `assets/db/schema.sql`. Skills, rules, and hook scripts are no longer bundled — ICM owns those via `icm init --mode standard`
- These directories are **static/vendored** — do NOT clone or pull external repos into them at install time; files are shipped with whetstone and change only on a new whetstone release

### Install Constraints
*~3,000 tokens/session saved*
- `src/setup.rs` copies slash commands into `.claude/commands/` via `copy_dir_recursive`; provider assets come from the provider's own `init`
- Never use `git clone` or `git submodule` during install; copy bundled files only
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
rtk uv run <cmd>        # Compact uv project command output
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
