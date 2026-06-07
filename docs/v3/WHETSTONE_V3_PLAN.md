# Whetstone v3 — Implementation Plan

> Breaking release (v3.0.0). Converts whetstone into a thin orchestrator over Headroom / RTK / ICM,
> removes AutoMem and the MemStack asset layer, adds a v2→v3 migration layer, and fixes all confirmed bugs.
> Ship `3.0.0-rc.1` first, dogfood, then `3.0.0`.

**Legend:** `[ ]` todo · `[~]` in progress · `[x]` done · 🔴 blocker · ⚠️ verify-before-relying

---

## Scope decisions — LOCK THESE FIRST

- [ ] Confirm: memory providers collapse to `Icm | Skip` (AutoMem removed entirely)
- [ ] Confirm: whetstone becomes a thin orchestrator — delegates integration to `rtk init` / `icm init`, stops hand-writing hooks
- [ ] Confirm: remove the 20-skill MemStack bundle, 8 rules, `pro-skills.md` catalog, and the `whetstone db` CLI command (keep the memstack.db *reader* internally for migration only)
- [ ] Confirm: ICM owns memory (its own hooks + skills + CLAUDE.md via `icm init`)
- [ ] Confirm: this is a breaking change → `v3.0.0`, shipped via an `rc` first
- [ ] Calibration noted: v2 wired AutoMem with no endpoint/key + no backend, so AutoMem "migration" is mostly dead-config removal; the real data path is `memstack.db` → ICM

---

## Phase 0 — Foundations & external verification

- [ ] **0.1** ⚠️ Verify `headroom wrap` exists in the pinned Headroom version
  - [ ] If it does NOT exist: design fallback (start proxy + exec claude) for default `whetstone` command and `headroom wrap aider/codex` in docs
- [ ] **0.2** Capture exact current flags for `rtk init` and `icm init` against pinned versions
  - [ ] 🔴 Identify ICM's actual verb for adding/importing a memory (`icm remember` vs `icm import <file>` vs per-concept calls) — gates the migration importer
  - [ ] Pin tested `rtk` / `icm` / `headroom` versions in CI
- [ ] **0.3** Read RTK's actual current version (~0.28.x); set a real `MIN_VERSION` floor or remove the floor
- [ ] **0.4** Make `VERSION` the single source of truth; design propagation to CHANGELOG + `site/src/Releases.jsx` + hardcoded `v2.x` strings
- [ ] **0.5** Cut a `v3` branch; gate new flows behind `--channel rc` / prerelease tag
- [ ] **Acceptance:** one-page "interface contract" doc recording verified `rtk`/`icm`/`headroom` commands + pinned versions

---

## Phase 1 — Core refactor to thin orchestrator

- [ ] **1.1** New `integrations.rs` — shell out to `rtk init` and `icm init`, capture/normalize output (replaces `hooks.rs::build_hooks_value` + `copy_hook_scripts`)
- [ ] **1.2** New `doctor.rs` / `whetstone doctor` — after tool inits, read `~/.claude/settings.json` and:
  - [ ] Normalize hook ordering so RTK's PreToolUse Bash hook is **last**
  - [ ] Confirm ICM hooks present + well-formed
  - [ ] Report anything off (repurpose old `entry_is_whetstone_managed` from "replace" → "inspect/normalize")
- [ ] **1.3** Collapse `MemoryProvider` to `{ Icm, Skip }`
  - [ ] Delete `install_automem`
  - [ ] Delete the `mcpServers.memory` AutoMem branch in `build_hooks_value`
  - [ ] Delete AutoMem detection
- [ ] **1.4** Delete the five whetstone hook scripts from `assets/hooks/`; stop copying them
  - [ ] Move proxy auto-start to `headroom wrap` (if 0.1 confirms) OR systemd/launchd service
  - [ ] Add optional `whetstone proxy service install` helper
- [ ] **1.5** New versioned `whetstone.json` project manifest (replaces `config.local.json`) recording: whetstone version, provider, integration-version, migration id
- [ ] **Acceptance:** fresh `whetstone setup` on clean machine → working v3 (RTK + ICM wired by their own inits, proxy running, `whetstone doctor` green, **zero** whetstone-authored hooks in settings.json)

---

## Phase 2 — Confirmed bug fixes

- [ ] **2.1** Remove default `--model` injection in `wrapper.rs`; let Claude Code settings choose
  - [ ] Drop hardcoded `LATEST_MODEL` upgrade prompt, or make "latest" config/remote-driven
- [ ] **2.2** Set RTK `MIN_VERSION` to a real floor or remove it (per 0.3)
- [ ] **2.3** Replace invalid `icm init --mode standard` with current default (`icm init`, no flag) or `--mode all`
- [ ] **2.4** Wire `update --full` (currently `_full` is ignored) — forces tool upgrades + per-project refresh (lands in Phase 4)
- [ ] **2.5** `just release` regenerates CHANGELOG top entry + Releases feed from `VERSION`; remove hardcoded `2.2.2` in `Releases.jsx` / `InstallTerminal.jsx`
- [ ] **2.6** Stdin hook contract — resolved by deletion (broken `$CLAUDE_TOOL_INPUT` hooks gone in Phase 1); `doctor` validates surviving hooks
- [ ] **Acceptance:** one regression test per fix; `whetstone version` and site agree; no hardcoded model or fictional version floor remains

---

## Phase 3 — Migration layer (`whetstone migrate`)

> Staged, reversible, idempotent. Mirror the `release` preconditions discipline.

### 3.1 Detect (read-only, no writes)
- [ ] Detect `.claude/db/memstack.db` (MemStack data)
- [ ] Detect v2 whetstone-authored hooks in `~/.claude/settings.json` (reuse `entry_is_whetstone_managed`)
- [ ] Detect `mcpServers.memory` → `@verygoodplugins/mcp-automem`; check for `AUTOMEM_ENDPOINT` / `AUTOMEM_API_KEY`
- [ ] Detect managed `.claude/skills/`, `.claude/rules/`, `.claude/commands/`, `MEMSTACK.md`, `config.local.json`
- [ ] Emit a detection report

### 3.2 Backup + export archive (`.whetstone/migration-<ts>/`)
- [ ] Timestamped `settings.json` backup
- [ ] `memstack.db.v2bak` (rename, never delete original)
- [ ] `memstack-export.md` + `memstack-export.jsonl` (sessions / insights / context / plans)
- [ ] `automem-export.jsonl` if reachable

### 3.3 AutoMem teardown
- [ ] If endpoint+key exist and service responds: best-effort pull memories via recall API into export; else skip with clear note
- [ ] Remove `mcpServers.memory` entry (backed up)
- [ ] Do NOT tear down the user's external Railway/Docker service — print decommission instructions instead

### 3.4 MemStack → ICM migration (the real-data path)
- [ ] Ensure ICM installed
- [ ] Read memstack.db via internalized reader; map records:
  - [ ] `insights` → ICM memories tagged by project; importance from `type` (architecture/decision → high/critical; pattern/tool → normal)
  - [ ] `sessions` → one ICM memory each (accomplished/decisions/next_steps), tagged project + date
  - [ ] `project_context` (architecture_decisions/known_issues/backlog) → ICM concepts/memories
  - [ ] `plans` → markdown export only (not imported)
- [ ] Use ICM's verified import path (from 0.2): bulk JSONL if available, else per-memory CLI loop
- [ ] Idempotency: tag every record (`source=whetstone-migration`, `migration-id=<ts>`); write sentinel into renamed backup so re-runs detect prior completion

### 3.5 Cleanup of v2 managed files
- [ ] Build `MANAGED_SKILLS` / `MANAGED_RULES` manifest (analogous to `MANAGED_HOOK_SCRIPTS`) — remove only whetstone's own, preserve user-authored
- [ ] Remove v2 hooks from settings.json
- [ ] Remove `~/.claude/hooks/*.sh` whetstone scripts (if unreferenced)
- [ ] Remove managed project assets + `MEMSTACK.md`

### 3.6 Re-init the v3 way
- [ ] Run `rtk init`
- [ ] Run `icm init`
- [ ] Run `whetstone doctor` to normalize
- [ ] Write new `whetstone.json` recording the migration

### 3.7 Flags
- [ ] `--dry-run` (full plan, no changes — mirror `release-dry-run`)
- [ ] `--yes` (non-interactive)
- [ ] `--rollback <migration-id>` (restore settings.json + memstack.db + removed files; AutoMem service not restored, only its config re-added)

### 3.8 Auto-detect hand-off
- [ ] `whetstone setup` / `update` detect v2 markers and offer to run `migrate`

- [ ] **Acceptance:** on a fixture v2 project (settings.json w/ v2 hooks + seeded memstack.db + AutoMem mcpServers entry):
  - [ ] `migrate --dry-run` reports exact plan
  - [ ] `migrate` → clean v3 state, ICM holds migrated memories, no duplicate on re-run
  - [ ] `migrate --rollback` restores v2 state byte-for-byte (except external AutoMem service)

---

## Phase 4 — Update & per-project refresh

- [ ] **4.1** `whetstone update`: after upgrading global tools, compare `whetstone.json` integration-version vs binary's bundled version
  - [ ] If behind: re-run `rtk init` / `icm init`, re-apply slash commands, run `doctor`
  - [ ] `--full` forces it
- [ ] **4.2** Optionally re-run `headroom learn` on update so CLAUDE.md learned-patterns block doesn't rot
- [ ] **4.3** Extend version cache to track integration-version
- [ ] **Acceptance:** bumping bundled integration-version + `whetstone update` in an existing project re-applies inits and updates `whetstone.json`

---

## Phase 5 — Asset cleanup & content

- [ ] **5.1** Delete the 20 MemStack skills, 8 rules, `pro-skills.md`, `kdp-format`, and consultancy skills (humanize/quill/scan/governor/consolidate)
- [ ] **5.2** Delete the n8n webhook from `diary.md` and the `cc_monitor` telemetry from (now-removed) session hooks — resolves the "NO TELEMETRY" contradiction
- [ ] **5.3** Ship ≤2 slash commands calling real binaries: `/whetstone-headroom` (proxy stats), `/whetstone-status` (doctor summary)
- [ ] **5.4** Remove the `ecc-tools` auto-generated `whetstone` skill from anything shipped (asserts camelCase + `*.test.rs`/`__tests__` for a Rust repo)
  - [ ] If keeping auto-gen for local dev: make it language-aware; either way it must not ship
- [ ] **5.5** Single canonical DB path constant (now only the migration reader cares)
- [ ] **Acceptance:** shipped asset tree has no third-party webhook, no telemetry, no dangling skill refs, nothing unrelated to token optimization

---

## Phase 6 — Installer & first-run

- [ ] **6.1** `install.sh`: detect missing `uv`, offer to install (`curl -LsSf https://astral.sh/uv/install.sh | sh`) instead of aborting in setup
- [ ] **6.2** Fix wizard-via-pipe: after `curl | bash`, re-exec setup against `/dev/tty` OR print "run `whetstone setup` for interactive configuration"
- [ ] **6.3** Verify proxy-up-before-first-call ordering (tied to 0.1 `headroom wrap` outcome)
- [ ] **6.4** Installing over a v2 project hands off to `whetstone migrate` (Phase 3.8)
- [ ] **Acceptance:** clean curl-install on macOS + Linux → working v3 with clear next-step messaging; install over v2 triggers migration

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
