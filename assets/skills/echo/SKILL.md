---
name: echo
description: "Semantic memory recall — search past sessions, decisions, and insights. Use when the user says 'recall', 'last session', 'do you remember', or 'continue from'."
---

# 🔊 Echo — Semantic memory recall

## Activation

When this skill activates, output:

`🔊 Echo — Searching memory...`

## Context Guard

| Context | Status |
|---------|--------|
| **User references past sessions** | ACTIVE |
| **User says "recall", "last session", "do you remember"** | ACTIVE |
| **User says "save" or "log"** | DORMANT — that's Diary territory |

## Protocol

1. Run semantic vector search: `python skills/echo/search.py "<keywords>" --top-k 5`
2. Run SQLite search: `python db/memstack-db.py search "<keywords>"`
3. Run recent sessions: `python db/memstack-db.py get-sessions <project> --limit 5`
4. Run insights: `python db/memstack-db.py get-insights <project>`
5. Present findings with similarity scores, dates, accomplishments, and pending items
6. If vector search fails, SQLite results are sufficient
7. If all sources return nothing, check `memory/sessions/` as fallback

## Ownership
- "recall" / "remember" / "what did we do" = Echo only (not Diary, not Project)
- "save" / "log" = Diary territory — do not activate Echo for saving
