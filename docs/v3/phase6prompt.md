# Phase 6 Prompt — Installer & First-Run

## Context
whetstone v3 (see Phase 0). Fixes the install path and first-run UX. **Depends on Phase 0.1** (`headroom wrap` outcome) and **Phase 3** (migration hand-off).

## House rules
- Rust 2021 + POSIX shell for `install.sh`; `anyhow::Result` + `.context()`; `ui::fail()` for fatal; snake_case.
- Must pass `just release-check`. Test the curl-install on macOS and Linux.

## Objective
A clean curl-install ends in a working v3 with clear next-step messaging, missing prerequisites are handled gracefully, and installing over a v2 project triggers migration.

## Tasks
- [ ] **6.1** In `install.sh`, detect a missing `uv` and offer to install it (`curl -LsSf https://astral.sh/uv/install.sh | sh`) instead of letting `whetstone setup` abort on the preflight check.
- [ ] **6.2** Fix the wizard-via-pipe issue: after `curl | bash`, stdin is the exhausted pipe so the advertised TUI wizard is silently bypassed (you get the non-interactive ICM path). Either re-exec `whetstone setup` against `/dev/tty`, or print a clear "run `whetstone setup` for interactive configuration" message at the end of the one-liner.
- [ ] **6.3** Verify proxy-is-up-before-first-call ordering (tied to Phase 0.1). If `headroom wrap` does not auto-start the proxy, ensure the default `whetstone` command starts/awaits the proxy before exec'ing claude, so the first API call doesn't fail against a dead `ANTHROPIC_BASE_URL`.
- [ ] **6.4** If installing over a v2 project (markers from Phase 3.1 present), hand off to `whetstone migrate` rather than silently colliding two memory systems.

## Files likely touched
`install.sh`, `src/setup.rs`, `src/wizard.rs`, `src/wrapper.rs`, `src/preflight.rs`, `src/main.rs`.

## Deliverable / Done when
Clean curl-install on macOS + Linux ends in a working v3 with explicit next-step messaging; a missing `uv` is offered rather than fatal; the first claude call succeeds against a live proxy; installing over a v2 project triggers `whetstone migrate`.
