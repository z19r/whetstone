---
name: diary
description: "Session documentation with structured handoff — log sessions to SQLite with auto-extracted insights. Use when the user says 'save diary', 'log session', or 'wrapping up'."
---

# 📓 Diary — Session documentation

## Activation

When this skill activates, output:

`📓 Diary — Logging session...`

## Context Guard

| Context | Status |
|---------|--------|
| **User says "save diary", "log session", "wrapping up"** | ACTIVE |
| **End of significant work session** | ACTIVE |
| **Mid-task or session start with nothing done** | DORMANT |

## Protocol

1. Summarize: project, date, accomplishments, files changed, commits, decisions, next steps
2. Include **Session Handoff** section: what's in progress, uncommitted changes, exact pickup instruction, session context that would be lost
3. Save session: `python db/memstack-db.py add-session '<json>'`
4. Extract each decision as an insight: `python db/memstack-db.py add-insight '<json>'`
5. Update project context: `python db/memstack-db.py set-context '<json>'`
6. Save markdown backup to `memory/sessions/{date}-{project}.md`

## Ownership
- "save diary" / "log session" / "wrapping up" = Diary only (not Project, not Echo)
- Do not activate mid-task or at session start when nothing has been done
