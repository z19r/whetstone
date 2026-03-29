#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# setup-stack.sh — Install & configure Headroom + RTK + MemStack for Claude Code
#
# Usage: Run from any git project root:
#   bash /path/to/setup-stack.sh
#
# What it does:
#   1. Installs Headroom (context compression proxy) if missing
#   2. Installs RTK (CLI output compression) if missing
#   3. Configures RTK hooks globally for Claude Code
#   4. Configures Headroom as the API proxy
#   5. Installs MemStack (skill/memory framework) into the current project
#   6. Merges all hooks in ~/.claude/settings.json
#   7. Generates STACK-SETUP.md documentation
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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(pwd)"
CLAUDE_DIR="$HOME/.claude"
SETTINGS_JSON="$CLAUDE_DIR/settings.json"
SHELL_PROFILE=""

# Detect shell profile
if [[ -f "$HOME/.zshrc" ]]; then
    SHELL_PROFILE="$HOME/.zshrc"
elif [[ -f "$HOME/.bashrc" ]]; then
    SHELL_PROFILE="$HOME/.bashrc"
elif [[ -f "$HOME/.profile" ]]; then
    SHELL_PROFILE="$HOME/.profile"
fi

# ============================================================================
# Pre-flight checks
# ============================================================================

preflight() {
    info "Running pre-flight checks..."

    # Must be in a git repo
    if ! git rev-parse --is-inside-work-tree &>/dev/null; then
        fail "Not inside a git repository. Run this from a git project root.\n       To init one: git init"
    fi
    ok "Inside git repo: $(git rev-parse --show-toplevel)"

    # Python 3.10+
    if ! command -v python3 &>/dev/null; then
        fail "Python 3 not found. Install Python 3.10+."
    fi
    PY_VERSION=$(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')
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
            fail "Cannot install jq automatically. Install it manually: https://jqlang.github.io/jq/download/"
        fi
    fi
    ok "jq $(jq --version 2>/dev/null || echo 'installed')"

    # curl
    command -v curl &>/dev/null || fail "curl not found."
    ok "curl available"

    # pip
    if ! command -v pip &>/dev/null && ! command -v pip3 &>/dev/null; then
        fail "pip not found. Install pip for Python."
    fi
    ok "pip available"

    echo ""
}

# ============================================================================
# Step 1: Install Headroom
# ============================================================================

install_headroom() {
    info "Step 1: Headroom (context compression proxy)"

    if command -v headroom &>/dev/null; then
        local ver
        ver=$(headroom --version 2>/dev/null || echo "unknown")
        ok "Headroom already installed: $ver"
    else
        info "Installing headroom-ai with proxy, code, and MCP extras..."
        pip install "headroom-ai[proxy,code,mcp]" || pip3 install "headroom-ai[proxy,code,mcp]"
        ok "Headroom installed: $(headroom --version 2>/dev/null)"
    fi
    echo ""
}

# ============================================================================
# Step 2: Install RTK
# ============================================================================

install_rtk() {
    info "Step 2: RTK (CLI output compression)"

    if command -v rtk &>/dev/null; then
        # Verify it's the right rtk (not Rust Type Kit)
        if rtk gain &>/dev/null; then
            ok "RTK already installed: $(rtk --version 2>/dev/null)"
        else
            warn "Found 'rtk' but it's not rtk-ai (might be Rust Type Kit). Installing correct version..."
            curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh
            # Ensure ~/.local/bin is in PATH
            export PATH="$HOME/.local/bin:$PATH"
            ok "RTK installed: $(rtk --version 2>/dev/null)"
        fi
    else
        info "Installing RTK via install script..."
        curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh
        export PATH="$HOME/.local/bin:$PATH"
        ok "RTK installed: $(rtk --version 2>/dev/null)"
    fi

    # Ensure PATH persistence
    if [[ -n "$SHELL_PROFILE" ]] && ! grep -q 'HOME/.local/bin' "$SHELL_PROFILE" 2>/dev/null; then
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$SHELL_PROFILE"
        info "Added ~/.local/bin to PATH in $SHELL_PROFILE"
    fi
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
        warn "RTK binary not functional yet — hook will activate on next session"
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
            echo "# Headroom context compression proxy for Claude Code" >> "$SHELL_PROFILE"
            echo "export ANTHROPIC_BASE_URL=$base_url" >> "$SHELL_PROFILE"
            ok "Added ANTHROPIC_BASE_URL=$base_url to $SHELL_PROFILE"
        fi
    else
        warn "No shell profile found. Set manually: export ANTHROPIC_BASE_URL=$base_url"
    fi

    # Export for current session
    export ANTHROPIC_BASE_URL="$base_url"
    echo ""
}

# ============================================================================
# Step 5: Install MemStack for the current project
# ============================================================================

install_memstack() {
    info "Step 5: Installing MemStack into project"

    local skills_dir="$PROJECT_DIR/.claude/skills"

    if [[ -d "$skills_dir/skills" ]]; then
        ok "MemStack already installed at $skills_dir"
    else
        info "Cloning MemStack into .claude/skills/..."
        mkdir -p "$PROJECT_DIR/.claude"
        git clone https://github.com/cwinvestments/memstack.git "$skills_dir"
        ok "MemStack cloned"
    fi

    # Create config.local.json
    local project_name
    project_name=$(basename "$PROJECT_DIR")
    cat > "$skills_dir/config.local.json" <<CONF
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
    if [[ -f "$skills_dir/db/memstack-db.py" ]]; then
        info "Initializing MemStack database..."
        (cd "$skills_dir" && python3 db/memstack-db.py init 2>/dev/null) && ok "Database initialized" || warn "DB init skipped (may need manual setup)"
    fi

    # Install optional semantic search deps
    info "Installing semantic search dependencies (lancedb, sentence-transformers)..."
    pip install lancedb sentence-transformers 2>/dev/null || pip3 install lancedb sentence-transformers 2>/dev/null || warn "Semantic search deps failed to install — keyword search will still work"

    echo ""
}

# ============================================================================
# Step 6: Merge hooks in settings.json
# ============================================================================

merge_hooks() {
    info "Step 6: Installing hooks to ~/.claude/hooks/ and updating settings.json"

    local hooks_dir="$CLAUDE_DIR/hooks"
    mkdir -p "$hooks_dir"

    # Copy memstack hook scripts into ~/.claude/hooks/
    local skills_hooks="$PROJECT_DIR/.claude/skills/.claude/hooks"
    if [[ -d "$skills_hooks" ]]; then
        for hook in pre-tool-notify.sh pre-push.sh post-commit.sh session-start.sh session-end.sh; do
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
    echo "$existing" | jq --arg hd "$hooks_dir" '
    . + {
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": ($hd + "/rtk-rewrite.sh")}]
                },
                {
                    "matcher": "Write|Edit|MultiEdit|Bash",
                    "hooks": [{"type": "command", "command": ($hd + "/pre-tool-notify.sh"), "timeout": 10000}]
                },
                {
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": ("bash -c '\''echo \"$CLAUDE_TOOL_INPUT\" | grep -q \"git push\" && " + $hd + "/pre-push.sh || exit 0'\''"), "timeout": 60000}]
                }
            ],
            "PostToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": ("bash -c '\''echo \"$CLAUDE_TOOL_INPUT\" | grep -q \"git commit\" && " + $hd + "/post-commit.sh || exit 0'\''"), "timeout": 10000}]
                }
            ],
            "SessionStart": [
                {"hooks": [{"type": "command", "command": ($hd + "/session-start.sh"), "timeout": 10000}]}
            ],
            "Stop": [
                {"hooks": [{"type": "command", "command": ($hd + "/session-end.sh"), "timeout": 10000}]}
            ]
        }
    }' > "$SETTINGS_JSON.tmp" && mv "$SETTINGS_JSON.tmp" "$SETTINGS_JSON"

    ok "All hooks registered with absolute paths in ~/.claude/hooks/"
    echo ""
}

# ============================================================================
# Step 7: Generate documentation
# ============================================================================

generate_docs() {
    info "Step 7: Generating STACK-SETUP.md"

    cat > "$PROJECT_DIR/STACK-SETUP.md" <<'DOCS'
# Claude Code Optimization Stack

This project has been configured with three complementary tools that optimize Claude Code's token usage and provide structured project management.

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
1. **RTK** compresses CLI output *before* it enters Claude's context (60-90% savings)
2. **Headroom** compresses the entire context *before* it hits the API (50-90% savings)
3. **MemStack** provides structured memory + skills so Claude works more efficiently

## Quick Start

### Start a session with full optimization

```bash
# Option A: Use headroom wrap (starts proxy + Claude Code together)
headroom wrap claude

# Option B: Manual (start proxy first, then Claude Code)
headroom proxy --port 8787 &
claude
```

### Without the proxy (RTK + MemStack only)

```bash
claude   # RTK hooks and MemStack skills still active
```

## Tool Reference

### RTK (CLI Compression)

RTK is transparent — it runs via a Claude Code hook that rewrites bash commands automatically.

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

MemStack provides 77 specialist skills, persistent memory, and session management.

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
python .claude/skills/db/memstack-db.py stats       # DB statistics
python .claude/skills/db/memstack-db.py search "q"   # Search sessions
python .claude/skills/db/memstack-db.py get-sessions  # List sessions
python .claude/skills/db/memstack-db.py export-md     # Export to markdown
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
| `.claude/skills/config.local.json` | MemStack project config |
| `.claude/skills/db/memstack.db` | MemStack SQLite database |
| `~/.headroom/models.json` | Headroom model config (optional) |

## Environment Variables

| Variable | Value | Purpose |
|----------|-------|---------|
| `ANTHROPIC_BASE_URL` | `http://127.0.0.1:8787` | Route Claude Code through Headroom proxy |
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
ls .claude/skills/MEMSTACK.md          # Is it cloned?
ls .claude/skills/.claude/rules/       # Are rules present?
python .claude/skills/db/memstack-db.py stats  # Is DB initialized?
```

### Hooks not firing
```bash
cat ~/.claude/settings.json | jq '.hooks'  # Check hook config
# Verify hook scripts are executable:
ls -la .claude/skills/.claude/hooks/
```

## Uninstall

### Remove MemStack (per-project)
```bash
rm -rf .claude/skills
rm STACK-SETUP.md
```

### Remove RTK (global)
```bash
rtk init -g --uninstall    # Remove hooks
rm ~/.local/bin/rtk        # Remove binary
```

### Remove Headroom (global)
```bash
pip uninstall headroom-ai
# Remove from shell profile:
# Delete the ANTHROPIC_BASE_URL line from ~/.bashrc or ~/.zshrc
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
    echo -e "${GREEN}  Stack setup complete!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
    echo "Installed:"
    echo "  Headroom : $(headroom --version 2>/dev/null || echo 'check pip install')"
    echo "  RTK      : $(rtk --version 2>/dev/null || echo 'restart shell for PATH')"
    echo "  MemStack : .claude/skills/"
    echo ""
    echo "To start an optimized Claude Code session:"
    echo "  headroom wrap claude"
    echo ""
    echo "Or manually:"
    echo "  headroom proxy --port 8787 &"
    echo "  claude"
    echo ""
    echo "Documentation: $PROJECT_DIR/STACK-SETUP.md"
    echo ""
}

# ============================================================================
# Main
# ============================================================================

main() {
    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║  Claude Code Optimization Stack Setup        ║${NC}"
    echo -e "${BLUE}║  Headroom + RTK + MemStack                   ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════╝${NC}"
    echo ""

    preflight
    install_headroom
    install_rtk
    configure_rtk
    configure_headroom
    install_memstack
    merge_hooks
    generate_docs
    summary
}

main "$@"
