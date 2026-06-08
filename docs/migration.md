# Migration Guide — Whetstone v2 → v3

Whetstone v3 is a single Rust binary that **orchestrates** Headroom, RTK and ICM
instead of bundling skills, rules, and a memstack database itself. Migrating
from v2 happens in one command and is fully reversible.

```bash
whetstone migrate                     # interactive
whetstone migrate --dry-run           # show the plan, write nothing
whetstone migrate -y                  # non-interactive (CI)
whetstone migrate --rollback <ID>     # restore a previous v2 state
```

## What `whetstone migrate` does

The migration is staged, archive-backed, and idempotent:

1. **Detect** v2 markers (read-only). Looks for `.claude/memstack/`,
   `MEMSTACK.md`, the v2 hook scripts under `~/.claude/hooks/`, the
   `mcpServers.memory` AutoMem block in `~/.claude/settings.json`, and the
   v2-flavoured skills/rules/commands lists.
2. **Archive** the relevant state under `.whetstone/migration-<id>/`
   (`<id>` is a UTC timestamp like `20260607-153012`):
   - `settings.json.bak` — full backup of `~/.claude/settings.json`
   - `memstack.db.bak` — v2 session DB before re-init
   - `automem.json.bak` — the AutoMem MCP block, if any
   - `memstack-export.jsonl` — MemStack rows in import-ready form
   - `rollback-manifest.json` — every path touched + how to restore it
3. **Tear down** the AutoMem MCP block in `settings.json` (the external
   Railway/Docker service itself is yours to decommission separately).
4. **Map** MemStack memories → ICM via `icm import` (or per-record
   `icm store` as a fallback). Imports are tagged with
   `source=whetstone-migration, migration-id=<id>` so re-running is safe.
5. **Remove** only whetstone-**managed** skills, rules, commands and hook
   scripts. User-authored siblings in the same directories survive.
6. **Re-init the v3 way:** `rtk init --auto-patch`, `icm init --mode standard`,
   `whetstone doctor`, and stamp a fresh `.claude/whetstone.json` manifest
   pinning tool versions.

A sentinel file `.whetstone-migration-completed` lands in the archive once
every step succeeds.

## `--dry-run`

```bash
whetstone migrate --dry-run
```

Prints the detection report and the full plan — what would be archived,
what would be removed, which provider it would init — and writes
nothing. Safe to run on a production project; no files are touched and no
external commands fire.

## Non-interactive (`-y`)

```bash
whetstone migrate -y
```

Skips the confirmation prompt. Useful for CI and automation. The detect →
archive → rollback-manifest write happens before the destructive steps
start, so a `-y` run that fails mid-migration is still rollback-able.

## `--rollback <ID>`

```bash
whetstone migrate --rollback 20260607-153012
```

Reads `.whetstone/migration-<ID>/rollback-manifest.json` and restores every
file it tracks:

- Restores `~/.claude/settings.json` from `settings.json.bak`
- Restores `.claude/memstack/db/memstack.db` from `memstack.db.bak`
- Re-injects the AutoMem `mcpServers.memory` block if one was present
- Re-creates any whetstone-managed skills/rules/commands the migration
  removed
- Clears the `migration_id` marker on `.claude/whetstone.json` (or removes
  the v3 manifest if it didn't exist pre-migration)

Rollback is byte-for-byte for everything inside the project and global
`~/.claude/`. The **external AutoMem service** (FalkorDB + Qdrant) is not
in whetstone's blast radius — restart it yourself if you had one running.

To list available rollbacks:

```bash
ls .whetstone/
# migration-20260607-153012
# migration-20260607-161145
```

## What survives untouched

- **User-authored files** in `.claude/skills/`, `.claude/rules/`,
  `.claude/commands/`, and `~/.claude/hooks/` that whetstone didn't ship.
- `CLAUDE.md` and any project-local conventions.
- Your shell profile beyond the single `ANTHROPIC_BASE_URL` line whetstone
  manages.

## Auto-detection on `whetstone setup`

`whetstone setup` checks for v2 markers up front and hands off to
`whetstone migrate` automatically if it finds any. This fires in **both**
the interactive wizard and the headless setup path — installing v3 over a
v2 project will never silently collide two memory systems.

## After migrating

```bash
whetstone doctor       # confirm versions + hooks
whetstone version      # should print 3.x
icm --version          # provider sanity check
```

If anything looks off, roll back, file the failure mode, and try again.

## See also

- [CLI Reference](cli-reference.md) — full v3 command surface
- [Installation](install.md) — fresh-install path (no v2 history)
- [Troubleshooting](troubleshooting.md) — hooks, proxy, skills
