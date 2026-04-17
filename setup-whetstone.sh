#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# Whetstone — Headroom + RTK + MemStack installer for Claude Code
#
# Usage (project root, git repo):
#   bash /path/to/setup-whetstone.sh
#   curl -fsSL https://example.com/install.sh | bash
#
# What it does:
#   1. Installs Headroom, RTK, whetstone + whetstone-rtk → ~/.local/bin
#   2. Configures RTK hooks and Headroom (ANTHROPIC_BASE_URL)
#   3. Optionally MemStack skills + hooks in this repo + ~/.claude
# ============================================================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail()  { echo -e "${RED}[FAIL]${NC} $*"; exit 1; }

extract_semver() {
    local raw="$1"
    local match
    match=$(echo "$raw" | sed -nE 's/.*([0-9]+\.[0-9]+\.[0-9]+).*/\1/p')
    echo "$match"
}

version_lt() {
    local left="$1"
    local right="$2"
    if [[ -z "$left" || -z "$right" ]]; then
        return 1
    fi
    [[ "$(printf '%s\n%s\n' "$left" "$right" | sort -V | head -n1)" \
        != "$right" ]]
}

# Python packages: uv only (preflight ensures uv exists).
py_install() {
    uv tool install "$@"
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Remote installer sets WHETSTONE_ROOT to the cloned repo (for bin/).
REPO_ROOT="${WHETSTONE_ROOT:-$SCRIPT_DIR}"
PROJECT_DIR="$(pwd)"
CLAUDE_DIR="$HOME/.claude"
SETTINGS_JSON="$CLAUDE_DIR/settings.json"
SHELL_PROFILE=""
VERSION_FILE="$REPO_ROOT/VERSION"
WHETSTONE_VERSION="dev"
MIN_HEADROOM_VERSION="0.14.0"
MIN_RTK_VERSION="0.7.0"

if [[ -f "$VERSION_FILE" ]]; then
    WHETSTONE_VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"
fi

# Detect shell profile
if [[ -f "$HOME/.zshrc" ]]; then
    SHELL_PROFILE="$HOME/.zshrc"
elif [[ -f "$HOME/.bashrc" ]]; then
    SHELL_PROFILE="$HOME/.bashrc"
elif [[ -f "$HOME/.profile" ]]; then
    SHELL_PROFILE="$HOME/.profile"
fi

# Official RTK installer (installs binary to ~/.local/bin).
RTK_INSTALL_SH="https://raw.githubusercontent.com/rtk-ai/rtk/\
refs/heads/master/install.sh"

# ============================================================================
# Pre-flight checks
# ============================================================================

preflight() {
    info "Running pre-flight checks..."

    # Must be in a git repo
    if ! git rev-parse --is-inside-work-tree &>/dev/null; then
        fail "$(printf '%s\n%s' \
            'Not inside a git repository. Run from a git project root.' \
            '       To init one: git init')"
    fi
    ok "Inside git repo: $(git rev-parse --show-toplevel)"

    # Python 3.10+
    if ! command -v python3 &>/dev/null; then
        fail "Python 3 not found. Install Python 3.10+."
    fi
    PY_VERSION=$(
        python3 -c \
            'import sys; v=sys.version_info; print(f"{v.major}.{v.minor}")'
    )
    PY_MAJOR=$(echo "$PY_VERSION" | cut -d. -f1)
    PY_MINOR=$(echo "$PY_VERSION" | cut -d. -f2)
    if (( PY_MAJOR < 3 || (PY_MAJOR == 3 && PY_MINOR < 10) )); then
        fail "Python $PY_VERSION found but 3.10+ required."
    fi
    ok "Python $PY_VERSION"

    # git
    command -v git &>/dev/null || fail "git not found."
    ok "git $(git --version | awk '{print $3}')"

    # jq
    if ! command -v jq &>/dev/null; then
        warn "jq not found — installing..."
        if command -v pacman &>/dev/null; then
            sudo pacman -S --noconfirm jq
        elif command -v apt-get &>/dev/null; then
            sudo apt-get install -y jq
        elif command -v brew &>/dev/null; then
            brew install jq
        else
            fail "$(printf '%s %s' \
                'Cannot install jq automatically. Install manually:' \
                'https://jqlang.github.io/jq/download/')"
        fi
    fi
    ok "jq $(jq --version 2>/dev/null || echo 'installed')"

    # curl
    command -v curl &>/dev/null || fail "curl not found."
    ok "curl available"

    # uv (replaces pip for installs in this script)
    if ! command -v uv &>/dev/null; then
        fail "uv not found. Install uv, then re-run this script.\n\
       curl -LsSf https://astral.sh/uv/install.sh | sh\n\
       https://docs.astral.sh/uv/"
    fi
    ok "uv $(uv --version 2>/dev/null | awk '{print $2}')"

    echo ""
}

# ============================================================================
# Step 1: Install Headroom
# ============================================================================

install_headroom() {
    info "Step 1: Headroom (context compression proxy)"

    if command -v headroom &>/dev/null; then
        local raw ver
        raw=$(headroom --version 2>/dev/null || echo "unknown")
        ver=$(extract_semver "$raw")
        if version_lt "$ver" "$MIN_HEADROOM_VERSION"; then
            warn "Headroom $ver is older than $MIN_HEADROOM_VERSION"
            info "Upgrading headroom-ai..."
            py_install --upgrade "headroom-ai[proxy,code,mcp]"
            ok "Headroom upgraded: $(headroom --version 2>/dev/null)"
        else
            ok "Headroom already installed: $raw"
        fi
    else
        info "Installing headroom-ai with proxy, code, and MCP extras..."
        py_install "headroom-ai[proxy,code,mcp]"
        ok "Headroom installed: $(headroom --version 2>/dev/null)"
    fi
    echo ""
}

# ============================================================================
# ~/.local/bin on PATH (RTK and other user binaries)
# ============================================================================

ensure_local_bin_on_path() {
    export PATH="$HOME/.local/bin:$PATH"
    local line='export PATH="$HOME/.local/bin:$PATH"'
    local rc
    for rc in "$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.profile"; do
        [[ -f "$rc" ]] || continue
        if grep -qF '$HOME/.local/bin' "$rc" 2>/dev/null \
            || grep -qF "${HOME}/.local/bin" "$rc" 2>/dev/null; then
            continue
        fi
        echo "$line" >> "$rc"
        info "Added ~/.local/bin to PATH in $rc"
    done
}

# ============================================================================
# Step 2: Install RTK (from GitHub install.sh → ~/.local/bin)
# ============================================================================

install_rtk() {
    info "Step 2: RTK (CLI output compression)"

    local need_install=0
    if command -v rtk &>/dev/null; then
        if rtk gain &>/dev/null; then
            local raw ver
            raw=$(rtk --version 2>/dev/null || echo "unknown")
            ver=$(extract_semver "$raw")
            if version_lt "$ver" "$MIN_RTK_VERSION"; then
                warn "RTK $ver is older than $MIN_RTK_VERSION"
                need_install=1
            else
                ok "RTK already installed: $raw"
            fi
        else
            warn "$(printf '%s %s' \
                "Found 'rtk' but it is not rtk-ai (e.g. Rust Type Kit)." \
                'Replacing with rtk-ai from GitHub...')"
            need_install=1
        fi
    else
        info "Installing RTK from GitHub into ~/.local/bin..."
        need_install=1
    fi

    if [[ "$need_install" -eq 1 ]]; then
        curl -fsSL "$RTK_INSTALL_SH" | sh
        ensure_local_bin_on_path
        ok "RTK installed: $(rtk --version 2>/dev/null)"
    fi

    ensure_local_bin_on_path
    echo ""
}

# ============================================================================
# Step 3: Configure RTK globally for Claude Code
# ============================================================================

configure_rtk() {
    info "Step 3: Configuring RTK for Claude Code"

    mkdir -p "$CLAUDE_DIR/hooks"

    # Use rtk init if available, otherwise set up manually
    if command -v rtk &>/dev/null && rtk gain &>/dev/null; then
        # Check if hook already exists
        if [[ -f "$CLAUDE_DIR/hooks/rtk-rewrite.sh" ]]; then
            ok "RTK hook already exists at $CLAUDE_DIR/hooks/rtk-rewrite.sh"
        else
            info "Running rtk init -g --hook-only..."
            rtk init -g --hook-only --auto-patch 2>/dev/null || {
                warn "rtk init failed, hook may need manual setup"
            }
            ok "RTK global hook configured"
        fi
    else
        warn "RTK binary not functional yet — hook will activate on next"\
            " session"
    fi
    echo ""
}

# ============================================================================
# Step 4: Configure Headroom as API proxy
# ============================================================================

configure_headroom() {
    info "Step 4: Configuring Headroom proxy integration"

    local base_url="http://127.0.0.1:8787"

    if [[ -n "$SHELL_PROFILE" ]]; then
        if grep -q 'ANTHROPIC_BASE_URL' "$SHELL_PROFILE" 2>/dev/null; then
            ok "ANTHROPIC_BASE_URL already set in $SHELL_PROFILE"
        else
            echo "" >> "$SHELL_PROFILE"
            echo "# Headroom context compression proxy for Claude Code" \
                >> "$SHELL_PROFILE"
            echo "export ANTHROPIC_BASE_URL=$base_url" >> "$SHELL_PROFILE"
            ok "Added ANTHROPIC_BASE_URL=$base_url to $SHELL_PROFILE"
        fi
    else
        warn "$(printf '%s %s=%s' \
            'No shell profile found. Set manually: export' \
            'ANTHROPIC_BASE_URL' "$base_url")"
    fi

    # Export for current session
    export ANTHROPIC_BASE_URL="$base_url"
    echo ""
}

# ============================================================================
# Whetstone CLI → ~/.local/bin (Headroom wrap + rtk with proxy URL)
# ============================================================================

install_whetstone_bins() {
    info "Installing whetstone and whetstone-rtk into ~/.local/bin..."
    local whet="$REPO_ROOT/bin/whetstone"
    local wr="$REPO_ROOT/bin/whetstone-rtk"
    if [[ ! -f "$whet" || ! -f "$wr" ]]; then
        warn "bin/whetstone missing next to script — skipping CLI install"
        return 0
    fi
    mkdir -p "$HOME/.local/bin"
    install -m 0755 "$whet" "$HOME/.local/bin/whetstone"
    install -m 0755 "$wr" "$HOME/.local/bin/whetstone-rtk"
    ensure_local_bin_on_path
    ok "whetstone, whetstone-rtk → ~/.local/bin"
    echo ""
}

# ============================================================================
# Prompt: MemStack skills + Claude hooks for this project
# ============================================================================

prompt_memstack_install() {
    INSTALL_MEMSTACK=1
    echo ""
    info "MemStack adds skills under .claude/skills/ and registers Claude"
    info "Code hooks (session, git helpers, etc.) under ~/.claude/."
    if [[ -t 0 ]]; then
        read -r -p \
            "Install MemStack skills and hooks for this project? [Y/n] " \
            reply
        reply="${reply,,}"
        if [[ "$reply" == "n" || "$reply" == "no" ]]; then
            INSTALL_MEMSTACK=0
        fi
    else
        info "Non-interactive: installing MemStack (skills + hooks)."
    fi
    echo ""
}

# ============================================================================
# Step 5: Install MemStack for the current project
# ============================================================================

install_memstack() {
    info "Step 5: Installing MemStack into project"

    local claude_dir="$PROJECT_DIR/.claude"
    local skills_dir="$claude_dir/skills"
    local bundled_skills="$REPO_ROOT/.claude/skills"
    local bundled_memstack="$REPO_ROOT/.claude/memstack"

    # Copy skills (the 28 skill directories)
    if [[ -d "$skills_dir" ]] && ls "$skills_dir"/*/ >/dev/null 2>&1; then
        ok "Skills already installed at $skills_dir"
    else
        if [[ -d "$bundled_skills" ]]; then
            info "Copying skills into .claude/skills/..."
            mkdir -p "$skills_dir"
            cp -a "$bundled_skills"/* "$skills_dir/"
            ok "Skills copied"
        else
            fail "Bundled skills not found at $bundled_skills — is your Whetstone clone intact?"
        fi
    fi

    # Copy MemStack supporting files (rules, commands, db)
    for subdir in rules commands db; do
        if [[ -d "$bundled_memstack/$subdir" ]] && [[ ! -d "$claude_dir/$subdir" ]]; then
            cp -a "$bundled_memstack/$subdir" "$claude_dir/$subdir"
        fi
    done

    # Copy MEMSTACK.md
    if [[ -f "$bundled_memstack/MEMSTACK.md" ]] && [[ ! -f "$claude_dir/MEMSTACK.md" ]]; then
        cp "$bundled_memstack/MEMSTACK.md" "$claude_dir/"
    fi

    # Create config.local.json
    local project_name
    project_name=$(basename "$PROJECT_DIR")
    cat > "$claude_dir/config.local.json" <<CONF
{
  "version": "3.2.3",
  "author": "$(git config user.name 2>/dev/null || echo 'User')",
  "projects": {
    "$project_name": {
      "dir": "$PROJECT_DIR",
      "claude_md": "$PROJECT_DIR/CLAUDE.md",
      "deploy_target": "",
      "repo": ""
    }
  },
  "headroom": {
    "auto_start": true,
    "port": 8787,
    "health_url": "http://127.0.0.1:8787/health",
    "startup_flags": "",
    "required_extras": ["[code]"]
  }
}
CONF
    ok "Created config.local.json for project '$project_name'"

    # Initialize database
    if [[ -f "$claude_dir/db/memstack-db.py" ]]; then
        info "Initializing MemStack database..."
        if (cd "$claude_dir" && python3 db/memstack-db.py init \
            2>/dev/null); then
            ok "Database initialized"
        else
            warn "DB init skipped (may need manual setup)"
        fi
    fi

    # Install optional semantic search deps
    info 'Installing semantic search deps (lancedb, sentence-transformers)...'
    py_install lancedb sentence-transformers 2>/dev/null || \
        warn "Semantic search deps failed; keyword search still works"

    echo ""
}

# ============================================================================
# Step 6: Merge hooks in settings.json
# ============================================================================

merge_hooks() {
    info 'Step 6: Copy hooks to ~/.claude/hooks/; update settings.json'

    local hooks_dir="$CLAUDE_DIR/hooks"
    mkdir -p "$hooks_dir"

    # Copy memstack hook scripts into ~/.claude/hooks/
    local skills_hooks="$REPO_ROOT/.claude/memstack/hooks"
    if [[ -d "$skills_hooks" ]]; then
        for hook in \
            pre-tool-notify.sh pre-push.sh post-commit.sh \
            session-start.sh session-end.sh; do
            if [[ -f "$skills_hooks/$hook" ]]; then
                cp "$skills_hooks/$hook" "$hooks_dir/$hook"
            fi
        done
        chmod +x "$hooks_dir"/*.sh 2>/dev/null
        ok "Copied MemStack hooks to $hooks_dir/"
    else
        warn "MemStack hooks not found at $skills_hooks — skipping"
    fi

    # Back up existing settings
    if [[ -f "$SETTINGS_JSON" ]]; then
        cp "$SETTINGS_JSON" "$SETTINGS_JSON.bak.$(date +%s)"
        ok "Backed up existing settings.json"
    fi

    local existing='{}'
    if [[ -f "$SETTINGS_JSON" ]]; then
        existing=$(cat "$SETTINGS_JSON")
    fi

    # All hooks use absolute paths in ~/.claude/hooks/
    local jq_merge_hooks
    jq_merge_hooks=$(cat <<'JQF'
    . + {
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [
                        {
                            "type": "command",
                            "command": ($hd + "/rtk-rewrite.sh")
                        }
                    ]
                },
                {
                    "matcher": "Write|Edit|MultiEdit|Bash",
                    "hooks": [
                        {
                            "type": "command",
                            "command": ($hd + "/pre-tool-notify.sh"),
                            "timeout": 10000
                        }
                    ]
                },
                {
                    "matcher": "Bash",
                    "hooks": [
                        {
                            "type": "command",
                            "command": (
                                "bash -c '\''echo \"$CLAUDE_TOOL_INPUT\" "
                                + "| grep -q \"git push\" && "
                                + $hd
                                + "/pre-push.sh || exit 0'\''"
                            ),
                            "timeout": 60000
                        }
                    ]
                }
            ],
            "PostToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [
                        {
                            "type": "command",
                            "command": (
                                "bash -c '\''echo \"$CLAUDE_TOOL_INPUT\" "
                                + "| grep -q \"git commit\" && "
                                + $hd
                                + "/post-commit.sh || exit 0'\''"
                            ),
                            "timeout": 10000
                        }
                    ]
                }
            ],
            "SessionStart": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": ($hd + "/session-start.sh"),
                            "timeout": 10000
                        }
                    ]
                }
            ],
            "Stop": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": ($hd + "/session-end.sh"),
                            "timeout": 10000
                        }
                    ]
                }
            ]
        }
    }
JQF
    )

    echo "$existing" | jq --arg hd "$hooks_dir" "$jq_merge_hooks" \
        > "$SETTINGS_JSON.tmp" \
        && mv "$SETTINGS_JSON.tmp" "$SETTINGS_JSON"

    ok "All hooks registered with absolute paths in ~/.claude/hooks/"
    echo ""
}

# ============================================================================
# Step 7: Generate documentation
# ============================================================================

generate_docs() {
    info "Step 7: Generating STACK-SETUP.md"

    cat > "$PROJECT_DIR/STACK-SETUP.md" <<'DOCS'
# Whetstone (Claude Code stack)

This project was set up with Whetstone: Headroom, RTK, and MemStack for
token-efficient Claude Code sessions.

## Architecture

```
You (prompt) ──> Claude Code
                     │
                     ├── Bash tool calls ──> [RTK Hook] ──> rtk <cmd>
                     │                        rewrites         │
                     │                        commands      compressed
                     │                                      output back
                     │                                      to context
                     │
                     ├── Context window ──> [Headroom Proxy] ──> Anthropic API
                     │                      compresses            (50-90%
                     │                      messages              fewer tokens)
                     │
                     └── Skills/Memory ──> [MemStack]
                                           77 skills, SQLite DB,
                                           session persistence
```

**Token flow:**
1. **RTK** compresses CLI output *before* it enters Claude's context
   (60-90% savings)
2. **Headroom** compresses the entire context *before* it hits the API
   (50-90% savings)
3. **MemStack** provides structured memory + skills so Claude works more
   efficiently

## Quick Start

### Start a session with full optimization

```bash
# Recommended (PATH): proxy + Claude with Headroom URL set
whetstone

# Same as: headroom wrap claude (after setup)
whetstone claude

# Manual
headroom proxy --port 8787 &
claude
```

### RTK with Headroom URL in the environment

```bash
whetstone rtk gain
whetstone-rtk gain   # same
```

### Without the proxy (RTK + MemStack only)

```bash
claude   # RTK hooks and MemStack skills still active
```

## Tool Reference

### RTK (CLI Compression)

RTK is transparent — it runs via a Claude Code hook that rewrites bash
commands automatically.

**Verify it's working:**
```bash
rtk --version        # Should show version
rtk gain             # Token savings summary
rtk gain --history   # Command-by-command history
rtk gain --graph     # ASCII graph of savings over time
rtk discover         # Find missed optimization opportunities
```

**What gets compressed (examples):**
| Command | Before | After | Savings |
|---------|--------|-------|---------|
| `git status` | ~45 lines | ~5 lines | ~89% |
| `cargo test` | ~4800 tokens | ~11 tokens | ~99% |
| `git diff` (large) | ~21,500 tokens | ~1,259 tokens | ~94% |
| `ls -la` | verbose listing | tree format | ~70% |

**Manual usage (if needed):**
```bash
rtk git status       # Compact git status
rtk ls .             # Token-optimized directory listing
rtk grep "pattern" . # Grouped search results
rtk test cargo test  # Show test failures only
```

### Headroom (Context Compression)

Headroom is an HTTP proxy that sits between Claude Code and the Anthropic API.

**Check status:**
```bash
headroom --version
curl -s localhost:8787/health | jq    # Health check when proxy is running
curl -s localhost:8787/stats | jq     # Detailed compression stats
```

**Proxy commands:**
```bash
headroom proxy                        # Start on default port 8787
headroom proxy --port 9000            # Custom port
headroom proxy --budget 10.00         # Set spending budget
headroom proxy --log-file session.jsonl  # Log requests
```

**MCP tools (available in Claude Code):**
- `headroom_compress` — compress content on demand
- `headroom_retrieve` — retrieve original uncompressed content
- `headroom_stats` — session compression statistics

**Learn from past sessions:**
```bash
headroom learn              # Analyze past sessions
headroom learn --apply      # Write learnings to CLAUDE.md
```

### MemStack (Skills & Memory)

MemStack provides 77 specialist skills, persistent memory, and session
management.

**Key skills (trigger by keyword):**
| Skill | Trigger Words | What It Does |
|-------|---------------|--------------|
| Echo | "recall", "last session", "remember" | Semantic memory recall |
| Diary | "save diary", "log session" | Session logging + handoff |
| Work | "todo", "resume plan", "copy plan" | Task tracking with SQLite |
| State | "update state", "where was I" | Living STATE.md management |
| Verify | "verify", "check this work" | Pre-commit verification |
| Project | "handoff", "context running low" | Session handoff |
| Sight | "diagram", "visualize" | Architecture diagrams (Mermaid) |

**Slash commands:**
```
/memstack-search <query>    # Search memory database
/memstack-headroom          # Check Headroom proxy status
```

**Database CLI:**
```bash
python .claude/db/memstack-db.py stats       # DB statistics
python .claude/db/memstack-db.py search "q"   # Search sessions
python .claude/db/memstack-db.py get-sessions  # List sessions
python .claude/db/memstack-db.py export-md     # Export to markdown
```

## Hooks (What Fires When)

| Event | Hook | Tool |
|-------|------|------|
| Before any Bash call | RTK rewrites command | RTK |
| Before Write/Edit/Bash | TTS notification | MemStack |
| Before `git push` | Build check + secrets scan | MemStack |
| After `git commit` | Debug artifact scan | MemStack |
| Session start | Headroom auto-start + indexing | MemStack |
| Session end | Session reporting | MemStack |

## Configuration Files

| File | Purpose |
|------|---------|
| `~/.claude/settings.json` | Hook registrations (global) |
| `~/.claude/hooks/rtk-rewrite.sh` | RTK command rewriter |
| `.claude/config.local.json` | MemStack project config |
| `.claude/db/memstack.db` | MemStack SQLite database |
| `~/.headroom/models.json` | Headroom model config (optional) |

## Environment Variables

| Variable | Value | Purpose |
|----------|-------|---------|
| `ANTHROPIC_BASE_URL` | `http://127.0.0.1:8787` | Headroom proxy URL |
| `HEADROOM_LOG_LEVEL` | `INFO` (default) | Headroom logging verbosity |
| `OPENAI_API_KEY` | (optional) | Higher-quality embeddings for MemStack Echo |

## Troubleshooting

### RTK commands not being rewritten
```bash
rtk --version          # Is RTK installed?
which rtk              # Is it in PATH?
rtk gain               # Is it the RIGHT rtk? (not Rust Type Kit)
cat ~/.claude/hooks/rtk-rewrite.sh  # Does hook exist?
```

### Headroom proxy not compressing
```bash
curl localhost:8787/health    # Is proxy running?
echo $ANTHROPIC_BASE_URL      # Is env var set?
headroom proxy                # Start it manually
```

### MemStack skills not loading
```bash
ls .claude/MEMSTACK.md                 # Is it installed?
ls .claude/rules/                      # Are rules present?
python .claude/db/memstack-db.py stats # Is DB initialized?
```

### Hooks not firing
```bash
cat ~/.claude/settings.json | jq '.hooks'  # Check hook config
# Verify hook scripts are executable:
ls -la ~/.claude/hooks/
```

## Uninstall

### Remove MemStack (per-project)
```bash
rm -rf .claude/skills .claude/db .claude/rules .claude/commands .claude/MEMSTACK.md .claude/config.local.json
rm STACK-SETUP.md
```

### Remove RTK (global)
```bash
rtk init -g --uninstall    # Remove hooks
rm ~/.local/bin/rtk        # Remove binary
```

### Remove Headroom (global)
```bash
uv pip uninstall headroom-ai
# or: uv tool uninstall headroom-ai
# Remove ANTHROPIC_BASE_URL from ~/.bashrc / ~/.zshrc if desired
```

### Remove Whetstone wrappers
```bash
rm -f ~/.local/bin/whetstone ~/.local/bin/whetstone-rtk
```

### Restore original settings.json
```bash
# Find your backup:
ls ~/.claude/settings.json.bak.*
# Restore it:
cp ~/.claude/settings.json.bak.TIMESTAMP ~/.claude/settings.json
```
DOCS

    ok "Generated STACK-SETUP.md"
    echo ""
}

# ============================================================================
# Summary
# ============================================================================

summary() {
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}  Whetstone setup complete!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
    echo "Version: $WHETSTONE_VERSION"
    echo ""
    echo "Installed:"
    echo "  Headroom : $(headroom --version 2>/dev/null \
        || echo 'check uv pip install')"
    echo "  RTK      : $(rtk --version 2>/dev/null \
        || echo 'restart shell for PATH')"
    echo "  CLI      : whetstone, whetstone-rtk (~/.local/bin)"
    if [[ "$INSTALL_MEMSTACK" -eq 1 ]]; then
        echo "  MemStack : .claude/skills/"
    else
        echo "  MemStack : skipped (skills and hooks not installed)"
    fi
    echo ""
    echo "Start Claude Code with Headroom:"
    echo "  whetstone"
    echo ""
    echo "Or:"
    echo "  headroom wrap claude"
    echo ""
    if [[ "$INSTALL_MEMSTACK" -eq 1 ]]; then
        echo "Documentation: $PROJECT_DIR/STACK-SETUP.md"
    fi
    echo ""
}

# ============================================================================
# Main
# ============================================================================

main() {
    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║  Whetstone v$WHETSTONE_VERSION setup         ║${NC}"
    echo -e "${BLUE}║  Headroom + RTK + MemStack                   ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════╝${NC}"
    echo ""

    preflight
    install_headroom
    install_rtk
    configure_rtk
    configure_headroom
    install_whetstone_bins
    prompt_memstack_install
    if [[ "$INSTALL_MEMSTACK" -eq 1 ]]; then
        install_memstack
        merge_hooks
        generate_docs
    else
        info "Skipped MemStack skills, hook install, and STACK-SETUP.md."
    fi
    summary
}

main "$@"
