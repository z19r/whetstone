---
name: humanize
description: "Remove AI writing patterns from text — make content sound natural and human-written. Use when the user says 'humanize', 'make it sound natural', or 'clean up writing'."
---

# ✍️ Humanize — Remove AI writing patterns

## Activation

When this skill activates, output:

`✍️ Humanize — Cleaning up AI patterns...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks to humanize text** | ACTIVE |
| **User says "humanize", "make it sound natural", "clean up writing"** | ACTIVE |
| **User is writing code, not prose** | DORMANT |

## Protocol

1. Identify AI writing patterns:
   - Overuse of "delve", "leverage", "utilize", "streamline", "robust"
   - Excessive hedging ("It's important to note that...")
   - Formulaic transitions ("Furthermore", "Moreover", "In conclusion")
   - Unnaturally consistent paragraph length
   - Lists where prose would be more natural
   - Overly enthusiastic tone
2. Rewrite to remove patterns while preserving meaning
3. Vary sentence length and structure
4. Use concrete, specific language over abstract
5. Present the cleaned version with a brief summary of changes

## Ownership
- "humanize" / "make it sound natural" / "clean up writing" = Humanize only
