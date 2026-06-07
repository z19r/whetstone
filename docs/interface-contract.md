# Whetstone v3 — Interface Contract

**Status:** Phase 0 verified · **Captured:** 2026-06-07 · **Branch:** `v3`

This document records the verified external command surface that whetstone v3
delegates to. Everything below was inspected with `--help` against the pinned
versions on the captured date. Phases 1, 3, and 6 build on this contract; if a
flag changes upstream, update this file before changing whetstone code.

## Pinned tool versions

| Tool       | Pinned version | Source / install path                                          |
| ---------- | -------------- | -------------------------------------------------------------- |
| `headroom` | **0.23.0**     | PyPI `headroom-ai` (via `uv tool install`)                     |
| `rtk`      | **0.42.3**     | `install.sh` from `github.com/rtk-ai/rtk` (NOT crates.io)      |
| `icm`      | **0.10.43**    | crates.io `icm` (cargo install)                                |

> **Name collision note:** the `rtk` crate on crates.io is "Rust Type Kit" — a
> different project. Whetstone installs rtk-ai via the upstream install script
> and detects collisions via `rtk gain` (rtk-ai-specific subcommand).

## 0.1 — `headroom wrap` decision

**Question:** does `headroom wrap <tool>` exist in the pinned Headroom version?

**Evidence (`headroom --help`, `headroom wrap --help`):** YES.

```
headroom wrap [OPTIONS] COMMAND [ARGS]...
  Starts a Headroom proxy, configures environment, launches target tool so
  all API calls route through Headroom automatically.

  Examples:
    headroom wrap claude
    headroom wrap codex
    headroom wrap copilot -- --model claude-sonnet-4-20250514
    headroom wrap aider
    headroom wrap cline
    headroom wrap continue
    headroom wrap goose
    headroom wrap openhands

  Options (verified for `headroom wrap claude`):
    -p, --port INTEGER     Proxy port (default: 8787)
    --no-context-tool
    --no-rtk
    --no-mcp
    --no-serena
    --no-proxy
    --learn                persistent cross-session memory
    -v, --verbose
```

**Decision:** whetstone v3's default command becomes `headroom wrap claude
--model claude-opus-4-7` (or current default). No fallback required against
the pinned 0.23.0. If we later pin a version that lacks `wrap`, the fallback
is: `headroom proxy` in background + `exec ANTHROPIC_BASE_URL=… claude …`.

Inverse exists: `headroom unwrap` undoes durable wrapping. Useful for the
v3 `whetstone uninstall` path.

## 0.2 — Init verbs and ICM import verb

### `rtk init` — verified surface

```
rtk init [OPTIONS]

Options:
  -g, --global         Add to global assistant config directory instead of local
      --agent <AGENT>  claude (default) | cursor | windsurf | cline | kilocode | pi | hermes
      --gemini         Initialize for Gemini CLI instead of Claude Code
      --opencode       Install OpenCode plugin
      --copilot        Install GitHub Copilot
      --codex          Target Codex CLI (uses AGENTS.md + RTK.md)
      --auto-patch     Auto-patch settings.json
      --no-patch       Skip settings.json patching
      --hook-only      Hook only, no RTK.md
      --show           Print what would be installed
      --dry-run
      --skip-env       Set SKIP_ENV_VALIDATION=1 for processes
      --uninstall      Remove RTK artifacts
  -v, --verbose...     -v / -vv / -vvv
```

**Whetstone v3 invocation (Phase 1):** `rtk init --auto-patch` (default agent=claude).
For non-Claude editors detected by `whetstone doctor`, pass `--agent <name>`
or the explicit flag (`--gemini`, `--copilot`, `--codex`, `--opencode`).

### `icm init` — verified surface

```
icm init [OPTIONS]

Options:
      --db <DB>      Path to SQLite database
  -m, --mode <MODE>  cli | skill | hook | mcp | standard | all  [default: standard]
                     - standard: cli + skill + hook (NO MCP)
                     - all:      everything including MCP
                     - mcp:      MCP server only
  -f, --force        Overwrite existing hook entries pointing at a stale icm binary
      --no-embeddings  Skip embedding model download
```

**Whetstone v3 invocation (Phase 1):** `icm init --mode standard` (matches plan §3.2).
Opt-in MCP via `--mode all` only when the user explicitly requests it
(see plan §1.3 `MemoryProvider::Icm { mode }`).

### ICM memory import — **the migration-blocking verb** (plan §3.4)

ICM exposes two paths for adding memories:

| Verb         | Use case                                   |
| ------------ | ------------------------------------------ |
| `icm store`  | Single memory — `--topic`, `--importance`, `-k` (keywords), content positional |
| `icm import` | Bulk — accepts a path; supports `--format auto` and explicit format flags for claude-ai, claude-code, chatgpt, slack, plain text |

**Decision for Phase 3 migration:** prefer `icm import` with a generated file
(text or JSONL) containing the v2 MemStack → ICM payload. Fall back to a
per-memory `icm store` loop only if `icm import` rejects the format we emit.
Either path satisfies plan §3.4's "ICM's verified import path."

Idempotency tag (plan §3.4 line 98) goes into the import payload as
`source=whetstone-migration, migration-id=<ts>` so the importer can refuse
duplicates.

## 0.3 — RTK `MIN_VERSION` decision

| Source                                | Value     |
| ------------------------------------- | --------- |
| Installed (this machine, today)       | 0.42.3    |
| `src/rtk.rs::MIN_VERSION` (v2.6.0)    | 0.39.0    |
| `latest_remote_version()`             | GitHub releases tag (rtk-ai/rtk) |

The plan (0.3) flags "set a real floor or drop entirely." The verified
`rtk init` flag surface above (notably `--auto-patch`, `--no-patch`,
`--agent <AGENT>`) was stabilized between 0.39 and 0.42, so a floor is still
load-bearing for v3 — without it whetstone can't safely call `rtk init
--auto-patch`.

**Decision:**

- Keep `MIN_VERSION`.
- **Raise the floor to `0.42.0`** in Phase 2 (`src/rtk.rs:8`). Anything older
  predates the verified `--auto-patch` semantics whetstone v3 depends on.
- Continue to display `latest_remote_version()` as informational, not as gate.

Same pattern for `headroom` (`src/headroom.rs:8`): raise floor to **0.23.0**
(pinned for `wrap` semantics).

## 0.4 — Single source of truth for VERSION (design only)

**Today (v2.6.0) — `2.6.0` lives in these places:**

| Path                          | Form                              |
| ----------------------------- | --------------------------------- |
| `VERSION`                     | `2.6.0`                           |
| `Cargo.toml:3`                | `version = "2.6.0"`               |
| `site/src/Footer.jsx:14,36,65`| `v2.6.0`                          |
| `site/src/App.jsx:94,102`     | `v2.6.0`                          |
| `site/src/Hero.jsx:31,42`     | `v2.6.0`                          |
| `site/src/Nav.jsx:56`         | `v2.6.0`                          |
| `site/src/InstallTerminal.jsx:160,181` | `v2.6.0`                 |
| `site/src/CompressionDemo.jsx:16,81`   | `v2.6.0`                 |
| `site/src/Releases.jsx`       | hardcoded `2.2.2` (stale, plan §2.5) |
| `CHANGELOG.md`                | versioned headings                |

**Design — propagation contract (implementation lands in Phase 2.5):**

1. `VERSION` is the single canonical source.
2. `just release` (Phase 2.5):
   - reads `VERSION`,
   - rewrites `Cargo.toml`'s `version =` line,
   - prepends a stub entry to `CHANGELOG.md`,
   - regenerates `site/src/Releases.jsx` top entry (currently hardcoded `2.2.2`),
   - replaces every literal `v?<old-version>` token in `site/src/*.jsx` with
     `v?<new-version>` (limited to a known allowlist of paths above — no blind
     repo-wide sed).
3. CI verification (`just release-check`): scan repo for `v?<VERSION-mismatch>`
   tokens in the allowlist; fail if any disagree with `VERSION`.
4. Out of scope for the propagator: prose mentions of "SQuAD v2" benchmarks
   (`Hero.jsx:66`, `FAQ.jsx:48`) — that's a benchmark name, not a whetstone version.

No code change in Phase 0 — only the inventory + contract.

## 0.5 — Branch + RC gating

**Branch:** `v3` exists locally (cut from `v3-docs` HEAD, which carries the
execution kit in `docs/v3/`). This file is the first commit on the new branch.

**Pre-release tagging:**

- `VERSION` will bump to `3.0.0-rc.1` at the start of Phase 1.
- Cargo / SemVer pre-release suffix (`-rc.N`) keeps cargo-publish gated and
  makes the `latest_remote_version()` comparison naturally pin RC users to RC.

**RC gating mechanism for new flows (design only — implementation Phase 1+):**

- New top-level subcommands introduced in v3 (`whetstone doctor`, `whetstone
  migrate`) are unconditional — they don't need a flag.
- New behaviors that *replace* v2 behavior (delegating to `rtk init` / `icm
  init` instead of the in-tree hook scripts) live behind `--channel rc`
  during the RC window, falling back to v2 behavior otherwise.
- After `3.0.0` final, `--channel rc` becomes a no-op flag (kept for one minor
  for compatibility, then removed).

## Done-when checklist (this file is the deliverable)

- [x] `headroom wrap` answered with evidence (§0.1).
- [x] `rtk init` / `icm init` flags captured against pinned versions (§0.2).
- [x] ICM import verb identified: `icm import` (bulk) + `icm store` (per-memory) (§0.2).
- [x] RTK `MIN_VERSION` decision recorded (§0.3 — raise to 0.42.0 in Phase 2).
- [x] VERSION propagation inventory + contract drafted (§0.4).
- [x] `v3` branch exists (§0.5).

Phase 1 may now proceed.
