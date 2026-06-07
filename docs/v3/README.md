# Whetstone v3 — Execution Kit

This folder is everything you need to drive the whetstone **v3.0.0** refactor through Claude Code: a master plan with checkboxes, and one self-contained prompt per phase.

## What's in here

| File | Purpose |
|------|---------|
| `WHETSTONE_V3_PLAN.md` | Master tracker. Every task as a checkbox. **Tick boxes here as you go.** |
| `phase0prompt.md` | Foundations & external verification (🔴 do first) |
| `phase1prompt.md` | Core refactor to thin orchestrator |
| `phase2prompt.md` | Confirmed bug fixes |
| `phase3prompt.md` | Migration layer (`whetstone migrate`) — the part with real data at stake |
| `phase4prompt.md` | Update & per-project refresh |
| `phase5prompt.md` | Asset cleanup & content |
| `phase6prompt.md` | Installer & first-run |
| `phase7prompt.md` | Honesty pass (site + docs) |
| `phase8prompt.md` | Testing, release, rollout |

> Numbering matches the plan exactly. `phase0prompt.md` is the foundational phase; there's no off-by-one against the tracker.

## How to run this in Claude Code

1. Drop this whole folder into the whetstone repo (e.g. `docs/v3/`) and commit it, so Claude Code has the plan and prompts in-repo as context.
2. Start on the `v3` branch (created in Phase 0).
3. For each phase, in order:
   - Open a Claude Code session in the repo root.
   - Paste the phase prompt, or tell Claude: `Read docs/v3/phase1prompt.md and implement it.`
   - Let it work, review the diff, run `just release-check`.
   - **Check off the matching boxes in `WHETSTONE_V3_PLAN.md`** and commit (the plan file is your source of truth for progress).
4. Don't batch phases into one session — each prompt is sized to be a focused unit of work with its own acceptance criteria.

## Order & dependencies

```
0  ──┬─→ 1 ──┬─→ 3 ──→ 6 ──┐
     │       │            ├─→ 7 ──→ 8
     └─→ 2 ──┴─→ 4        │
                  5 ──────┘   (5 is low-risk; slot anytime after 1)
```

- **Phase 0 gates everything.** Two items are hard blockers:
  - **0.1** Does `headroom wrap <tool>` exist in your pinned Headroom version? The default `whetstone` command depends on the answer.
  - **0.2** What is ICM's actual verb for importing a memory? The Phase 3 migration importer cannot be finalized without it.
  - Write both answers into `docs/interface-contract.md` — later prompts assume those facts exist.
- **Phase 3 depends on Phase 1** (it reuses the orchestrator + `whetstone doctor` to re-init) and on **0.2**.
- **Phases 1 and 2** can run roughly in parallel.
- **Phase 5** is low-risk cleanup; do it any time after Phase 1, but keep its `MANAGED_SKILLS`/`MANAGED_RULES` lists in sync with Phase 3.5.
- **Phases 7–8** come last so docs describe shipped behavior and the release candidate covers real code.

## Non-negotiables (baked into every phase prompt)

- Rust 2021; `anyhow::Result` + `.context()`; `ui::fail()` for fatal errors; snake_case.
- Every change passes `just release-check` (fmt + clippy `-D warnings` + test).
- **Never** hand-write Claude Code hooks — delegate to `rtk init` / `icm init`.
- **Never** ship telemetry or third-party webhooks (this is what removes the n8n webhook and the `cc_monitor` reporting).
- Migration never deletes user data — it renames/backs up, and supports `--dry-run` and `--rollback`.

## Milestones

- **M1** — fresh v3 install works
- **M2** — all confirmed bugs fixed + version single-source-of-truth
- **M3** — `migrate` dry-run / full / rollback green on the fixture
- **M4** — update-refresh + installer + cleanup done
- **M5** — docs/site honest, rc shipped, `3.0.0` released

## One caution before you start

The single thing that can quietly go wrong is **Phase 3.4 (MemStack → ICM import)** if Phase 0.2 is guessed instead of verified. Confirm ICM's import path against the installed binary, and keep the migration idempotent (tag every record with `migration-id`, write a sentinel into the renamed backup) so a re-run never duplicates memories.
