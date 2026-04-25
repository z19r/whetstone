---
name: governor
description: "Portfolio governance — enforce tier and phase constraints on projects. Use when the user says 'new project', 'what tier', 'scope', or 'project init'."
---

# 🏛️ Governor — Portfolio governance

## Activation

When this skill activates, output:

`🏛️ Governor — Checking project governance...`

## Context Guard

| Context | Status |
|---------|--------|
| **User is initializing a new project** | ACTIVE |
| **User says "new project", "what tier", "scope", "project init"** | ACTIVE |
| **User is working within an established project** | DORMANT |

## Protocol

1. Determine project tier based on complexity and scope:
   - **Tier 1 (Solo)**: Single developer, simple scope, <1 week
   - **Tier 2 (Team)**: Multi-developer, moderate scope, 1-4 weeks
   - **Tier 3 (Enterprise)**: Cross-team, complex scope, >1 month
2. Apply phase constraints based on tier
3. Set up appropriate governance files (STATE.md, CLAUDE.md sections)
4. Advise on scope boundaries

## Ownership
- "new project" / "what tier" / "scope" / "project init" = Governor only
