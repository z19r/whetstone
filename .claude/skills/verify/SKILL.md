---
name: verify
description: "Pre-commit work verification — run checks and generate a pass/fail report before committing. Use when the user says 'verify', 'check this work', or 'does it pass'."
---

# ✅ Verify — Pre-commit verification

## Activation

When this skill activates, output:

`✅ Verify — Running verification checks...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks to verify work before committing** | ACTIVE |
| **User says "verify", "check this work", "does it pass"** | ACTIVE |
| **Automated git push checks** | DORMANT — that's the pre-push hook |

## Protocol

1. Run the project's build command (detect from package.json, Makefile, justfile, etc.)
2. Run the project's test suite if one exists
3. Run linting if configured
4. Check for uncommitted debug artifacts (console.log, debugger, TODO)
5. Check for secrets or sensitive data in staged files
6. Generate a pass/fail report:

```
✅ Verify — Report

Build:    ✅ passed
Tests:    ✅ 42 passed, 0 failed
Lint:     ✅ no issues
Debug:    ✅ no artifacts found
Secrets:  ✅ clean

Verdict: READY TO COMMIT
```

## Ownership
- "verify" / "check this work" / "does it pass" = Verify only (not Seal hook)
