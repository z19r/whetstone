# Whetstone

Token optimization and project memory for AI coding assistants (Claude Code
first). A single Rust binary installs and configures everything; project hooks
and skills land in whatever git repo you run it from.

```
whetstone setup ──┬── Headroom (context compression proxy)
                  ├── RTK (CLI output compression)
                  └── Memory (ICM or AutoMem provider)
```

## Install

```bash
cd ~/my-project
curl -fsSL https://raw.githubusercontent.com/z19r/whetstone/main/install.sh | bash
```

Or from source: `cargo install whetstone && whetstone setup`

See [docs/install.md](docs/install.md) for prerequisites, setup details, and project configuration.

## Architecture

```
User → AI Coding Tool
         ├── Bash calls → [RTK Hook] → compressed output (60-90% savings)
         ├── Context    → [Headroom Proxy :8787] → LLM API (50-90% savings)
         └── Memory     → [ICM or AutoMem] → persistent context
```

Token savings compound: RTK shrinks tool output before it enters the context window, then Headroom compresses the entire context before it hits the API.

## What Each Tool Does

**Headroom** — Context compression proxy between your AI tool and the LLM provider. Multi-stage pipeline: cache alignment, content routing, statistical JSON compression, AST-aware code compression, score-based message dropping. Benchmarks: 97% accuracy at 19% tokens (SQuAD v2).

**RTK** — Single Rust binary that compresses CLI output. `cargo test` goes from ~4,800 tokens to ~11. `git diff` from ~21,500 to ~1,259. Transparent via hook — `git status` becomes `rtk git status` automatically.

**Memory** — Choose between ICM (embedded SQLite, zero dependencies) or AutoMem (graph memory via FalkorDB + Qdrant). Bundled skills, rules, and hooks for pre-push safety, post-commit cleanup, and session lifecycle.

## Documentation

| Doc | Contents |
|-----|----------|
| [Installation](docs/install.md) | Prerequisites, quick start, new/existing project setup |
| [CLI Reference](docs/cli-reference.md) | All commands, flags, RTK quick reference |
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
