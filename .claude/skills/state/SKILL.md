---
name: state
description: "Living STATE.md — track current task, blockers, and next steps in a persistent state file. Use when the user says 'update state', 'project state', or 'where was I'."
---

# 📍 State — Living STATE.md management

## Activation

When this skill activates, output:

`📍 State — Updating project state...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks about current state** | ACTIVE |
| **User says "update state", "project state", "where was I"** | ACTIVE |
| **User says "recall" or "remember"** | DORMANT — that's Echo territory |
| **User says "save project" or "handoff"** | DORMANT — that's Project territory |

## Protocol

1. Read current `STATE.md` if it exists
2. Update with: current task, blockers, next steps, recent changes
3. Include timestamp and branch info
4. Write updated `STATE.md` to project root

## STATE.md Format
```markdown
# Project State
**Updated:** {timestamp}
**Branch:** {current branch}

## Current Task
{what's being worked on}

## Blockers
{anything blocking progress, or "None"}

## Next Steps
{ordered list of what to do next}

## Recent Changes
{summary of recent commits/modifications}
```

## Ownership
- "update state" / "project state" / "where was I" = State only (not Echo, not Project)
