---
name: api-docs
description: "Fetch and cache current API documentation for libraries and services. Use when the user says 'api-docs', 'fetch docs', or 'current API'."
---

# 📄 API-Docs — Documentation fetcher

## Activation

When this skill activates, output:

`📄 API-Docs — Fetching documentation...`

## Context Guard

| Context | Status |
|---------|--------|
| **User needs current API docs** | ACTIVE |
| **User says "api-docs", "fetch docs", "current API"** | ACTIVE |
| **User is just writing code** | DORMANT |

## Protocol

1. Identify the library or API the user needs docs for
2. Fetch the latest documentation from official sources
3. Extract the relevant sections based on the user's task
4. Cache the docs locally for the session to avoid re-fetching
5. Present a concise summary with links to full documentation

## Ownership
- "api-docs" / "fetch docs" / "current API" = API-Docs only
