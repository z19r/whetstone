# Configuration Reference

## Global Files

| File | Owner | Purpose |
|------|-------|---------|
| `~/.claude/settings.json` | RTK + whetstone | All hooks, including whetstone's absolute `.../rtk hook claude` command |
| `~/.whetstone/settings.json` | whetstone | Global whetstone settings (Headroom telemetry/memory, default model, Anthropic API URL) |
| `~/.claude/RTK.md` | RTK | RTK instructions for Claude Code context |
| `~/.claude/CLAUDE.md` | Claude Code | Global instructions (references `@RTK.md`) |
| `~/.headroom/models.json` | Headroom | Custom model context limits and pricing |
| `~/.local/share/rtk/history.db` | RTK | Token savings tracking database |

## Per-Project Files

| File | Owner | Purpose |
|------|-------|---------|
| `.claude/whetstone.json` | whetstone | Manifest — provider, pinned tool versions, integration version |
| `.claude/commands/` | whetstone | Slash commands (`/whetstone-status`, `/whetstone-headroom`) |
| `.claude/skills/` | ICM | Skills written by `icm init --mode standard` |
| `.claude/icm.db` | ICM | Session / memory store |
| `STACK-SETUP.md` | whetstone setup | Per-project quick reference |
| `CLAUDE.md` | Claude Code | Project-specific instructions |

> v3 does **not** bundle skills or rules. The provider (ICM) owns its own
> assets; whetstone only writes the v3 slash commands and the manifest.
> Migrating from v2? See the [Migration Guide](migration.md).

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `ANTHROPIC_BASE_URL` | (none) | Route API calls through Headroom proxy. Set to `http://127.0.0.1:8787` |
| `ANTHROPIC_TARGET_API_URL` | (none) | Custom upstream Anthropic API URL the Headroom proxy targets. Whetstone exports it from the **Anthropic API URL** setting before launch; an externally-set value takes precedence |
| `OPENAI_BASE_URL` | (none) | For OpenAI-compatible tools through Headroom. Set to `http://127.0.0.1:8787/v1` |
| `HEADROOM_LOG_LEVEL` | `INFO` | Proxy logging verbosity (`DEBUG`, `INFO`, `WARNING`, `ERROR`) |
| `HEADROOM_PORT` | `8787` | Alternative to `--port` flag |
| `HEADROOM_BUDGET` | (none) | Daily USD spending limit |
| `HEADROOM_DEFAULT_MODE` | `optimize` | `optimize`, `audit` (observe only), or `off` |
| `WHETSTONE_ASSETS` | (none) | Override path to assets directory |

## Whetstone Settings

`whetstone settings` opens an interactive TUI to edit layered settings. Each
setting can be scoped **Off**, **Global**, or **Project**, with project values
taking precedence over global ones.

| Setting | Effect |
|---------|--------|
| Headroom Telemetry | Toggle Headroom telemetry |
| Headroom Memory | Persist the `--memory` flag (Headroom cross-session memory) |
| API Model | Default model injected into `headroom wrap claude` (otherwise: newest Sonnet, else `claude-opus-4-6`) |
| Anthropic API URL | Custom upstream Anthropic API URL for the Headroom proxy (exported as `ANTHROPIC_TARGET_API_URL`) |

Values persist to:

- **Global** — `~/.whetstone/settings.json`
- **Project** — the `settings` block of `.claude/whetstone.json`
