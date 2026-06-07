# Phase 5 Prompt — Asset Cleanup & Content

## Context
whetstone v3 (see Phase 0). Removes the broken/inappropriate bundled MemStack asset layer and replaces it with a minimal, relevant set. Low-risk; can run any time after Phase 1. Coordinate with **Phase 3.5** so the migration's cleanup logic and the shipped asset tree agree on what is "managed."

## House rules
- Rust 2021; `anyhow::Result` + `.context()`; `ui::fail()` for fatal; snake_case.
- Must pass `just release-check`.
- The shipped asset tree must contain **no third-party webhooks, no telemetry, no dangling skill references, and nothing unrelated to token optimization.**

## Objective
Strip whetstone down to assets that actually serve token optimization and are safe to distribute under MIT.

## Tasks
- [ ] **5.1** Delete the 20 MemStack skills, the 8 rules, `pro-skills.md` (its ~40 referenced skill files don't exist), `kdp-format`, and the consultancy skills (`humanize`, `quill`, `scan`, `governor`, `consolidate`).
- [ ] **5.2** Delete the n8n webhook from `diary.md` and the `cc_monitor` telemetry code from the (now-removed) session hooks. This resolves the "NO TELEMETRY" contradiction with the marketing footer.
- [ ] **5.3** Ship at most two slash commands that call real binaries: `/whetstone-headroom` (proxy stats via the Headroom health/stats endpoint) and `/whetstone-status` (a `whetstone doctor` summary).
- [ ] **5.4** Remove the `ecc-tools` auto-generated `whetstone` SKILL.md from anything shipped (it asserts camelCase filenames and `*.test.rs` / `__tests__` for a Rust repo, both wrong). If kept for local dev only, make the generator language-aware — but it must not ship.
- [ ] **5.5** Reduce to a single canonical DB-path constant (now only the migration reader cares about it).

## Files likely touched
`assets/skills/*` (delete most), `assets/rules/*` (delete), `assets/commands/*` (replace with the two new ones), `assets/MEMSTACK.md` (remove or replace), `src/setup.rs` / `src/db.rs` (DB path constant), `.claude/` ecc-tools artifacts (stop shipping).

## Deliverable / Done when
The shipped asset tree has no webhook, no telemetry, no broken skill catalog, and no consultancy/book-formatting skills — only the two slash commands and whatever minimal, relevant content remains. `MANAGED_SKILLS`/`MANAGED_RULES` (Phase 3.5) match what ships.
