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
| `HEADROOM_*` (any) | (varies) | External Headroom knobs (e.g. `HEADROOM_TARGET_RATIO`). Win over `headroom_env` map; `HEADROOM_PORT` is reserved |

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

The launch-time model-update prompt (see the CLI Reference) also writes the
project **API Model** when you pin a newer model. Models you permanently dismiss
from that prompt are recorded in the top-level `dismissed_models` array of
`.claude/whetstone.json` so they are not offered again for that project.

### Headroom launch environment (`headroom_env`)

whetstone sets a small set of opinionated Headroom defaults when it launches
the proxy:

- `HEADROOM_SAVINGS_PROFILE=agent-90` (compression posture for agents)
- `HEADROOM_CODE_AWARE_ENABLED=1` (AST-aware compression for code)
- `HEADROOM_NO_MEMORY_TOOLS=1` + `HEADROOM_NO_MEMORY_CONTEXT=1` **only when
  the project's memory provider is ICM** (ICM owns memory)

Override any Headroom knob by adding it to a `headroom_env` map in
`.claude/whetstone.json` (project) or `~/.whetstone/settings.json` (global);
project entries win over global. An externally-set `HEADROOM_*` env var wins
over both. `HEADROOM_PORT` is reserved (whetstone pins `8787`) and ignored
if set via the map.

    "headroom_env": { "HEADROOM_TARGET_RATIO": "0.2" }
