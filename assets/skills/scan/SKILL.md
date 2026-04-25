---
name: scan
description: "Project analysis and pricing — analyze codebase complexity and estimate project cost. Use when the user says 'scan project', 'estimate', or 'how much to charge'."
---

# 🔍 Scan — Project analysis & pricing

## Activation

When this skill activates, output:

`🔍 Scan — Analyzing project...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks to analyze or estimate a project** | ACTIVE |
| **User says "scan project", "estimate", "how much to charge"** | ACTIVE |
| **User is just browsing code** | DORMANT |

## Protocol

1. Count files, lines of code, and languages used
2. Identify frameworks, dependencies, and architecture patterns
3. Assess complexity: simple / moderate / complex / enterprise
4. Estimate effort in hours based on complexity
5. Suggest pricing based on effort and market rates
6. Present findings in a structured report

## Ownership
- "scan project" / "estimate" / "how much to charge" = Scan only
