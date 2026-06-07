# Phase 3 Prompt — Migration Layer (`whetstone migrate`)

## Context
whetstone v3 (see Phase 0). This is the heart of the upgrade: a v2→v3 migration that removes AutoMem config, consolidates MemStack `memstack.db` data into ICM, and cleans up the old hand-rolled hooks/skills. **Depends on Phase 1** (the orchestrator + `doctor`, reused for re-init) and **Phase 0.2** (ICM import verb).

Calibration: v2 wired AutoMem with no endpoint/key and never deployed a backend, so for almost all installs AutoMem migration is dead-config removal. The **real data path is `memstack.db` → ICM** — invest effort there. Make the whole thing staged, reversible, and idempotent, mirroring the `release.rs` precondition discipline.

## House rules
- Rust 2021; `anyhow::Result` + `.context()`; `ui::fail()` for fatal; snake_case.
- Must pass `just release-check`. Never delete user data — rename/back up only.

## Objective
Ship `whetstone migrate` with `--dry-run`, `--yes`, and `--rollback <id>`, plus auto-detect hand-off from setup/update.

## Tasks
### 3.1 Detect (read-only)
- [ ] Detect `.claude/db/memstack.db`; v2 whetstone hooks in `~/.claude/settings.json` (reuse `entry_is_whetstone_managed`); `mcpServers.memory` → `@verygoodplugins/mcp-automem` (+ presence of `AUTOMEM_ENDPOINT`/`AUTOMEM_API_KEY`); managed `.claude/{skills,rules,commands}`, `MEMSTACK.md`, `config.local.json`. Emit a report.

### 3.2 Backup + export archive (`.whetstone/migration-<ts>/`)
- [ ] Timestamped `settings.json` backup; `memstack.db.v2bak` (rename, never delete); `memstack-export.md` + `memstack-export.jsonl` (sessions/insights/context/plans); `automem-export.jsonl` if reachable.

### 3.3 AutoMem teardown
- [ ] If endpoint+key exist and the service responds, best-effort pull memories via recall API into the export; else skip with a clear note. Remove the `mcpServers.memory` entry (backed up). Do NOT tear down the user's external Railway/Docker service — print decommission instructions.

### 3.4 MemStack → ICM (real data)
- [ ] Ensure ICM installed. Internalize a `memstack.db` reader. Map: `insights` → ICM memories tagged by project, importance from `type` (architecture/decision → high/critical; pattern/tool → normal); `sessions` → one ICM memory each (accomplished/decisions/next_steps), tagged project+date; `project_context` → concepts/memories; `plans` → markdown export only.
- [ ] Use ICM's verified import path (Phase 0.2): bulk JSONL if available, else per-memory CLI loop. Tag every record (`source=whetstone-migration`, `migration-id=<ts>`) and write a sentinel into the renamed backup so re-runs detect completion and never duplicate.

### 3.5 Cleanup of v2 managed files
- [ ] Build `MANAGED_SKILLS` / `MANAGED_RULES` manifests (like `MANAGED_HOOK_SCRIPTS`) so only whetstone's own files are removed and user-authored ones survive. Remove v2 hooks from settings.json, the `~/.claude/hooks/*.sh` whetstone scripts (if unreferenced), and managed project assets + `MEMSTACK.md`.

### 3.6 Re-init the v3 way
- [ ] Run `rtk init`, `icm init`, then `whetstone doctor`. Write the new `whetstone.json` recording the migration.

### 3.7 Flags
- [ ] `--dry-run` (full plan, no writes — mirror `release-dry-run`), `--yes`, `--rollback <migration-id>` (restore settings.json + memstack.db + removed files; re-add AutoMem config but do not restore the external service).

### 3.8 Auto-detect
- [ ] `whetstone setup` / `update` detect v2 markers and offer to run `migrate`.

## Files likely touched
`src/migrate.rs` (new), `src/cli.rs` (add `migrate` + flags), `src/db.rs` (reader survives here), `src/integrations.rs`, `src/doctor.rs`, `src/setup.rs`, `src/update.rs`, `src/main.rs`.

## Deliverable / Done when
On a fixture v2 project (settings.json with v2 hooks + seeded `memstack.db` + an AutoMem `mcpServers` entry): `migrate --dry-run` reports the exact plan; `migrate` yields a clean v3 state with ICM holding the migrated memories and no duplicates on re-run; `migrate --rollback` restores the v2 state byte-for-byte except the external AutoMem service.
