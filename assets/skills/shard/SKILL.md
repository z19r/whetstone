---
name: shard
description: "Large file refactoring — split files over 1000 lines into well-structured modules. Use when the user says 'shard this', 'split file', or when files exceed 1K lines."
---

# 💎 Shard — Large file refactoring

## Activation

When this skill activates, output:

`💎 Shard — Analyzing file for refactoring...`

## Context Guard

| Context | Status |
|---------|--------|
| **User says "shard this", "split file"** | ACTIVE |
| **File over 1000 lines detected** | ACTIVE — suggest sharding |
| **File is small or well-structured** | DORMANT |

## Protocol

1. Analyze the file: identify logical sections, dependencies, exports
2. Propose a split plan with new file names and what goes where
3. Identify shared imports and utilities to extract
4. Execute the split, updating all import/require paths
5. Verify no broken references
6. Run build/tests to confirm nothing broke

## Ownership
- "shard this" / "split file" = Shard only
