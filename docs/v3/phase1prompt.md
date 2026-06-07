# Phase 1 Prompt — Core Refactor to Thin Orchestrator

## Context
whetstone v3 (see Phase 0). This phase replaces whetstone's hand-rolled Claude Code integration with delegation to each tool's own `init`, and collapses the memory providers. **Depends on Phase 0** (verified `rtk init` / `icm init` flags, `headroom wrap` decision).

## House rules
- Rust 2021; `anyhow::Result` + `.context()`; `ui::fail()` for fatal; snake_case.
- Must pass `just release-check`. Add/extend unit tests for every new module.
- whetstone must end this phase with **zero whetstone-authored hooks** written into settings.json.

## Objective
Turn whetstone into a thin orchestrator: install tools, run their inits, normalize the result, and track state in a versioned manifest.

## Tasks
- [ ] **1.1** New `src/integrations.rs`: functions that shell out to `rtk init` and `icm init` (using the flags verified in Phase 0), capturing and normalizing output. This replaces `hooks.rs::build_hooks_value` and `copy_hook_scripts`.
- [ ] **1.2** New `src/doctor.rs` + a `whetstone doctor` subcommand. After the tool inits, it reads `~/.claude/settings.json` and:
  - Normalizes hook ordering so RTK's PreToolUse `Bash` hook is **last**.
  - Confirms ICM hooks are present and well-formed.
  - Reports anything off. Repurpose the old `entry_is_whetstone_managed` logic from "replace" to "inspect/normalize."
- [ ] **1.3** Collapse `MemoryProvider` in `src/memory.rs` to `{ Icm, Skip }`. Delete `install_automem` (setup.rs), the `mcpServers.memory` AutoMem branch in `hooks.rs::build_hooks_value`, and AutoMem detection. Update `src/wizard.rs` / `src/setup.rs` prompts.
- [ ] **1.4** Delete the five whetstone hook scripts from `assets/hooks/` and stop copying them (`hooks.rs::copy_hook_scripts`). Move proxy auto-start to `headroom wrap` (if Phase 0.1 confirmed it) or the systemd/launchd service. Add an optional `whetstone proxy service install` helper.
- [ ] **1.5** Introduce a versioned `whetstone.json` project manifest replacing `config.local.json` (rework `src/config.rs`). Fields: whetstone version, provider, integration-version, optional migration id.

## Files likely touched
`src/integrations.rs` (new), `src/doctor.rs` (new), `src/hooks.rs` (gut/delete), `src/memory.rs`, `src/setup.rs`, `src/wizard.rs`, `src/config.rs`, `src/cli.rs` (add `doctor`, optional `proxy service`), `src/main.rs` (dispatch), `assets/hooks/*` (remove).

## Deliverable / Done when
A fresh `whetstone setup` on a clean machine produces a working v3 install where RTK and ICM are wired by their own inits, the proxy runs, `whetstone doctor` reports green, and there are zero whetstone-authored hooks in settings.json.
