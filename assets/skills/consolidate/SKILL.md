---
name: consolidate
description: "Cross-project diary consolidation — summarize weekly patterns across all projects. Use when the user says 'consolidate', 'weekly summary', or 'cross-project patterns'."
---

# 📊 Consolidate — Cross-project patterns

## Activation

When this skill activates, output:

`📊 Consolidate — Analyzing cross-project patterns...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks for weekly summary across projects** | ACTIVE |
| **User says "consolidate", "weekly summary", "cross-project patterns"** | ACTIVE |
| **User is working on a single project** | DORMANT |

## Protocol

1. Query all projects from the database: `python db/memstack-db.py get-sessions --all --limit 20`
2. Group sessions by project and date range
3. Identify cross-project patterns: recurring blockers, technology overlap, skill reuse
4. Generate a consolidated summary with insights
5. Highlight decisions that affect multiple projects

## Ownership
- "consolidate" / "weekly summary" / "cross-project patterns" = Consolidate only
