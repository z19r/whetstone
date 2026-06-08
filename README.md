# Whetstone

A single Rust binary that installs and orchestrates three upstream tools for
Claude Code (and friends): **Headroom** (context proxy), **RTK** (Bash-output
hook), and **ICM** (project memory). Idempotent setup, version-pinned manifest,
reversible v2 → v3 migration, release automation.

```
whetstone setup ──┬── Headroom (context compression proxy, upstream)
                  ├── RTK (Bash-output rewrite hook, upstream)
                  └── ICM (embedded SQLite project memory, upstream)
```

Compression is upstream's job. The glue — installing, versioning, configuring,
migrating, and tearing down cleanly — is whetstone's.

## Install

```bash
cd ~/my-project
curl -fsSL https://raw.githubusercontent.com/z19r/whetstone/main/install.sh | bash
```

Or from source: `cargo install whetstone && whetstone setup`

See [docs/install.md](docs/install.md) for prerequisites, setup details, and project configuration.

## Upgrading from v2 → v3

v3 is a structural rewrite. **`whetstone setup` will refuse to install over a v2
project** and hand off to `whetstone migrate`. The migration is one command,
archive-backed, and reversible.

```bash
whetstone migrate                 # interactive
whetstone migrate --dry-run       # preview the plan, write nothing
whetstone migrate -y              # non-interactive (CI)
whetstone migrate --rollback <id> # restore a prior v2 state byte-for-byte
```

Full procedure, archive contents, and rollback semantics: [docs/migration.md](docs/migration.md).

## Breaking changes (v3.0.0)

- **AutoMem provider removed.** `MemoryProvider` is now `{ Icm, Skip }`. The `mcpServers.memory` block is torn out of `~/.claude/settings.json` (archived). If you ran an external FalkorDB + Qdrant service, tear it down yourself — it's no longer in whetstone's blast radius.
- **Bundled skills, rules, and hook scripts removed.** The `assets/hooks/*.sh`, `assets/skills/`, `assets/rules/`, and the `MEMSTACK.md` shim are gone. ICM owns its own assets; whetstone delegates to `icm init --mode standard`.
- **Hooks are tool-managed.** Whetstone no longer hand-merges `~/.claude/settings.json`. `rtk init --auto-patch` and `icm init` write their own hook entries. `whetstone doctor` reports drift.
- **Migration is mandatory.** Existing v2 installs must run `whetstone migrate` before any v3 command will configure them.
- **`config.local.json` replaced by `.claude/whetstone.json`** (schema version, integration version, provider, tool versions, timestamps).
- **Hardcoded `--model` injection removed.** `whetstone claude` no longer forces a model; Claude Code's own settings choose it.

Full release notes in [CHANGELOG.md](CHANGELOG.md).

## Architecture

```
User → AI Coding Tool
         ├── Bash calls → [RTK Hook]            → compressed output
         ├── Context    → [Headroom Proxy :8787]→ LLM API
         └── Memory     → [ICM, embedded SQLite]→ persistent context
```

## What each piece does

**Whetstone (this repo)** — single Rust binary. Installs the three tools below,
ensures `uv` is present (offers to install it), version-pins everything in
`.claude/whetstone.json`, exposes `setup`, `update`, `migrate`, `doctor`, and
`release`. No runtime dependencies of its own.

**Headroom** *(upstream — [headroom-ai](https://pypi.org/project/headroom-ai/))* —
HTTP proxy in front of the LLM provider. Multi-stage pipeline: cache alignment,
content routing, statistical JSON compression, AST-aware code compression,
score-based message dropping. Compression numbers belong to Headroom; run
`curl localhost:8787/stats` to measure your own.

**RTK** *(upstream — [rtk-ai/rtk](https://github.com/rtk-ai/rtk))* — Bash-output
compression via PreToolUse hook. Compresses output before it enters the context
window. Caveat: it only fires on **Bash** tool calls; Claude Code's native
Read/Grep/Glob bypass it. Compression isn't free either — RTK's own tracker has
logged a ~18% net cost-increase case. Run `rtk gain` and `rtk discover` to keep
yourself honest, and consider RTK's audit mode if a particular rewrite feels
suspect.

**ICM** *(upstream — [rtk-ai/icm](https://github.com/rtk-ai/icm))* — embedded
SQLite memory store; skills, hooks, and CLI installed by
`icm init --mode standard`. Whetstone v2's bundled MemStack/skills/rules layer
is gone — ICM owns its own assets now.

## Documentation

| Doc | Contents |
|-----|----------|
| [Installation](docs/install.md) | Prerequisites, quick start, new/existing project setup |
| [CLI Reference](docs/cli-reference.md) | All v3 commands, flags, RTK quick reference + caveats |
| [Migration Guide](docs/migration.md) | v2 → v3 with `migrate`, `--dry-run`, `--rollback` |
| [Editor Setup](docs/editors.md) | Claude Code, Cursor, Copilot, Windsurf, Cline, Aider, Codex, Gemini CLI, OpenCode + compatibility matrix |
| [Headroom Service](docs/headroom-service.md) | systemd, launchd, and background setup |
| [Configuration](docs/configuration.md) | Global/per-project files, environment variables |
| [Troubleshooting](docs/troubleshooting.md) | Common issues, uninstall, manual removal |

## Development

```bash
just build          # Debug build
just test           # Run all tests
just lint           # Clippy lints
just fmt            # Format code
just check          # Build + test + lint
```

Source layout in [CLAUDE.md](CLAUDE.md).
