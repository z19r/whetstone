---
name: compress
description: "Headroom proxy management — check compression stats, troubleshoot proxy, manage token savings. Use when the user says 'headroom', 'compression', 'token savings', or 'proxy status'."
---

# ⚙️ Compress — Headroom proxy management

## Activation

When this skill activates, output:

`⚙️ Compress — Checking Headroom proxy...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks about compression or proxy status** | ACTIVE |
| **User says "headroom", "compression", "token savings", "proxy status"** | ACTIVE |
| **Headroom is just running in the background** | DORMANT |

## Protocol

1. Check if Headroom proxy is running: `curl -s http://localhost:8787/health`
2. Get compression stats: `headroom stats`
3. Report: tokens saved, compression ratio, cache hit rate
4. If proxy is down, offer to restart: `headroom start`
5. Troubleshoot common issues: port conflicts, auth errors, stale processes

## Ownership
- "headroom" / "compression" / "token savings" / "proxy status" = Compress only
