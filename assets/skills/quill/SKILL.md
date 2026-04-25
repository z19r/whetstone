---
name: quill
description: "Client quotation generator — create professional project quotes and proposals. Use when the user says 'create quotation', 'generate quote', or 'proposal'."
---

# ✒️ Quill — Client quotation generator

## Activation

When this skill activates, output:

`✒️ Quill — Generating quotation...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks to create a quote or proposal** | ACTIVE |
| **User says "create quotation", "generate quote", "proposal"** | ACTIVE |
| **User is discussing pricing abstractly** | DORMANT |

## Protocol

1. Gather project details: scope, deliverables, timeline
2. If Scan data is available, use it for effort estimates
3. Structure the quotation with: overview, scope, deliverables, timeline, pricing, terms
4. Format as a professional document
5. Save to project directory

## Ownership
- "create quotation" / "generate quote" / "proposal" = Quill only
