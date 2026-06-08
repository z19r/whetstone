# Whetstone v3 — Implementation Plan

> Breaking release (v3.0.0). Converts whetstone into a thin orchestrator over Headroom / RTK / ICM,
> removes AutoMem and the MemStack asset layer, adds a v2→v3 migration layer, and fixes all confirmed bugs.
> Ship `3.0.0-rc.1` first, dogfood, then `3.0.0`.

**Legend:** `[ ]` todo · `[~]` in progress · `[x]` done · 🔴 blocker · ⚠️ verify-before-relying

---

## Scope decisions — LOCK THESE FIRST

- [x] Confirm: memory providers collapse to `Icm | Skip` (AutoMem removed entirely)
- [x] Confirm: whetstone becomes a thin orchestrator — delegates integration to `rtk init` / `icm init`, stops hand-writing hooks
- [x] Confirm: remove the 20-skill MemStack bundle, 8 rules, `pro-skills.md` catalog, and the `whetstone db` CLI command (keep the memstack.db *reader* internally for migration only)
- [x] Confirm: ICM owns memory (its own hooks + skills + CLAUDE.md via `icm init`)
- [x] Confirm: this is a breaking change → `v3.0.0`, shipped via an `rc` first
- [x] Calibration noted: v2 wired AutoMem with no endpoint/key + no backend, so AutoMem "migration" is mostly dead-config removal; the real data path is `memstack.db` → ICM

---

## Phase 0 — Foundations & external verification

- [x] **0.1** ⚠️ Verify `headroom wrap` exists in the pinned Headroom version
  - [ ] If it does NOT exist: design fallback (start proxy + exec claude) for default `whetstone` command and `headroom wrap aider/codex` in docs
- [x] **0.2** Capture exact current flags for `rtk init` and `icm init` against pinned versions
  - [x] 🔴 Identify ICM's actual verb for adding/importing a memory (`icm remember` vs `icm import <file>` vs per-concept calls) — gates the migration importer
  - [x] Pin tested `rtk` / `icm` / `headroom` versions in CI
- [x] **0.3** Read RTK's actual current version (~0.28.x); set a real `MIN_VERSION` floor or remove the floor
- [x] **0.4** Make `VERSION` the single source of truth; design propagation to CHANGELOG + `site/src/Releases.jsx` + hardcoded `v2.x` strings
- [x] **0.5** Cut a `v3` branch; gate new flows behind `--channel rc` / prerelease tag
- [x] **Acceptance:** one-page "interface contract" doc recording verified `rtk`/`icm`/`headroom` commands + pinned versions

---

## Phase 1 — Core refactor to thin orchestrator

- [x] **1.1** New `integrations.rs` — shell out to `rtk init` and `icm init`, capture/normalize output (replaces `hooks.rs::build_hooks_value` + `copy_hook_scripts`)
- [x] **1.2** New `doctor.rs` / `whetstone doctor` — after tool inits, read `~/.claude/settings.json` and:
  - [x] Normalize hook ordering so RTK's PreToolUse Bash hook is **last**
  - [x] Confirm ICM hooks present + well-formed
  - [x] Report anything off (repurpose old `entry_is_whetstone_managed` from "replace" → "inspect/normalize")
- [x] **1.3** Collapse `MemoryProvider` to `{ Icm, Skip }`
  - [x] Delete `install_automem`
  - [x] Delete the `mcpServers.memory` AutoMem branch in `build_hooks_value` (entire `hooks.rs` deleted)
  - [x] Delete AutoMem detection
- [x] **1.4** Delete the five whetstone hook scripts from `assets/hooks/`; stop copying them
  - [x] Move proxy auto-start to `headroom wrap` (confirmed in 0.1)
  - [ ] Add optional `whetstone proxy service install` helper (deferred — `headroom install` already provides this)
- [x] **1.5** New versioned `whetstone.json` project manifest (replaces `config.local.json`) recording: whetstone version, provider, integration-version, migration id
- [x] **Acceptance:** fresh `whetstone setup` on clean machine → working v3 (RTK + ICM wired by their own inits, proxy running, `whetstone doctor` green, **zero** whetstone-authored hooks in settings.json)

---

## Phase 2 — Confirmed bug fixes

- [x] **2.1** Remove default `--model` injection in `wrapper.rs`; let Claude Code settings choose
  - [x] Drop hardcoded `LATEST_MODEL` upgrade prompt (prompt + dismissal-file machinery deleted)
- [x] **2.2** Set RTK `MIN_VERSION` to a real floor or remove it (per 0.3) — raised to `0.42.0` (interface-contract pin)
- [x] **2.3** Replace invalid `icm init --mode standard` with current default — contract §0.2 verified `standard` IS the default; regression test locks the invocation
- [x] **2.4** Wire `update --full` — `dependency_decision` helper now honours the flag and forces a refresh of `rtk` / `headroom` even when up-to-date. Per-project asset refresh deferred to Phase 4.
- [x] **2.5** `just release` regenerates CHANGELOG top entry + `site/src/version.js` from `VERSION`. Hardcoded `2.2.2` removed from `Releases.jsx` / `InstallTerminal.jsx`. New `version::tests::cargo_pkg_version_matches_version_file` pins Cargo ↔ VERSION.
- [x] **2.6** Stdin hook contract resolved by deletion in Phase 1. New `doctor::tests::no_source_references_claude_tool_input_env_var` audits `src/` and `assets/` so the env-var-gating pattern can't silently reappear.
- [x] **Acceptance:** one regression test per fix; `release-check` green (47 tests); `whetstone version` and site agree; no hardcoded model or fictional version floor remains

---

## Phase 3 — Migration layer (`whetstone migrate`)

> Staged, reversible, idempotent. Mirror the `release` preconditions discipline.

### 3.1 Detect (read-only, no writes)
- [x] Detect `.claude/db/memstack.db` (MemStack data)
- [x] Detect v2 whetstone-authored hooks in `~/.claude/settings.json` (`entry_is_whetstone_managed` in `src/migrate.rs`)
- [x] Detect `mcpServers.memory` → `@verygoodplugins/mcp-automem`; check for `AUTOMEM_ENDPOINT` / `AUTOMEM_API_KEY`
- [x] Detect managed `.claude/skills/`, `.claude/rules/`, `.claude/commands/`, `MEMSTACK.md`, `config.local.json`
- [x] Emit a detection report (`Detection::render`)

### 3.2 Backup + export archive (`.whetstone/migration-<ts>/`)
- [x] Timestamped `settings.json` backup
- [x] `memstack.db.v2bak` (rename, never delete original)
- [x] `memstack-export.md` + `memstack-export.jsonl` (sessions / insights / context / plans)
- [x] `automem-export.jsonl` if reachable

### 3.3 AutoMem teardown
- [x] If endpoint+key exist and service responds: best-effort pull memories via recall API into export; else skip with clear note
- [x] Remove `mcpServers.memory` entry (backed up to `automem-mcp-entry.json`)
- [x] Do NOT tear down the user's external Railway/Docker service — print decommission instructions instead

### 3.4 MemStack → ICM migration (the real-data path)
- [x] Ensure ICM installed (warn + skip if missing)
- [x] Read memstack.db via internalized reader; map records:
  - [x] `insights` → ICM memories tagged by project; importance from `type` (architecture → critical; decision → high; pattern/tool/bug-fix → normal)
  - [x] `sessions` → one ICM memory each (accomplished/decisions/next_steps), tagged project + date
  - [x] `project_context` (architecture_decisions/known_issues/backlog) → ICM memories
  - [x] `plans` → markdown export only (not imported)
- [x] Use ICM's verified import path (from 0.2): `icm import --format auto` bulk; per-record `icm store` fallback
- [x] Idempotency: tag every record (`source=whetstone-migration`, `migration-id=<ts>`); sentinel `.whetstone-migration-completed` + `whetstone.json::migration_id` prevent duplicate re-runs

### 3.5 Cleanup of v2 managed files
- [x] Build `MANAGED_SKILLS` / `MANAGED_RULES` / `MANAGED_COMMANDS` / `MANAGED_HOOK_SCRIPTS` manifests — remove only whetstone's own, preserve user-authored
- [x] Remove v2 hooks from settings.json
- [x] Remove `~/.claude/hooks/*.sh` whetstone scripts (if unreferenced)
- [x] Remove managed project assets + `MEMSTACK.md` + `config.local.json`

### 3.6 Re-init the v3 way
- [x] Run `rtk init` (via `integrations::run_all`)
- [x] Run `icm init` (via `integrations::run_all`)
- [x] Run `whetstone doctor` to normalize
- [x] Write new `whetstone.json` recording the migration id

### 3.7 Flags
- [x] `--dry-run` (full plan, no changes — mirror `release-dry-run`)
- [x] `--yes` (non-interactive)
- [x] `--rollback <migration-id>` (restore settings.json + memstack.db + removed files; AutoMem service not restored, only its config re-added)

### 3.8 Auto-detect hand-off
- [x] `whetstone setup` / `update` detect v2 markers and offer to run `migrate` (early return if migration runs)

- [x] **Acceptance:** on a fixture v2 project (settings.json w/ v2 hooks + seeded memstack.db + AutoMem mcpServers entry):
  - [x] `migrate --dry-run` reports exact plan (smoke-tested against current repo)
  - [x] `migrate` → clean v3 state, ICM holds migrated memories, no duplicate on re-run (sentinel + manifest gate)
  - [x] `migrate --rollback` restores v2 state byte-for-byte (except external AutoMem service)

---

## Phase 4 — Update & per-project refresh

- [x] **4.1** `whetstone update`: after upgrading global tools, compare `whetstone.json` integration-version vs binary's bundled version — `refresh_project_integration` in `src/update.rs` loads `WhetstoneManifest`, runs `project_refresh_decision`, and short-circuits when at-or-ahead
  - [x] If behind: re-run `rtk init` / `icm init` (via `integrations::run_all`), re-apply slash commands (`setup::refresh_managed_subdirs`), run `doctor::run`, and bump `manifest.integration_version` + `tool_versions` + `touch_and_save`
  - [x] `--full` forces it — `project_refresh_decision(Some(v), v, true)` returns `Refresh { forced: true }`; `setup::refresh_all_assets` also re-copies skills + MEMSTACK.md in this path
- [x] **4.2** Optionally re-run `headroom learn` on update so CLAUDE.md learned-patterns block doesn't rot — `headroom::learn()` is best-effort: returns `Ok(false)` on missing binary or unknown subcommand, warns (never fails) on real errors
- [x] **4.3** Extend version cache to track integration-version — added `integration_version_bundled` and `integration_version_project` to `VersionCache`, both `#[serde(default)]` so pre-Phase-4 caches still parse (covered by `version_cache_parses_legacy_payload_without_integration_fields`)
- [x] **Acceptance:** unit tests in `src/update.rs::tests` pin the version-diff/refresh policy: `behind_bundled_triggers_refresh_without_full`, `full_flag_forces_refresh_even_at_version`, `full_flag_does_not_mark_genuine_upgrade_as_forced`, `no_manifest_means_nothing_to_refresh`, `ahead_of_bundled_skips_when_not_full`. Bumping `INTEGRATION_VERSION` in `src/config.rs` and running `whetstone update` re-applies inits + asset refresh and updates `.claude/whetstone.json` in place; `--full` forces it

---

## Phase 5 — Asset cleanup & content

- [x] **5.1** Deleted the 20 MemStack skills, 8 rules, `pro-skills.md`, `kdp-format`, consultancy skills, plus `MEMSTACK.md` and the two `memstack-*` commands — shipped asset tree shrank to `assets/commands/whetstone-*.md` + `assets/db/schema.sql`
- [x] **5.2** Removed automatically with 5.1 — `diary.md` (n8n webhook) and the `cc_monitor` session hooks were already gone from the shipped tree
- [x] **5.3** Shipped two slash commands calling real binaries: `assets/commands/whetstone-headroom.md` (Headroom `/health` + `/stats`), `assets/commands/whetstone-status.md` (`whetstone doctor`)
- [x] **5.4** ecc-tools auto-gen skill no longer ships (the bundle no longer contains a `skills/` dir at all); the repo-local `.claude/skills/whetstone/` and `.agents/` artifacts are dev-only and tracked by Phase 7.5
- [x] **5.5** New `pub const V2_DB_RELATIVE: &str = "db/memstack.db"` in `src/migrate.rs` is the single source of truth; `src/db.rs::db_path()` and the rollback/detection sites all go through it
- [x] **Acceptance:** shipped asset tree contains only the two slash commands and `db/schema.sql`; no third-party webhook, no telemetry, no dangling skill refs; `MANAGED_COMMANDS` lists both the v2 and v3 names so future migrations stay symmetric; `just release-check` green (68 tests)

---

## Phase 6 — Installer & first-run

- [x] **6.1** `install.sh`: `ensure_uv` runs after binary+assets install; prompts (via `/dev/tty`) and shells out to `curl -LsSf https://astral.sh/uv/install.sh | sh`; falls back to auto-install in non-interactive mode. `~/.local/bin` is prepended to `PATH` so the newly installed uv is visible to the subsequent `whetstone setup` exec.
- [x] **6.2** `install.sh`: when `/dev/tty` is readable+writable, the trailing `exec whetstone setup` is invoked with `</dev/tty`, so `ui::is_interactive()` returns true under `curl | bash` and the TUI wizard actually runs. Without a tty the script exits cleanly with a "run `whetstone setup` to configure" instruction.
- [x] **6.3** `wrapper::wrap_claude` now calls `ensure_proxy_ready()` before exec'ing `headroom wrap claude`: probes `http://127.0.0.1:8787/health`, spawns `headroom proxy --port 8787` detached if no answer, polls every 250ms up to 15s, and soft-warns (continues anyway) on timeout. Closes the race between claude's first API call and the SessionStart hook starting the proxy.
- [x] **6.4** `setup::run` now calls `migrate::detect_and_offer(false)?` *before* dispatching to either `wizard::run` or `run_sequential`, so the v2 hand-off fires under the interactive TUI install path too (previously the wizard silently colonised a v2 project).
- [x] **Acceptance:** clean curl-install ends in a working v3 with clear next-step messaging; missing `uv` is offered (not fatal); the first claude call sees a live proxy; installing over a v2 project triggers `whetstone migrate` regardless of wizard vs sequential mode; `just release-check` green (68 tests).

---

## Phase 7 — Honesty pass (site + docs)

- [ ] **7.1** Replace "97% @ 19% on SQuAD v2" with a cited Headroom benchmark OR numbers from your own `python -m headroom.evals benchmark` run (Headroom's published figures: ~95%+ accuracy preservation at 40–90% reduction)
- [ ] **7.2** Add RTK net-cost caveat + "run `rtk gain` / `rtk discover`, consider audit mode" note (RTK's tracker documents a ~18% cost increase case)
- [ ] **7.3** Reposition pitch around the one-binary orchestration/polish you built, not compression numbers owned by Headroom/RTK
- [ ] **7.4** Rewrite editors matrix (Memory = ICM via `icm init`; AutoMem row removed); update all `docs/*` for v3 commands
  - [ ] Add a **Migration Guide** (v2→v3, including `--dry-run` / `--rollback`)
- [ ] **7.5** Repo housekeeping: stop tracking/shipping dev-env config (`.claude/` ecc-tooling, `.serena/`); reconcile the `/.claude` gitignore
- [ ] **Acceptance:** every published stat is traceable; docs describe only v3 commands; standalone migration guide exists

---

## Phase 8 — Testing, release, rollout

- [ ] **8.1** Unit tests: `integrations.rs` (init invocation + arg shaping), `doctor.rs` (ordering normalization), `migrate.rs` (detection, importance mapping, idempotency, rollback), update-refresh diffing
- [ ] **8.2** Integration test: Phase 3 fixture project, full migrate + rollback round-trip
  - [ ] Windows-path no-op test (RTK hook absent on native Windows — migration skips hook steps gracefully)
- [ ] **8.3** E2E smoke on the same OS/arch matrix as `release.yml`
- [ ] **8.4** Release: `just release set 3.0.0`, but ship `3.0.0-rc.1` first
  - [ ] Add a prerelease path to `verify-release` (it currently asserts not-prerelease) OR run the rc from the v3 branch
- [ ] **8.5** CHANGELOG with explicit **BREAKING** section: AutoMem removed, `whetstone db` removed, hooks now tool-managed, migration required → link the guide
- [ ] **Acceptance:** rc dogfooded on ≥1 real v2 project (yours), green CI matrix, then 3.0.0

---

## Traceability — audit item → phase

| Audit item | Phase |
|---|---|
| §1 tool choices (RTK caveat surfaced) | 7.2 |
| §2 thin-orchestrator architecture | 1 |
| §2 two-memory-systems → ICM only | 1.3, 3, 5.1 |
| §3.1 stdin hook contract | 1.1, 1.4 (deleted) |
| §3.2 ICM `--mode standard` | 2.3 |
| §3.3 RTK MIN_VERSION | 0.3, 2.2 |
| §3.4 `update --full` no-op | 2.4, 4.1 |
| §3.5 model hardcoding | 2.1 |
| §3.6 version drift | 0.4, 2.5 |
| §3.7 `headroom wrap` verify | 0.1, 6.3 |
| §4 MemStack DB calls / webhook / telemetry / catalog / consultancy skills / ecc-tools skill | 5.1–5.5 |
| §5 installer (uv, wizard-via-tty, proxy order) | 6 |
| §6 AutoMem trap, global-hooks gating, DB path | 1.3+3.3, 1.2/1.4, 5.5 |
| §6 RTK hook ordering | 1.2 |
| §7 update doesn't refresh assets | 4.1 |
| §8 unverifiable stat, RTK honesty, positioning, matrix, housekeeping | 7 |
| Migration layer (AutoMem + MemStack→ICM + v2 cleanup) | 3 |

---

## Key risks

- [ ] **ICM import verb unknown (0.2)** — mitigate: runtime capability-check; if no bulk import, write JSONL + instruct user to run ICM import rather than guessing a verb
- [ ] **Tool interface drift** — pin tested `rtk`/`icm`/`headroom` versions in CI; `doctor` validates post-init; document tested versions
- [ ] **Data loss during migrate** — backups + archive + dry-run + rollback; memstack.db renamed, never deleted
- [ ] **`headroom wrap` may not exist (0.1)** — fallback to proxy + service; resolve in Phase 0 before committing the default command

---

## Sequencing & milestones

Order: 1 & 2 (parallel-ish) → 3 (depends on 1) → 4 → 6 → 5 (low-risk, slot anytime after 1) → 7 → 8

- [ ] **M1** — fresh v3 install works
- [ ] **M2** — all confirmed bugs fixed + version single-source-of-truth
- [ ] **M3** — `migrate` dry-run / full / rollback green on the fixture
- [ ] **M4** — update-refresh + installer + cleanup done
- [ ] **M5** — docs/site honest, rc shipped, 3.0.0 released
