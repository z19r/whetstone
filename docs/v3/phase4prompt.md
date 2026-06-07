# Phase 4 Prompt — Update & Per-Project Refresh

## Context
whetstone v3 (see Phase 0). Fixes the gap where a new whetstone release ships fixes but existing projects never receive them. **Depends on Phase 1** (`whetstone.json` manifest, `integrations.rs`, `doctor`) and **Phase 2.4** (`--full` plumbed).

## House rules
- Rust 2021; `anyhow::Result` + `.context()`; `ui::fail()` for fatal; snake_case.
- Must pass `just release-check`. Add tests for the version-diff/refresh logic.

## Objective
Make `whetstone update` refresh per-project integration when the bundled integration-version is ahead of what the project recorded, and make `--full` force it.

## Tasks
- [ ] **4.1** In `src/update.rs`, after upgrading the global tools, compare the project's `whetstone.json` integration-version against the binary's bundled integration-version. If behind: re-run `rtk init` / `icm init` (via `integrations.rs`), re-apply the slash commands, run `whetstone doctor`, and update `whetstone.json`. `--full` forces this regardless of version.
- [ ] **4.2** Optionally re-run `headroom learn` during update so the CLAUDE.md learned-patterns block doesn't rot.
- [ ] **4.3** Extend the existing version cache (`VersionCache` in `update.rs`) to also track integration-version.

## Files likely touched
`src/update.rs`, `src/integrations.rs`, `src/doctor.rs`, `src/config.rs` (whetstone.json), `src/cli.rs`.

## Deliverable / Done when
Bumping the binary's bundled integration-version and running `whetstone update` in an existing project re-applies the tool inits and updates `whetstone.json`. `whetstone update --full` forces the refresh even when versions match.
