# Phase 2 Prompt — Confirmed Bug Fixes

## Context
whetstone v3 (see Phase 0). These are verified defects, independent enough to run alongside Phase 1. **Depends on Phase 0** (RTK version floor decision, VERSION propagation design).

## House rules
- Rust 2021; `anyhow::Result` + `.context()`; `ui::fail()` for fatal; snake_case.
- Must pass `just release-check`. One regression test per fix.

## Objective
Eliminate the confirmed bugs so v3 has no hardcoded model strings, no fictional version floors, no dead flags, and no version drift.

## Tasks
- [ ] **2.1 Model hardcoding** — In `src/wrapper.rs`, remove the default `--model` injection so Claude Code's own settings choose the model. Drop the hardcoded `LATEST_MODEL` upgrade prompt, or make "latest" config/remote-driven. (Hardcoded `claude-opus-4-7` is already stale, which is the point.)
- [ ] **2.2 RTK MIN_VERSION** — In `src/rtk.rs`, set `MIN_VERSION` to a real floor (per Phase 0.3) or remove the floor and rely on remote-latest comparison. Current value (`0.39.0`) is above any released RTK and makes `install()` re-install every run.
- [ ] **2.3 ICM init mode** — In `src/setup.rs::run_icm_init`, replace the invalid `--mode standard` with the current ICM default (`icm init`, no flag) or `--mode all`, per Phase 0.2.
- [ ] **2.4 `update --full`** — In `src/update.rs`, `run(_full: bool)` currently ignores the flag. Plumb `full` through now; the full per-project refresh behavior lands in Phase 4. Do not leave `_full` unused.
- [ ] **2.5 Version drift** — Make `just release` (and/or `src/release.rs`) regenerate the top `CHANGELOG.md` entry and the `site/src/Releases.jsx` feed data from `VERSION`. Remove hardcoded `2.2.2` in `Releases.jsx` and `InstallTerminal.jsx`.
- [ ] **2.6 Stdin hook contract** — Confirm the broken `$CLAUDE_TOOL_INPUT` hooks are gone (deleted in Phase 1). `whetstone doctor` (Phase 1) validates the surviving rtk/icm hooks are well-formed. No env-var-based hook gating may remain.

## Files likely touched
`src/wrapper.rs`, `src/rtk.rs`, `src/setup.rs`, `src/update.rs`, `src/release.rs`, `justfile`, `site/src/Releases.jsx`, `site/src/InstallTerminal.jsx`, `CHANGELOG.md`.

## Deliverable / Done when
`whetstone version` and the marketing site agree on the version; no hardcoded model string or fictional version floor remains anywhere; `update --full` is wired (even if its full effect is Phase 4); each fix has a regression test.
