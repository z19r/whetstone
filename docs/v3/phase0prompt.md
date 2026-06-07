# Phase 0 Prompt — Foundations & External Verification

## Context
You are working on **whetstone**, a Rust CLI (`whetstone-cli`) that installs and configures token-optimization tooling — Headroom (context-compression proxy), RTK (CLI-output hook), and ICM (local memory) — for Claude Code. We are executing a breaking **v3.0.0** refactor: whetstone becomes a thin orchestrator that delegates integration to each tool's own `init` command, drops AutoMem and the MemStack asset layer, adds a v2→v3 migration layer, and fixes confirmed bugs.

This prompt covers **Phase 0**, which must complete before Phases 1, 3, and 6 because the whole orchestrator depends on other tools' interfaces. **Do not write feature code in this phase** — this is verification and setup.

## House rules (apply to every phase)
- Rust 2021; `anyhow::Result` with `.context()`; `ui::fail()` for fatal errors; snake_case.
- All changes must pass `just release-check` (cargo fmt --check, clippy `-D warnings`, cargo test).
- Never hand-write Claude Code hooks — delegate to the tools' own init.
- Never ship telemetry or third-party webhooks.

## Objective
Produce a verified "interface contract" the rest of v3 builds on, and stand up the v3 branch.

## Tasks
- [ ] **0.1 (⚠️ blocker for default command)** Determine whether `headroom wrap <tool>` exists in the pinned Headroom version. If it does not, design a fallback: start `headroom proxy` (or use the service) then exec the underlying tool. Record the decision.
- [ ] **0.2 (🔴 blocker for migration)** Capture exact current flags for `rtk init` and `icm init` against pinned versions. Critically, identify ICM's actual verb for adding/importing a memory (e.g. `icm remember`, `icm import <file>`, or per-concept calls). Pin tested `rtk` / `icm` / `headroom` versions in CI.
- [ ] **0.3** Read RTK's current released version (~0.28.x). Decide: set a real `MIN_VERSION` floor, or remove the floor and rely on remote-latest comparison only.
- [ ] **0.4** Establish `VERSION` as the single source of truth; design how it will propagate to `CHANGELOG.md`, `site/src/Releases.jsx`, and any hardcoded `v2.x` strings (implemented in Phase 2).
- [ ] **0.5** Cut a `v3` branch; gate new flows behind a prerelease tag / `--channel rc`.

## Deliverable / Done when
- A one-page `docs/interface-contract.md` recording the verified `rtk` / `icm` / `headroom` commands and pinned versions v3 targets.
- The `headroom wrap` question (0.1) and ICM import verb (0.2) are answered with evidence, not assumption.
- `v3` branch exists.
