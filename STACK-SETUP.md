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
