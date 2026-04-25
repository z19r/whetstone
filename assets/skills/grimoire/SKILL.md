---
name: grimoire
description: "CLAUDE.md management — update project context files with decisions and patterns. Use when the user says 'update context', 'update claude', or 'save library'."
---

# 📖 Grimoire — CLAUDE.md management

## Activation

When this skill activates, output:

`📖 Grimoire — Updating project context...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks to update CLAUDE.md** | ACTIVE |
| **User says "update context", "update claude", "save library"** | ACTIVE |
| **User is just reading CLAUDE.md** | DORMANT |

## Protocol

1. Read the current `CLAUDE.md` file
2. Identify what needs to be added or updated based on the user's request
3. Merge new information without duplicating existing content
4. Preserve the existing structure and formatting
5. Add new sections as appropriate for decisions, patterns, or conventions

## Ownership
- "update context" / "update claude" / "save library" = Grimoire only
