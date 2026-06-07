# Phase 7 Prompt — Honesty Pass (Site + Docs)

## Context
whetstone v3 (see Phase 0). Aligns the marketing site and docs with reality and with the v3 architecture. Best done after Phases 1–6 land so docs describe shipped behavior.

## House rules
- Every published statistic must be traceable to a cited source or your own reproducible benchmark run.
- Docs describe **only** v3 commands.
- Site build stays static/self-contained (no build step), consistent with the existing `site/` setup.

## Objective
Make every claim defensible, reposition the pitch around what whetstone actually contributes, and document the v3 commands + migration.

## Tasks
- [ ] **7.1** Replace the unverifiable "97% accuracy at 19% tokens on SQuAD v2" with either a cited Headroom benchmark (Headroom's own published figures are ~95%+ accuracy preservation at 40–90% token reduction) or numbers from your own `python -m headroom.evals benchmark` run. Cite whichever you use.
- [ ] **7.2** Add the RTK net-cost caveat: compression can increase output tokens when it strips info the model needed (RTK's own tracker documents a ~18% cost-increase case), and the hook only fires on Bash tool calls — not Claude Code's native Read/Grep/Glob. Add a "run `rtk gain` / `rtk discover`, consider audit mode" note.
- [ ] **7.3** Reposition the pitch around what whetstone uniquely provides — single Rust binary, idempotent setup, orchestration, the TUI/version dashboard, release automation — not compression numbers that belong to Headroom and RTK.
- [ ] **7.4** Rewrite the editors matrix: Memory = ICM via `icm init`; remove the AutoMem row. Update all of `docs/*` for v3 commands. Add a standalone **Migration Guide** (v2→v3) covering `migrate`, `--dry-run`, and `--rollback`.
- [ ] **7.5** Repo housekeeping: stop tracking/shipping dev-env config (`.claude/` ecc-tooling, `.serena/`); reconcile the `/.claude` entry in `.gitignore` with what is actually committed.

## Files likely touched
`site/src/Hero.jsx`, `site/src/Numbers.jsx`, `site/src/CompressionDemo.jsx`, `site/src/Editors.jsx`, `site/src/FAQ.jsx`, `site/src/Modules.jsx`, `docs/*.md`, new `docs/migration.md`, `README.md`, `.gitignore`.

## Deliverable / Done when
No published stat is unverifiable; the RTK tradeoff is stated honestly; docs describe only v3 commands; a standalone migration guide exists; dev-env config is no longer shipped.
