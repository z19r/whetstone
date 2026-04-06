---
name: work
description: "Plan execution — create, update, and resume task plans with per-task status tracking. Use when the user says 'plan', 'todo', 'task', 'copy plan', 'append plan', or 'resume plan'."
---

# 📋 Work — Plan execution

## Activation

When this skill activates, output:

`📋 Work — Managing task plan...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks to create/manage a plan** | ACTIVE |
| **User says "plan", "todo", "task", "priorities"** | ACTIVE |
| **User is executing a task (not managing the list)** | DORMANT |

## Protocol

- **Step 0 (silent)**: Before any plan operation, read `STATE.md`, `CLAUDE.md`, recent diary entries, and git state to compile context. Do not show this step to the user.
- **New plan / "copy plan"**: Parse tasks, save each via `python db/memstack-db.py add-plan-task '<json>'`
- **Update / "append plan"**: Update task status via `python db/memstack-db.py update-task '<json>'`
- **Resume / "resume plan"**: Load plan via `python db/memstack-db.py get-plan <project>`, show status summary
- **Quick query / "todo"**: Show all pending and in-progress tasks with status indicators

## Status Values
`pending` | `in_progress` | `completed` | `blocked`

## Ownership
- "plan" / "todo" / "task" / "priorities" = Work only
- Do not activate when the user is executing a task (not managing the list)
