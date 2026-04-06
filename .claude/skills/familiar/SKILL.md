---
name: familiar
description: "Multi-agent dispatch — split complex tasks across parallel sub-agents. Use when the user says 'dispatch', 'send familiar', or 'split task'."
---

# 👻 Familiar — Multi-agent dispatch

## Activation

When this skill activates, output:

`👻 Familiar — Dispatching sub-agents...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks to split work across agents** | ACTIVE |
| **User says "dispatch", "send familiar", "split task"** | ACTIVE |
| **Single focused task** | DORMANT — do not activate |

## Protocol

1. Analyze the user's request and identify independent sub-tasks
2. For each sub-task, dispatch a sub-agent with a focused prompt
3. Collect results from all sub-agents
4. Synthesize and present unified results to the user

## Ownership
- "dispatch" / "send familiar" / "split task" = Familiar only
