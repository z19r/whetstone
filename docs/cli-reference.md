# CLI Reference

| Command | Description |
|---------|-------------|
| `whetstone` | Start Claude Code through Headroom (waits for proxy, then `headroom wrap claude`) |
| `whetstone setup [--full] [--headroom-extras EXTRAS]` | Install/configure all components (auto-detects v2 and hands off to `migrate`) |
| `whetstone project uninstall` | Interactive removal of whetstone files from the current project |
| `whetstone global uninstall` | Interactive removal of global components (binaries, RTK, Headroom) |
| `whetstone claude [args...]` | Run Claude Code through Headroom |
| `whetstone code [args...]` | Alias for `claude` |
| `whetstone proxy [args...]` | Run `headroom proxy` |
| `whetstone rtk [args...]` | Run RTK |
| `whetstone doctor` | Inspect installed tool versions, `~/.claude/settings.json` hooks, and the per-project manifest |
| `whetstone dashboard` | TUI dashboard for installed tool versions vs. pinned floors |
| `whetstone settings` | Interactively edit whetstone settings, layered global/project (Headroom telemetry, Headroom memory, default API model, edit/permission mode, Anthropic API URL) |
| `whetstone migrate [--dry-run] [-y] [--rollback ID]` | Migrate a v2 install to v3 (or roll back) — see [Migration Guide](migration.md) |
| `whetstone version` | Print version |
| `whetstone stats` | Token-savings summary from RTK + Headroom stats endpoints |
| `whetstone update [--full]` | Check for newer release; `--full` force-refreshes Headroom/RTK and bundled assets |
| `whetstone release patch\|minor\|major\|set X.Y.Z` | Verify, bump version, and open a release PR |
| `whetstone release-publish ...` | **Deprecated** — use `whetstone release` |
| `whetstone db <subcommand>` | Session database operations (init / add-session / add-insight / search / get-sessions / get-insights / stats) |
| `whetstone memory consolidate [--dry-run]` | Drain a stray project-local `.headroom` store into the global `~/.headroom` root (fixes cross-project memory litter) |
| `whetstone changelog-sync [--input F] [--output F] [--limit N]` | Regenerate `site/src/changelog.js` from `CHANGELOG.md` (maintainer tooling) |

The global `--memory` flag (e.g. `whetstone --memory` or `whetstone --memory claude`)
enables Headroom persistent cross-session memory for that run. Persist it per-project
or globally via `whetstone settings` (Headroom Memory).

Because whetstone runs a single shared Headroom proxy, it pins the memory store
root to a global location (`~/.headroom/memory.db` via `HEADROOM_MEMORY_DB_PATH`)
so per-project memory DBs land under `~/.headroom/memories/projects/` instead of
littering whichever project happened to launch the proxy. On launch, whetstone
also auto-consolidates any stray project-local `.headroom` store into that global
root (conservative: it never overwrites existing global data and leaves a legacy
seed DB in place rather than mixing stores). Run `whetstone memory consolidate
--dry-run` to preview.

Add a `headroom_env` map to `.claude/whetstone.json` (project) or
`~/.whetstone/settings.json` (global) to override any Headroom launch knob
(savings profile, target ratio, memory behavior, etc.). These maps are
hand-edited JSON — the `whetstone settings` TUI does not yet expose them.
Precedence: external `HEADROOM_*` env vars > project > global > defaults;
`HEADROOM_PORT` is reserved.

## Model Update Prompt

On launch (`whetstone` / `whetstone claude`), whetstone checks the 12h-cached
Anthropic models list. If a model newer than the one this project runs — or a
brand-new model family — is available, it shows a full-screen modal offering to:

- **Pin** it as the project default (writes `api_model` to `.claude/whetstone.json`)
- **Use for this session** only (no persistence)
- **Dismiss** it permanently for this project (recorded in `dismissed_models`)
- **Not now** (offered again next launch)

The prompt is skipped when the terminal is non-interactive, whetstone is offline,
the directory is not a v3 project, or `--model` was passed explicitly.

## Headroom Extras

`--headroom-extras` controls which Headroom optional packages are installed:

| Value | Installs |
|-------|----------|
| `all` (default) | `headroom-ai[proxy,code,mcp]` |
| `none` | `headroom-ai` (base only) |
| `proxy,code` | `headroom-ai[proxy,code]` (custom) |

## Versioning & Updates

Whetstone uses a single `VERSION` file as the source of truth.

```bash
whetstone version                  # Current version
whetstone update                   # Check for newer release
whetstone update --full            # Force-upgrade Headroom/RTK
```

For contributors:

```bash
just release-check                # Release verification gate
just release patch                 # Verify, bump version, and open release PR
```

## Headroom Proxy Flags

```
headroom proxy [OPTIONS]

--host HOST          Network interface (default: 127.0.0.1)
--port PORT          Listen port (default: 8787)
--budget AMOUNT      Daily USD spending limit
--log-file PATH      JSONL request log
--no-optimize        Passthrough mode (no compression)
--no-cache           Disable response caching
--llmlingua          Enable ML-based compression (~2GB download)
--llmlingua-device   auto|cuda|cpu|mps
--llmlingua-rate     Compression ratio, 0.0-1.0 (default: 0.3 = keep 30%)
--backend            bedrock|vertex_ai|azure|openrouter (default: anthropic)
--region             Cloud region (for bedrock/vertex_ai)
```

## RTK Quick Reference

> **Heads up — RTK is not always a net win.** The PreToolUse hook only fires on
> **Bash** tool calls; Claude Code's native `Read`, `Grep`, `Glob`, and file-edit
> tools bypass it. Compression can also strip context the model needed — RTK's own
> tracker has logged a ~18% net cost-increase case. Run `rtk gain` for cumulative
> savings, `rtk discover` for missed opportunities, and consider RTK's audit mode
> when a particular rewrite feels suspect.

```bash
# Analytics
rtk gain                  # Token savings summary
rtk gain --graph          # ASCII chart (30 days)
rtk gain --history        # Per-command log
rtk gain --daily          # Day-by-day breakdown
rtk discover              # Find missed opportunities
rtk session               # Adoption rate across sessions

# File operations
rtk ls .                  # Compact directory tree
rtk read file.rs          # Smart file reading
rtk grep "pattern" .      # Grouped search results
rtk find "*.rs" .         # Compact find

# Git (all transparent via hook)
rtk git status            # Compact status
rtk git log -n 10         # One-line commits
rtk git diff              # Condensed diff

# Test runners (failures only)
rtk test cargo test       # Rust
rtk pytest                # Python
rtk vitest run            # Vitest
rtk go test               # Go

# Build/lint (errors only)
rtk cargo build           # Cargo
rtk tsc                   # TypeScript
rtk lint                  # ESLint
```
