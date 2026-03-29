# AI Tooling Stack

Token optimization and project management for AI coding assistants.

Three tools, one setup script. Run `setup-stack.sh` from any git project to install and configure everything.

```
setup-stack.sh ─── installs & configures ──┬── Headroom  (context compression proxy)
                                           ├── RTK       (CLI output compression)
                                           └── MemStack  (skills + persistent memory)
```

---

## Install

**One-liner** (clone + run in your project):

```bash
git clone https://github.com/z19r/ai-tooling.git ~/.ai-tooling
cd ~/my-project
bash ~/.ai-tooling/setup-stack.sh
```

**Or just grab the script:**

```bash
curl -fsSL https://raw.githubusercontent.com/z19r/ai-tooling/main/setup-stack.sh -o /tmp/setup-stack.sh
cd ~/my-project
bash /tmp/setup-stack.sh
```

**Add to more projects later:**

```bash
cd ~/another-project
bash ~/.ai-tooling/setup-stack.sh
```

The script is idempotent — global tools (Headroom, RTK, hooks) are installed once, MemStack is cloned per-project.

---

## Table of Contents

- [Install](#install)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Setting Up a New Project](#setting-up-a-new-project)
- [Setting Up an Existing Project](#setting-up-an-existing-project)
- [Editor & Tool Configuration](#editor--tool-configuration)
  - [Claude Code (CLI)](#claude-code-cli)
  - [Claude Code (VS Code Extension)](#claude-code-vs-code-extension)
  - [Claude Code (JetBrains Extension)](#claude-code-jetbrains-extension)
  - [Cursor](#cursor)
  - [VS Code + GitHub Copilot](#vs-code--github-copilot)
  - [Windsurf](#windsurf)
  - [Cline / Roo Code (VS Code)](#cline--roo-code-vs-code)
  - [Aider](#aider)
  - [OpenAI Codex CLI](#openai-codex-cli)
  - [Gemini CLI](#gemini-cli)
  - [OpenCode](#opencode)
- [Running Headroom as a Service](#running-headroom-as-a-service)
- [Multi-Project Setup](#multi-project-setup)
- [What Each Tool Does](#what-each-tool-does)
- [Compatibility Matrix](#compatibility-matrix)
- [Configuration Reference](#configuration-reference)
- [Troubleshooting](#troubleshooting)
- [Uninstall](#uninstall)

---

## Architecture

```
                          ┌─────────────────────────────────────────────────┐
                          │              YOUR AI CODING TOOL                │
                          │  (Claude Code, Cursor, Copilot, Aider, etc.)   │
                          └──────────┬──────────────────┬──────────────────┘
                                     │                  │
                     ┌───────────────┤                  │
                     ▼               │                  ▼
              ┌─────────────┐        │         ┌──────────────────┐
              │  RTK Hook   │        │         │  MemStack Skills │
              │             │        │         │                  │
              │ Rewrites    │        │         │ 77 skills        │
              │ bash cmds   │        │         │ SQLite memory    │
              │ to compress │        │         │ Session tracking │
              │ CLI output  │        │         │ Safety hooks     │
              │             │        │         │                  │
              │ 60-90%      │        │         │ Persistent       │
              │ fewer tokens│        │         │ across sessions  │
              │ from tools  │        │         │                  │
              └─────────────┘        │         └──────────────────┘
                                     │
                                     ▼
                          ┌─────────────────────┐
                          │   Headroom Proxy     │
                          │   localhost:8787     │
                          │                     │
                          │ Compresses context  │
                          │ before it hits the  │
                          │ LLM API             │
                          │                     │
                          │ 50-90% fewer tokens │
                          │ sent to provider    │
                          └──────────┬──────────┘
                                     │
                                     ▼
                          ┌─────────────────────┐
                          │   LLM Provider API   │
                          │   (Anthropic, OpenAI,│
                          │    Bedrock, Vertex,  │
                          │    Azure, etc.)      │
                          └─────────────────────┘
```

**Token savings compound:** RTK shrinks tool output before it enters the context window, then Headroom compresses the entire context before it hits the API. A `cargo test` that produces 4,800 tokens becomes ~11 tokens via RTK, and any remaining context bloat gets compressed another 50-90% by Headroom.

---

## Prerequisites

| Requirement | Minimum | Check |
|-------------|---------|-------|
| Python | 3.10+ | `python3 --version` |
| Git | any | `git --version` |
| jq | any | `jq --version` |
| curl | any | `curl --version` |
| pip | any | `pip --version` |

The setup script will attempt to install `jq` via your package manager if missing.

---

## Quick Start

If you just want everything working in 60 seconds:

```bash
# 1. Clone this repo (or just grab the script)
git clone <this-repo> ~/ai-tooling

# 2. Go to your project
cd ~/my-project

# 3. Run setup
bash ~/ai-tooling/setup-stack.sh

# 4. Start coding with full optimization
headroom wrap claude
```

That's it. RTK hooks, Headroom proxy, and MemStack are all configured.

---

## Setting Up a New Project

```bash
# Create and init your project
mkdir my-new-project && cd my-new-project
git init

# Run the stack setup
bash /path/to/setup-stack.sh

# Verify
rtk --version                    # RTK installed
headroom --version               # Headroom installed
ls .claude/skills/MEMSTACK.md    # MemStack installed
```

The script will:
1. Verify you're in a git repo (aborts if not)
2. Install Headroom globally via pip (if missing)
3. Install RTK globally via install script (if missing)
4. Configure the RTK PreToolUse hook in `~/.claude/settings.json`
5. Add `ANTHROPIC_BASE_URL` to your shell profile
6. Clone MemStack into `.claude/skills/`
7. Initialize the MemStack SQLite database
8. Install semantic search dependencies (lancedb, sentence-transformers)
9. Merge all hooks into `~/.claude/settings.json`
10. Generate `STACK-SETUP.md` in your project root

---

## Setting Up an Existing Project

Identical process — the script is idempotent:

```bash
cd ~/existing-project
bash /path/to/setup-stack.sh
```

If components are already installed, the script skips them. If `.claude/skills/` already exists with MemStack, it skips the clone.

**If your project already has a `.claude/` directory:** The script creates `.claude/skills/` inside it. Your existing `.claude/settings.json`, `CLAUDE.md`, and other files are preserved. Only `~/.claude/settings.json` (global) is modified (with a timestamped backup).

---

## Editor & Tool Configuration

### Claude Code (CLI)

**Full stack support.** This is the primary target — all three tools work natively.

```bash
# Option A: One command (recommended)
headroom wrap claude

# Option B: Manual
headroom proxy --port 8787 &
claude

# Option C: Without Headroom (RTK + MemStack only)
claude
```

**What happens automatically:**
- RTK hook rewrites every Bash tool call (`git status` -> `rtk git status`)
- Headroom compresses context before API calls (if proxy is running)
- MemStack hooks fire on session start/end, commits, pushes
- MemStack skills activate on keyword triggers ("recall", "todo", "verify", etc.)

**Hook configuration** lives in `~/.claude/settings.json` (global). All hooks — RTK and MemStack — are installed to `~/.claude/hooks/` with absolute paths. Works in every project, every directory.

**MCP tools** (optional, adds `headroom_compress`, `headroom_retrieve`, `headroom_stats`):
```bash
headroom mcp install
headroom mcp status
```

---

### Claude Code (VS Code Extension)

**Full stack support.** The VS Code extension uses the same CLI and configuration.

**Setup:**
1. Run `setup-stack.sh` from your project root (terminal)
2. The extension reads the same `~/.claude/settings.json` hooks
3. RTK and MemStack hooks work identically to CLI

**Headroom proxy connection** — two options:

Option A — Set in VS Code settings (`settings.json`):
```json
{
  "claude-code.environmentVariables": [
    {
      "name": "ANTHROPIC_BASE_URL",
      "value": "http://localhost:8787"
    }
  ]
}
```

Option B — The setup script already added `ANTHROPIC_BASE_URL` to your shell profile, which VS Code's integrated terminal inherits.

**Start the proxy** before opening VS Code:
```bash
headroom proxy --port 8787 &
```
Or use the [systemd service](#running-headroom-as-a-service) to have it always running.

---

### Claude Code (JetBrains Extension)

**Full stack support.** Same as VS Code — the JetBrains extension uses the same CLI backend.

1. Run `setup-stack.sh` from your project terminal
2. Hooks are shared via `~/.claude/settings.json`
3. Start Headroom proxy before your session, or use the systemd service

**Environment variable:** Set `ANTHROPIC_BASE_URL=http://localhost:8787` in your JetBrains run configuration or shell profile.

---

### Cursor

**Partial support.** RTK and Headroom work. MemStack does not (Cursor has a different hook system).

**RTK setup:**
```bash
rtk init -g --agent cursor
```
This creates `~/.cursor/hooks/rtk-rewrite.sh` and patches Cursor's `hooks.json`.

**Headroom setup:**
1. Start the proxy: `headroom proxy --port 8787 &`
2. In Cursor: Settings > Models > Override OpenAI Base URL
3. Set to: `http://localhost:8787/v1`
4. Enter your API key

Alternatively, set the environment variable before launching:
```bash
ANTHROPIC_BASE_URL=http://localhost:8787 cursor
```

**MemStack:** Not supported. Cursor does not implement Claude Code's `PreToolUse`/`PostToolUse`/`SessionStart`/`Stop` lifecycle hooks. The skills and memory system will not activate.

---

### VS Code + GitHub Copilot

**Partial support.** RTK works. Headroom requires SDK integration. MemStack does not work.

**RTK setup:**
```bash
rtk init -g --copilot
```
This creates `.github/hooks/rtk-rewrite.json` and `.github/copilot-instructions.md`. Copilot Chat in VS Code gets transparent command rewriting. Copilot CLI uses deny-with-suggestion (CLI limitation — it cannot silently rewrite).

**Headroom:** No native base URL override in Copilot. For programmatic usage, use the SDK:
```typescript
import { withHeadroom } from 'headroom-ai/openai';
const client = withHeadroom(new OpenAI());
```

**MemStack:** Not supported (different hook architecture).

---

### Windsurf

**Partial support.** RTK works (project-scoped). Headroom via env var. MemStack does not work.

**RTK setup:**
```bash
cd your-project
rtk init --agent windsurf
```
This creates `.windsurfrules` in your project root. Windsurf's Cascade reads this file and prefixes commands with `rtk`. Note: project-scoped only (no global `-g` flag).

**Headroom:** Set the environment variable before launching:
```bash
ANTHROPIC_BASE_URL=http://localhost:8787 windsurf
```

**MemStack:** Not supported.

---

### Cline / Roo Code (VS Code)

**Partial support.** RTK works (project-scoped). Headroom via settings. MemStack does not work.

**RTK setup:**
```bash
cd your-project
rtk init --agent cline
```
This creates `.clinerules` in your project root. Project-scoped only.

**Headroom:** Cline has API base URL fields in its settings panel. Set the Anthropic base URL to `http://localhost:8787`.

**MemStack:** Not supported.

---

### Aider

**Partial support.** Headroom works natively. RTK is instruction-based. MemStack does not work.

**Headroom (one command):**
```bash
headroom wrap aider
```
This starts the proxy and launches Aider with the correct base URL.

**RTK:** No hook system in Aider. You can add instructions to `.aider.conf.yml` or use `rtk` commands directly in your prompts.

**MemStack:** Not supported.

---

### OpenAI Codex CLI

**Partial support.** RTK works (instruction-based). Headroom works. MemStack does not work.

**Headroom:**
```bash
headroom wrap codex
```

**RTK setup:**
```bash
rtk init -g --codex
```
This creates `~/.codex/RTK.md` and `~/.codex/AGENTS.md`. Codex reads these as global instructions and prefixes commands with `rtk`. This is instruction-based (no hook API), so compliance depends on the model.

**MemStack:** Not supported.

---

### Gemini CLI

**Partial support.** RTK works (hook-based). Headroom via env var. MemStack does not work.

**RTK setup:**
```bash
rtk init -g --gemini
```
This creates `~/.gemini/hooks/rtk-hook-gemini.sh` and patches `~/.gemini/settings.json` with a `BeforeTool` hook.

**Headroom:**
```bash
OPENAI_BASE_URL=http://localhost:8787/v1 gemini
```

**MemStack:** Not supported.

---

### OpenCode

**Partial support.** RTK works (plugin-based). Headroom via env var. MemStack does not work.

**RTK setup:**
```bash
rtk init -g --opencode
```
This creates `~/.config/opencode/plugins/rtk.ts` using the `tool.execute.before` plugin hook.

---

## Running Headroom as a Service

Instead of starting the proxy manually each session, run it as a background service.

### systemd (Linux)

Create `~/.config/systemd/user/headroom.service`:

```ini
[Unit]
Description=Headroom Context Compression Proxy
After=network.target

[Service]
Type=simple
ExecStart=%h/.local/bin/headroom proxy --port 8787
Restart=on-failure
RestartSec=5
Environment=HEADROOM_LOG_LEVEL=INFO

[Install]
WantedBy=default.target
```

Enable and start:
```bash
systemctl --user daemon-reload
systemctl --user enable --now headroom
systemctl --user status headroom

# Check it's running
curl -s localhost:8787/health | jq
```

### launchd (macOS)

Create `~/Library/LaunchAgents/com.headroom.proxy.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.headroom.proxy</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/headroom</string>
        <string>proxy</string>
        <string>--port</string>
        <string>8787</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/headroom.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/headroom.err</string>
</dict>
</plist>
```

Load:
```bash
launchctl load ~/Library/LaunchAgents/com.headroom.proxy.plist
```

### Quick background (any platform)

```bash
nohup headroom proxy --port 8787 > /tmp/headroom.log 2>&1 &
```

### Production (multi-worker)

```bash
pip install gunicorn uvicorn
gunicorn headroom.proxy:app \
  --workers 4 \
  --worker-class uvicorn.workers.UvicornWorker \
  --bind 0.0.0.0:8787
```

---

## Multi-Project Setup

You don't need to clone MemStack into every project. Use symlinks:

### Shared MemStack (recommended for multiple projects)

```bash
# Clone once to a central location
git clone https://github.com/cwinvestments/memstack ~/memstack

# Initialize
cd ~/memstack
cp config.json config.local.json
# Edit config.local.json with your project paths
python db/memstack-db.py init

# Symlink into each project
ln -s ~/memstack/.claude ~/project-a/.claude
ln -s ~/memstack/.claude ~/project-b/.claude
ln -s ~/memstack/.claude ~/project-c/.claude
```

Updates to `~/memstack` propagate instantly to all linked projects.

### Windows (junctions)

```cmd
mklink /J C:\Projects\my-project\.claude C:\Projects\memstack\.claude
```

### Remove a link (preserves source)

```bash
rm ~/project-a/.claude          # Linux/macOS (removes symlink only)
rmdir C:\Projects\my-project\.claude   # Windows (removes junction only)
```

---

## What Each Tool Does

### Headroom — Context Compression Proxy

Sits between your AI tool and the LLM provider. Compresses the context window through a multi-stage pipeline:

1. **CacheAligner** — Stabilizes system prompt prefix for KV cache hits
2. **ContentRouter** — Auto-detects content type (JSON, code, logs, etc.)
3. **SmartCrusher** — Statistical compression of JSON arrays (70-95% reduction)
4. **CodeCompressor** — AST-aware code compression via tree-sitter
5. **IntelligentContextManager** — Score-based message dropping

Compression is **reversible** via CCR (Compress-Cache-Retrieve): originals are cached locally, and the LLM gets a `headroom_retrieve` tool to fetch full data when needed.

**Benchmarks:** 97% accuracy at 19% tokens (SQuAD v2), 100% lossless at 77% tokens (Needle Retention).

### RTK — CLI Output Compression

A single Rust binary that compresses command output. When Claude Code runs `git status`, the hook rewrites it to `rtk git status`, which produces 5 lines instead of 45.

**Savings examples:**

| Command | Raw Tokens | RTK Tokens | Savings |
|---------|-----------|------------|---------|
| `cargo test` | ~4,800 | ~11 | 99% |
| `git diff` (large) | ~21,500 | ~1,259 | 94% |
| `pytest` | ~756 | ~24 | 96% |
| `git push` | ~200 | ~10 | 95% |
| `ls -la` | ~100 | ~30 | 70% |

**Commands:** `rtk gain` (stats), `rtk gain --graph` (visual), `rtk discover` (missed opportunities), `rtk session` (adoption metrics).

### MemStack — Skills & Memory Framework

77 specialist skills that activate on keyword triggers, plus persistent memory across Claude Code sessions.

**Core skills:** Echo (semantic recall), Diary (session logging), Work (task tracking), State (project state), Verify (pre-commit checks), Governor (scope control), Sight (architecture diagrams).

**Hooks:** Pre-push safety checks (build verification, secrets scanning), post-commit artifact detection, session start/end lifecycle management.

**Database:** SQLite with sessions, insights, project context, and task plans. Optional LanceDB vector search for semantic recall.

---

## Compatibility Matrix

| Feature | Claude Code | Cursor | Copilot | Windsurf | Cline | Aider | Codex | Gemini CLI |
|---------|:-----------:|:------:|:-------:|:--------:|:-----:|:-----:|:-----:|:----------:|
| **Headroom proxy** | `wrap` | Manual URL | SDK only | env var | settings | `wrap` | `wrap` | env var |
| **Headroom MCP** | `mcp install` | manual | -- | -- | -- | -- | -- | -- |
| **RTK hooks** | PreToolUse | preToolUse | PreToolUse | `.windsurfrules` | `.clinerules` | manual | instructions | BeforeTool |
| **RTK scope** | global | global | global | project | project | -- | global | global |
| **MemStack skills** | full | -- | -- | -- | -- | -- | -- | -- |
| **MemStack hooks** | full | -- | -- | -- | -- | -- | -- | -- |
| **MemStack memory** | full | -- | -- | -- | -- | -- | -- | -- |

Legend: full = fully supported, manual = requires manual configuration, -- = not supported

---

## Configuration Reference

### Global Files

| File | Owner | Purpose |
|------|-------|---------|
| `~/.claude/settings.json` | RTK + MemStack | All hooks (absolute paths to `~/.claude/hooks/`) |
| `~/.claude/hooks/rtk-rewrite.sh` | RTK | Bash command rewriter |
| `~/.claude/RTK.md` | RTK | RTK instructions for Claude Code context |
| `~/.claude/CLAUDE.md` | Claude Code | Global instructions (references `@RTK.md`) |
| `~/.headroom/models.json` | Headroom | Custom model context limits and pricing |
| `~/.local/share/rtk/history.db` | RTK | Token savings tracking database |

### Per-Project Files

| File | Owner | Purpose |
|------|-------|---------|
| `.claude/skills/` | MemStack | Skills framework (cloned repo) |
| `.claude/skills/config.local.json` | MemStack | Project-specific configuration |
| `.claude/skills/db/memstack.db` | MemStack | Session/memory database |
| `STACK-SETUP.md` | setup-stack.sh | Per-project quick reference |
| `CLAUDE.md` | Claude Code | Project-specific instructions |

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `ANTHROPIC_BASE_URL` | (none) | Route API calls through Headroom proxy. Set to `http://127.0.0.1:8787` |
| `OPENAI_BASE_URL` | (none) | For OpenAI-compatible tools through Headroom. Set to `http://127.0.0.1:8787/v1` |
| `HEADROOM_LOG_LEVEL` | `INFO` | Proxy logging verbosity (`DEBUG`, `INFO`, `WARNING`, `ERROR`) |
| `HEADROOM_PORT` | `8787` | Alternative to `--port` flag |
| `HEADROOM_BUDGET` | (none) | Daily USD spending limit |
| `HEADROOM_DEFAULT_MODE` | `optimize` | `optimize`, `audit` (observe only), or `off` |
| `OPENAI_API_KEY` | (none) | Optional: higher-quality embeddings for MemStack semantic search |

### Headroom Proxy Flags

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

### RTK Configuration

Optional config at `~/.config/rtk/config.toml`:

```toml
[tracking]
database_path = "~/.local/share/rtk/history.db"

[hooks]
exclude_commands = ["curl", "playwright"]   # Skip rewrite for these

[tee]
enabled = true            # Save raw output on failure
mode = "failures"         # "failures", "always", "never"
max_files = 20            # Rotation limit
```

### RTK Quick Reference

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

---

## Troubleshooting

### "headroom: command not found"

```bash
pip install "headroom-ai[proxy,code,mcp]"
# If installed but not on PATH:
python3 -m headroom proxy --port 8787
```

### "rtk: command not found"

```bash
# Install
curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh
# Add to PATH
export PATH="$HOME/.local/bin:$PATH"
# Persist
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc  # or ~/.bashrc
```

### "rtk gain" shows wrong output (Rust Type Kit conflict)

```bash
which rtk                 # Check which binary you have
# If it's the wrong one:
cargo uninstall rtk       # Remove Rust Type Kit
curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh
```

### RTK hook not rewriting commands

```bash
# Check hook exists
ls -la ~/.claude/hooks/rtk-rewrite.sh

# Check settings.json has the hook
cat ~/.claude/settings.json | jq '.hooks.PreToolUse'

# Test rewrite manually
echo '{"tool_name":"Bash","tool_input":{"command":"git status"}}' | bash ~/.claude/hooks/rtk-rewrite.sh

# Re-initialize
rtk init -g --hook-only --auto-patch
```

### Headroom proxy not compressing

```bash
# Is proxy running?
curl -s localhost:8787/health | jq

# Is env var set?
echo $ANTHROPIC_BASE_URL
# Should be: http://127.0.0.1:8787

# Start manually
headroom proxy --port 8787

# Check stats
curl -s localhost:8787/stats | jq
```

### MemStack skills not loading

```bash
# Is it installed?
ls .claude/skills/MEMSTACK.md

# Are rules present?
ls .claude/skills/.claude/rules/

# Is the database initialized?
python .claude/skills/db/memstack-db.py stats

# Re-initialize DB
python .claude/skills/db/memstack-db.py init
```

### Hooks not firing at all

```bash
# Check global settings
cat ~/.claude/settings.json | jq '.hooks'

# Check hook scripts exist and are accessible
ls -la .claude/skills/.claude/hooks/

# Restore from backup if settings.json is broken
ls ~/.claude/settings.json.bak.*
cp ~/.claude/settings.json.bak.NEWEST ~/.claude/settings.json
```

### Headroom proxy crashes or OOMs

```bash
# Run without ML compression (much lighter)
headroom proxy --port 8787

# Or with CPU-only ML compression
headroom proxy --port 8787 --llmlingua --llmlingua-device cpu

# Or just audit mode (observe, no compression)
HEADROOM_DEFAULT_MODE=audit headroom proxy --port 8787
```

### Semantic search not working in MemStack

```bash
# Check deps
pip show lancedb sentence-transformers

# Re-install
pip install lancedb sentence-transformers

# Index existing sessions
python .claude/skills/skills/echo/index-sessions.py

# Falls back to SQLite keyword search if vector search unavailable
```

---

## Uninstall

### Remove MemStack (per-project)

```bash
rm -rf .claude/skills
rm STACK-SETUP.md
```

### Remove RTK (global)

```bash
rtk init -g --uninstall        # Remove hooks from settings.json
rm ~/.local/bin/rtk            # Remove binary
rm -rf ~/.local/share/rtk      # Remove tracking database
```

### Remove Headroom (global)

```bash
pip uninstall headroom-ai

# Remove systemd service (if created)
systemctl --user disable --now headroom
rm ~/.config/systemd/user/headroom.service
systemctl --user daemon-reload

# Remove env var from shell profile
# Edit ~/.zshrc or ~/.bashrc and delete the ANTHROPIC_BASE_URL line
```

### Restore original settings.json

```bash
# List backups (created by setup-stack.sh)
ls -lt ~/.claude/settings.json.bak.* | head -5

# Restore
cp ~/.claude/settings.json.bak.TIMESTAMP ~/.claude/settings.json
```

### Full cleanup (everything)

```bash
# 1. Remove per-project files
rm -rf .claude/skills STACK-SETUP.md

# 2. Remove RTK
rtk init -g --uninstall 2>/dev/null
rm -f ~/.local/bin/rtk
rm -rf ~/.local/share/rtk

# 3. Remove Headroom
systemctl --user disable --now headroom 2>/dev/null
rm -f ~/.config/systemd/user/headroom.service
pip uninstall -y headroom-ai

# 4. Remove semantic search deps
pip uninstall -y lancedb sentence-transformers

# 5. Clean shell profile (edit manually)
#    Remove: export ANTHROPIC_BASE_URL=http://127.0.0.1:8787

# 6. Restore original hooks
ls ~/.claude/settings.json.bak.* | tail -1 | xargs -I{} cp {} ~/.claude/settings.json
```
