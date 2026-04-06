---
name: project
description: "Session handoff and lifecycle management — save project state for future sessions. Use when the user says 'save project', 'handoff', or when context is running low."
---

# 💾 Project — Session handoff & lifecycle

## Activation

When this skill activates, output:

`💾 Project — Saving project state...`

## Context Guard

| Context | Status |
|---------|--------|
| **User says "save project", "handoff"** | ACTIVE |
| **Context window running low** | ACTIVE — suggest handoff |
| **User says "save diary" or "log session"** | DORMANT — that's Diary territory |

## Protocol

1. Capture current project state: branch, uncommitted changes, recent commits
2. Summarize active work: what was being done, what's pending
3. Save context: `python db/memstack-db.py set-context '<json>'`
4. Generate handoff document with pickup instructions
5. If context is running low, proactively suggest saving state

## Ownership
- "save project" / "handoff" = Project only (not Diary, not Echo)
