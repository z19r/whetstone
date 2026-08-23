# Changelog

All notable changes to whetstone will be documented in this file.

## [Unreleased]

## [3.12.0] - 2026-08-23

### Added

- add install-tools command and doctor startup/extras verification (#89)
- add HEADROOM_BEACON opt-out toggle to settings (#87)

### Added

- `whetstone install-tools` — install or repair every managed dependency
  (headroom, rtk, claude code, memory provider), re-run their `init` hooks,
  and resync the project manifest; `--force` reinstalls everything
- `whetstone doctor` now checks that the managed dependencies still exist
  before inspecting `~/.claude/settings.json`, offers to reinstall the missing
  ones (`--fix` skips the prompt), and reports missing system prerequisites
- launching a managed tool that was uninstalled now offers to reinstall it
  instead of failing with a bare `exec` error

- `whetstone doctor` now verifies that headroom actually **starts**: it probes
  the running proxy, or spawns a throwaway one on a free port with the same
  args and env whetstone launches with, and reports headroom's own startup
  error when it dies. A start blocked by a rollout-gated flag (e.g.
  `read_maturation` in `~/.headroom/settings.json`) is offered for removal —
  `--fix` removes it, with a backup, and re-checks

### Fixed

- whetstone now verifies headroom's *extras*, not just its version: an install
  recorded by uv without `proxy`/`code`/`mcp` is reported by `doctor` and
  reinstalled as `headroom-ai[proxy,code,mcp]` instead of passing as healthy

## [3.11.0] - 2026-08-20

### Added

- add HEADROOM_BEACON opt-out toggle to settings (#87)

## [3.10.1] - 2026-08-10

### Added

- add configurable Claude Code edit mode setting (#81)

### Fixed

- actually check and upgrade ICM in whetstone update (#85)
- pin Headroom memory to a global root, stop cross-project litter (#83)
- stop passing --no-rtk to headroom wrap (#79)
- stop v3 slash commands from triggering false v2 migration (#78)

## [3.10.0] - 2026-08-10

### Added

- add configurable Claude Code edit mode setting (#81)

### Fixed

- pin Headroom memory to a global root, stop cross-project litter (#83)
- stop passing --no-rtk to headroom wrap (#79)
- stop v3 slash commands from triggering false v2 migration (#78)

## [3.9.0] - 2026-08-06

### Added

- add configurable Claude Code edit mode setting (#81)

### Fixed

- stop passing --no-rtk to headroom wrap (#79)
- stop v3 slash commands from triggering false v2 migration (#78)

## [3.8.1] - 2026-08-06

### Fixed

- stop passing --no-rtk to headroom wrap (#79)
- stop v3 slash commands from triggering false v2 migration (#78)

## [3.8.0] - 2026-08-04

### Added

- add curated HEADROOM_* knobs to settings TUI (#75)
- headroom_env launch passthrough + 80-char reflow (#74)
- launch-time model-update prompt (#72)
- scope uninstall into project/global; add Anthropic API URL setting (#71)

### Changed

- add superpowers planning and spec for model-update-prompt (#73)

### Added

- Launch-time model-update prompt: when Anthropic ships a model newer than the one this project runs — or a brand-new model family — `whetstone` shows a full-screen modal offering to pin it as the project default, use it for one session only, or dismiss it. Dismissals are remembered per-project (`dismissed_models` in `.claude/whetstone.json`). The prompt is skipped when non-interactive, offline, not a v3 project, or when `--model` is passed explicitly.
- Anthropic API URL setting in `whetstone settings` — a custom upstream Anthropic API URL for the Headroom proxy, exported as `ANTHROPIC_TARGET_API_URL` before launch (project- or global-scoped; an externally-set env var takes precedence)
- Opinionated Headroom launch defaults with full override control: whetstone
  now sets `HEADROOM_CODE_AWARE_ENABLED=1` (and suppresses Headroom's own
  memory tools when ICM is the provider) alongside the existing `agent-90`
  savings profile. Every Headroom knob is overridable via a `headroom_env`
  map in `.claude/whetstone.json` or `~/.whetstone/settings.json`
  (project-over-global), with external `HEADROOM_*` env vars winning over
  both. `HEADROOM_PORT` stays whetstone-owned.

### Changed

- Split `whetstone uninstall` into `whetstone project uninstall` (removes per-project files) and `whetstone global uninstall` (removes the whetstone binary, RTK, and Headroom). The top-level `uninstall` command is deprecated: it now removes nothing and prints guidance toward the two scoped commands.

## [3.7.0] - 2026-07-02

### Added

- add --memory flag and Headroom Memory setting (#67)

### Fixed

- re-sync headroom MCP on update; prefer newest Sonnet as default model (#68)

## [3.6.2] - 2026-06-28

### Added

- integrate self-update and migration into setup command (#62)
- settings TUI with global/project layering (#57)

### Fixed

- store changelog regex patterns in variables for bash compatibility

### Changed

- update the claude models (#64)

## [3.6.1] - 2026-06-28

### Added

- integrate self-update and migration into setup command (#62)
- settings TUI with global/project layering (#57)

### Fixed

- store changelog regex patterns in variables for bash compatibility

### Changed

- update the claude models (#64)

## [3.6.0] - 2026-06-28

### Added

- integrate self-update and migration into setup command (#62)
- settings TUI with global/project layering (#57)

### Fixed

- store changelog regex patterns in variables for bash compatibility

## [3.5.2] - 2026-06-23

### Added

- settings TUI with global/project layering (#57)

### Fixed

- store changelog regex patterns in variables for bash compatibility

## [3.5.1] - 2026-06-15

### Added

- settings TUI with global/project layering

## [3.5.0] - 2026-06-15

### Added

- prompt to run setup in unconfigured projects

### Fixed

- pass --no-proxy so headroom wrap never hot-restarts the proxy
- start headroom proxy with HEADROOM_SAVINGS_PROFILE

## [3.4.0] - 2026-06-12

### Fixed

- conditional --savings-profile and installMethod mismatch fix

## [3.3.0] - 2026-06-11

### Changed

- version bump (no user-facing changes)

## [3.2.0] - 2026-06-11

### Added

- detect and fix stale headroom pip installs shadowing uv-managed binary
- detect and fix stale Claude Code native binary installs

### Fixed

- clause update logic

## [3.1.3] - 2026-06-09

### Fixed

- correct MIN_VERSION grep patterns in metadata generation

## [3.1.2] - 2026-06-09

### Added

- metadata-driven dynamic site content

## [3.1.1] - 2026-06-08

### Added

- ratatui inline viewport for wizard, pin default model

## [3.1.0] - 2026-06-08

### Added

- site changelog driven from CHANGELOG.md
- drive Releases section from CHANGELOG.md

## [3.0.0] - 2026-06-08

### BREAKING — read this before upgrading from v2

v3 is a structural rewrite. **`whetstone setup` will refuse to silently install
over a v2 project** and hand off to `whetstone migrate` instead. Migrating is
one command, archive-backed, and reversible. See the
[Migration Guide](docs/migration.md).

- **AutoMem provider removed.** `MemoryProvider` is now `{ Icm, Skip }`. The
  `mcpServers.memory` block is torn out of `~/.claude/settings.json` (backed up
  in the migration archive) and the external FalkorDB + Qdrant service, if
  one was running, is no longer in whetstone's blast radius — tear it down
  yourself.
- **Whetstone no longer bundles skills, rules, or hook scripts.** The five
  hand-rolled `assets/hooks/*.sh` scripts, the 20-skill / 8-rule
  `assets/skills/` + `assets/rules/` trees, the `MEMSTACK.md` shim, and the
  `whetstone`-managed entries in `~/.claude/settings.json` are all gone. ICM
  owns its own assets; whetstone delegates to `icm init --mode standard`.
- **Hooks are tool-managed.** `~/.claude/settings.json` is no longer
  hand-merged by whetstone. `rtk init --auto-patch` and `icm init` write
  their own hook entries; `whetstone doctor` inspects ordering and reports
  drift.
- **Migration is required.** Existing v2 installs (detected by
  `.claude/memstack/`, the v2 hook scripts, the AutoMem MCP block, or the
  v2-flavoured skills/rules layout) must run `whetstone migrate` before any
  v3 command will configure them. `whetstone setup` auto-detects v2 and
  routes to `migrate`. `--rollback <id>` restores the v2 state byte-for-byte
  (except the external AutoMem service).
- **`config.local.json` removed.** Replaced by `.claude/whetstone.json`
  (schema version, integration version, provider, tool versions, timestamps).
- **Hardcoded `--model` injection removed.** `whetstone claude` no longer
  forces a specific model; Claude Code's own settings choose it.

### Added

- **`whetstone migrate`** — staged, reversible v2 → v3 migration with
  `--dry-run`, `-y`, and `--rollback <id>`. Archives backups under
  `.whetstone/migration-<id>/`. See `docs/migration.md`.
- **`whetstone doctor`** — inspects installed tool versions,
  `~/.claude/settings.json` hooks, and the per-project manifest.
- **`whetstone dashboard`** — TUI for installed tool versions vs. pinned
  floors.
- **`whetstone stats`** — token-savings summary from RTK + Headroom stats
  endpoints.
- **`whetstone setup` auto-detects v2** and hands off to `migrate` in both
  the wizard and headless paths.
- **`install.sh` re-execs `whetstone setup` against `/dev/tty`** so the TUI
  wizard actually runs under `curl | bash`; offers to install `uv` instead
  of aborting the preflight.
- **`whetstone` (default cmd) waits for the proxy before exec'ing claude**
  so the first API call doesn't fire against a dead `ANTHROPIC_BASE_URL`.

### Changed

- **v3 phase 1 — thin orchestrator**: whetstone no longer hand-writes
  `~/.claude/settings.json` hooks. Setup delegates to `rtk init --auto-patch`
  and `icm init --mode standard`. New `whetstone doctor` subcommand inspects
  hook ordering and reports drift.
- **`whetstone.json` manifest**: replaces `config.local.json`. Records
  schema version, integration version, provider, tool versions, and
  timestamps. Lives at `.claude/whetstone.json`.
- **MemoryProvider** collapsed to `{ Icm, Skip }` — AutoMem install path,
  `preflight::check_npm`, and AutoMem detection removed.

### Removed

- All five `assets/hooks/*.sh` scripts and `src/hooks.rs` — the tools own
  their own Claude Code hooks now.
- Hardcoded `--model` injection in `wrapper.rs`. Claude Code's own
  settings choose the model. Model-upgrade prompt + dismissal-file
  machinery removed.

### Fixed

- **RTK `MIN_VERSION`**: was `0.39.0`, which never shipped — every
  `install()` re-installed. Raised to `0.42.0` (interface-contract pin).
- **`whetstone update --full`**: flag was previously ignored. Now forces
  a refresh of `rtk` and `headroom` even when up-to-date.
- **Version drift**: the marketing site reads `WHETSTONE_VERSION` from
  `site/src/version.js`, regenerated by `just release` from the repo-root
  `VERSION` file. No more hardcoded `2.2.2` in `Releases.jsx` /
  `InstallTerminal.jsx`.

## [2.3.2] - 2025-05-26

### Fixed

- Release workflow: give verify-release job explicit repo context so `gh release` commands work without a local git checkout.

## [2.3.1] - 2025-05-26

### Fixed

- Justfile release recipe corrections.

## [2.3.0] - 2025-05-25

### Added

- `whetstone version` command showing component versions with outdated indicators.
- TUI setup wizard.
- Suppressed noisy installer output during `whetstone update`.
