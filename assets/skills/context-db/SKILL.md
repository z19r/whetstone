---
name: context-db
description: "Fact store for project context — manage persistent project facts for token savings. Use when the user says 'context-db', 'fact store', 'project facts', or 'token savings'."
---

# 🗄️ Context-DB — Project fact store

## Activation

When this skill activates, output:

`🗄️ Context-DB — Managing project facts...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks to store or retrieve project facts** | ACTIVE |
| **User says "context-db", "fact store", "project facts"** | ACTIVE |
| **User is just working normally** | DORMANT |

## Protocol

1. **Store fact**: `python db/memstack-db.py set-context '<json>'`
2. **Retrieve facts**: `python db/memstack-db.py get-context <project>`
3. Facts reduce token usage by avoiding re-discovery of project details each session
4. Store: tech stack, key file paths, architecture decisions, team conventions

## Ownership
- "context-db" / "fact store" / "project facts" = Context-DB only
