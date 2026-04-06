---
name: sight
description: "Architecture visualization — generate diagrams of project structure, data flow, and system architecture. Use when the user says 'draw', 'diagram', 'visualize', or 'architecture'."
---

# 👁️ Sight — Architecture visualization

## Activation

When this skill activates, output:

`👁️ Sight — Generating visualization...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks for a diagram or visualization** | ACTIVE |
| **User says "draw", "diagram", "visualize", "architecture"** | ACTIVE |
| **User is discussing architecture without requesting visuals** | DORMANT |

## Protocol

1. Analyze the target: project structure, data flow, API routes, component tree, etc.
2. Choose the appropriate diagram type: flowchart, sequence, ERD, component, deployment
3. Generate Mermaid diagram syntax
4. Present the diagram with a brief explanation
5. Offer to refine or generate alternative views

## Supported Diagram Types
- **Flowchart**: Process flows, decision trees
- **Sequence**: API call chains, user interactions
- **ERD**: Database relationships
- **Component**: Module dependencies, architecture layers
- **Deployment**: Infrastructure, service topology

## Ownership
- "draw" / "diagram" / "visualize" / "architecture" = Sight only
