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

Idempotent: global tools install once; memory provider and skills are per-project under `.claude/`.

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

RTK hooks, Headroom, and memory provider (optional) are configured.

## Setting Up a New Project

```bash
mkdir my-new-project && cd my-new-project
git init
whetstone setup

# Verify
whetstone version
rtk --version
headroom --version
ls .claude/skills/               # if you chose a memory provider
```

The setup command will:
1. Verify you're in a git repo (aborts if not)
2. Install Headroom globally via uv (if missing)
3. Install RTK from GitHub into `~/.local/bin` (if missing)
4. Configure the RTK PreToolUse hook in `~/.claude/settings.json`
5. Add `ANTHROPIC_BASE_URL` to your shell profile
6. Copy the whetstone binary to `~/.local/bin`
7. Prompt for memory provider (ICM, AutoMem, or Skip)
8. Copy skills, rules, commands into `.claude/`
9. Install and configure chosen memory provider
10. Merge hooks into `~/.claude/settings.json`
11. Generate `STACK-SETUP.md` in your project root

## Setting Up an Existing Project

Identical process — the setup is idempotent:

```bash
cd ~/existing-project
whetstone setup
```

If components are already installed, setup skips them. If `.claude/` already has skills installed, it skips the copy.

**If your project already has a `.claude/` directory:** Setup creates `.claude/skills/` inside it. Your existing `.claude/settings.json`, `CLAUDE.md`, and other files are preserved. Only `~/.claude/settings.json` (global) is modified (with a timestamped backup).
