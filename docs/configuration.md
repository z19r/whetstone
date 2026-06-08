# Configuration Reference

## Global Files

| File | Owner | Purpose |
|------|-------|---------|
| `~/.claude/settings.json` | RTK + whetstone | All hooks, including whetstone's absolute `.../rtk hook claude` command |
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
| `OPENAI_BASE_URL` | (none) | For OpenAI-compatible tools through Headroom. Set to `http://127.0.0.1:8787/v1` |
| `HEADROOM_LOG_LEVEL` | `INFO` | Proxy logging verbosity (`DEBUG`, `INFO`, `WARNING`, `ERROR`) |
| `HEADROOM_PORT` | `8787` | Alternative to `--port` flag |
| `HEADROOM_BUDGET` | (none) | Daily USD spending limit |
| `HEADROOM_DEFAULT_MODE` | `optimize` | `optimize`, `audit` (observe only), or `off` |
| `WHETSTONE_ASSETS` | (none) | Override path to assets directory |
