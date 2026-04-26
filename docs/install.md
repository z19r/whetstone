# Installation

## One-Liner

Run from your git project root:

```bash
cd ~/my-project
curl -fsSL https://raw.githubusercontent.com/z19r/whetstone/main/install.sh | bash
```

This downloads the prebuilt binary for your OS/arch, installs it to
`~/.local/bin/whetstone`, fetches assets, and runs `whetstone setup`.

## From Source

```bash
cargo install whetstone
cd ~/my-project
whetstone setup
```

## Another Project Later

```bash
cd ~/another-project
whetstone setup
```

Idempotent: global tools install once; MemStack is per-project under `.claude/`.

## Prerequisites

| Requirement | Minimum | Check |
|-------------|---------|-------|
| Python | 3.10+ | `python3 --version` |
| Git | any | `git --version` |
| curl | any | `curl --version` |
| uv | any | `uv --version` |

## Quick Start

```bash
# 1. Install whetstone
curl -fsSL https://raw.githubusercontent.com/z19r/whetstone/main/install.sh | bash

# 2. Or, if already installed, go to your project (must be a git repo)
cd ~/my-project
whetstone setup

# 3. Start Claude Code through Headroom
whetstone
```

RTK hooks, Headroom, and MemStack (optional) are configured.

## Setting Up a New Project

```bash
mkdir my-new-project && cd my-new-project
git init
whetstone setup

# Verify
whetstone version
rtk --version
headroom --version
ls .claude/skills/MEMSTACK.md    # if you opted into MemStack
```

The setup command will:
1. Verify you're in a git repo (aborts if not)
2. Install Headroom globally via uv (if missing)
3. Install RTK from GitHub into `~/.local/bin` (if missing)
4. Configure the RTK PreToolUse hook in `~/.claude/settings.json`
5. Add `ANTHROPIC_BASE_URL` to your shell profile
6. Copy the whetstone binary to `~/.local/bin`
7. Optionally install MemStack into `.claude/`
8. Initialize the MemStack SQLite database (if MemStack on)
9. Merge hooks into `~/.claude/settings.json` (if MemStack on)
10. Generate `STACK-SETUP.md` in your project root (if MemStack on)

## Setting Up an Existing Project

Identical process — the setup is idempotent:

```bash
cd ~/existing-project
whetstone setup
```

If components are already installed, setup skips them. If `.claude/` already exists with MemStack, it skips the copy.

**If your project already has a `.claude/` directory:** Setup creates `.claude/skills/` inside it. Your existing `.claude/settings.json`, `CLAUDE.md`, and other files are preserved. Only `~/.claude/settings.json` (global) is modified (with a timestamped backup).
