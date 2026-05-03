# Whetstone (Claude Code stack)

This project was set up with Whetstone: Headroom, RTK, and MemStack for
token-efficient Claude Code sessions.

## Quick Start

```bash
whetstone              # Start Claude with Headroom proxy
whetstone claude       # Same as above
```

## Tools

| Tool | Purpose | Savings |
|------|---------|---------|
| Headroom | HTTP proxy compresses context before API | 50-90% |
| RTK | Hook rewrites CLI output before entering context | 60-90% |
| MemStack | Skills, SQLite memory, session hooks | efficiency |

## Hooks

| Event | Hook | Tool |
|-------|------|------|
| Before Bash | RTK rewrites command | RTK |
| Before Write/Edit/Bash | TTS notification | MemStack |
| Before `git push` | Build check + secrets scan | MemStack |
| After `git commit` | Debug artifact scan | MemStack |
| Session start | Headroom auto-start + indexing | MemStack |
| Session end | Session reporting | MemStack |

## Configuration

| File | Purpose |
|------|---------|
| `~/.claude/settings.json` | Hook registrations (global) |
| `.claude/config.local.json` | Project config |
| `.claude/db/memstack.db` | SQLite database |

## Database CLI

```bash
whetstone db stats
whetstone db search "query"
whetstone db get-sessions
whetstone db export-md
```

## Uninstall

Per-project: `whetstone uninstall`
