# Configuration Reference

## Global Files

| File | Owner | Purpose |
|------|-------|---------|
| `~/.claude/settings.json` | RTK + whetstone | All hooks (absolute paths to `~/.claude/hooks/`) |
| `~/.claude/hooks/rtk-rewrite.sh` | RTK | Bash command rewriter |
| `~/.claude/RTK.md` | RTK | RTK instructions for Claude Code context |
| `~/.claude/CLAUDE.md` | Claude Code | Global instructions (references `@RTK.md`) |
| `~/.headroom/models.json` | Headroom | Custom model context limits and pricing |
| `~/.local/share/rtk/history.db` | RTK | Token savings tracking database |

## Per-Project Files

| File | Owner | Purpose |
|------|-------|---------|
| `.claude/skills/` | whetstone | Skills directories |
| `.claude/rules/` | whetstone | Rule files |
| `.claude/commands/` | whetstone | Command files |
| `config.local.json` | whetstone | Project-specific configuration |
| `.claude/db/memstack.db` | whetstone | Session/memory database |
| `STACK-SETUP.md` | whetstone setup | Per-project quick reference |
| `CLAUDE.md` | Claude Code | Project-specific instructions |

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
