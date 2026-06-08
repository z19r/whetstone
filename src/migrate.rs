//! Phase 3 — v2 → v3 migration layer (`whetstone migrate`).
//!
//! Staged, reversible, idempotent transition:
//!   1. Detect v2 markers (read-only).
//!   2. Create a timestamped archive under `.whetstone/migration-<ts>/`
//!      with backups + JSONL exports.
//!   3. Tear down the AutoMem `mcpServers.memory` block (config only;
//!      external Railway/Docker service is the user's call).
//!   4. Map MemStack `memstack.db` rows → ICM via `icm import` (preferred)
//!      or per-record `icm store` (fallback).
//!   5. Remove only whetstone-managed skills/rules/commands and hook
//!      scripts — user-authored siblings survive.
//!   6. Re-init the v3 way (`rtk init`, `icm init`, `whetstone doctor`)
//!      and stamp a fresh `whetstone.json`.
//!
//! Every step records enough metadata in the archive that
//! `whetstone migrate --rollback <id>` can restore the v2 state
//! byte-for-byte (except the external AutoMem service).

use anyhow::{bail, Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::WhetstoneManifest;
use crate::memory::MemoryProvider;
use crate::{config, doctor, integrations, ui};

// ----------------------------------------------------------------------------
// Managed-asset manifests.
//
// Source of truth for what whetstone v2 ever shipped. Anything NOT in these
// lists is treated as user-authored and left alone during cleanup.
// ----------------------------------------------------------------------------

/// Skill directory names whetstone v2 bundled into `.claude/skills/`.
pub const MANAGED_SKILLS: &[&str] = &[
    "api-docs",
    "compress",
    "consolidate",
    "context-db",
    "diary",
    "echo",
    "familiar",
    "forge",
    "governor",
    "grimoire",
    "humanize",
    "kdp-format",
    "project",
    "quill",
    "scan",
    "shard",
    "sight",
    "state",
    "verify",
    "work",
];

/// Rule files whetstone v2 bundled into `.claude/rules/`.
pub const MANAGED_RULES: &[&str] = &[
    "diary.md",
    "echo.md",
    "headroom.md",
    "kdp-format.md",
    "memstack.md",
    "notify.md",
    "pro-skills.md",
    "work.md",
];

/// Command files whetstone bundles into `.claude/commands/`. Includes
/// both the v2 names (cleaned during migration) and the v3 names
/// shipped today, so cleanup paths stay symmetric across upgrades.
pub const MANAGED_COMMANDS: &[&str] = &[
    "memstack-headroom.md",
    "memstack-search.md",
    "whetstone-headroom.md",
    "whetstone-status.md",
];

/// Canonical relative path (under a project's `.claude/`) of the v2
/// MemStack SQLite database.
///
/// v3 no longer writes to this file. The migration reader uses it to
/// detect a v2 install and the legacy `whetstone db` CLI uses it for
/// backward-compat reads — both reach for the same path through this
/// constant.
pub const V2_DB_RELATIVE: &str = "db/memstack.db";

/// Hook scripts whetstone v2 dropped into `~/.claude/hooks/`.
pub const MANAGED_HOOK_SCRIPTS: &[&str] = &[
    "whetstone-session-start.sh",
    "whetstone-session-stop.sh",
    "whetstone-tts-notify.sh",
    "whetstone-pre-push.sh",
    "whetstone-post-commit.sh",
];

/// A `hooks.{event}[]` entry is whetstone-managed if any of its nested
/// `hooks[].command` strings references a known v2 script name or a
/// path under `~/.claude/hooks/whetstone-*`.
fn entry_is_whetstone_managed(entry: &Value) -> bool {
    let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) else {
        return false;
    };
    hooks.iter().any(|h| {
        h.get("command")
            .and_then(|c| c.as_str())
            .map(|cmd| {
                MANAGED_HOOK_SCRIPTS.iter().any(|name| cmd.contains(name))
                    || cmd.contains(".claude/hooks/whetstone-")
            })
            .unwrap_or(false)
    })
}

// ----------------------------------------------------------------------------
// Detection.
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Detection {
    pub project_dir: PathBuf,
    pub home_dir: PathBuf,
    pub settings_path: PathBuf,
    pub memstack_db: Option<PathBuf>,
    pub v2_hook_count: usize,
    pub automem_present: bool,
    pub automem_endpoint: Option<String>,
    pub automem_api_key_env_set: bool,
    pub managed_skills: Vec<String>,
    pub managed_rules: Vec<String>,
    pub managed_commands: Vec<String>,
    pub memstack_md: bool,
    pub config_local_json: bool,
    pub managed_hooks_on_disk: Vec<PathBuf>,
    pub already_migrated: Option<String>,
}

impl Detection {
    pub fn needs_migration(&self) -> bool {
        self.already_migrated.is_none()
            && (self.memstack_db.is_some()
                || self.v2_hook_count > 0
                || self.automem_present
                || !self.managed_skills.is_empty()
                || !self.managed_rules.is_empty()
                || !self.managed_commands.is_empty()
                || self.memstack_md
                || self.config_local_json
                || !self.managed_hooks_on_disk.is_empty())
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("v2 markers detected:\n");
        out.push_str(&format!(
            "  - MemStack DB: {}\n",
            opt_path(&self.memstack_db)
        ));
        out.push_str(&format!(
            "  - v2 whetstone hook entries in settings.json: {}\n",
            self.v2_hook_count
        ));
        out.push_str(&format!(
            "  - AutoMem mcpServers.memory present: {}\n",
            self.automem_present
        ));
        if let Some(ep) = &self.automem_endpoint {
            out.push_str(&format!("    endpoint: {ep}\n"));
        }
        if self.automem_api_key_env_set {
            out.push_str("    AUTOMEM_API_KEY env: set\n");
        }
        out.push_str(&format!(
            "  - managed skills:   {} / {}\n",
            self.managed_skills.len(),
            MANAGED_SKILLS.len()
        ));
        out.push_str(&format!(
            "  - managed rules:    {} / {}\n",
            self.managed_rules.len(),
            MANAGED_RULES.len()
        ));
        out.push_str(&format!(
            "  - managed commands: {} / {}\n",
            self.managed_commands.len(),
            MANAGED_COMMANDS.len()
        ));
        out.push_str(&format!("  - MEMSTACK.md present: {}\n", self.memstack_md));
        out.push_str(&format!(
            "  - legacy config.local.json: {}\n",
            self.config_local_json
        ));
        out.push_str(&format!(
            "  - managed hook scripts on disk: {}\n",
            self.managed_hooks_on_disk.len()
        ));
        if let Some(id) = &self.already_migrated {
            out.push_str(&format!("  - already migrated by: {id}\n"));
        }
        out
    }
}

fn opt_path(p: &Option<PathBuf>) -> String {
    match p {
        Some(p) => p.display().to_string(),
        None => "(none)".to_string(),
    }
}

pub fn detect() -> Result<Detection> {
    let project_dir = std::env::current_dir().context("reading current dir")?;
    let home_dir = dirs::home_dir().context("locating home dir")?;
    detect_at(&project_dir, &home_dir)
}

pub fn detect_at(project_dir: &Path, home_dir: &Path) -> Result<Detection> {
    let settings_path = home_dir.join(".claude/settings.json");
    let claude_dir = project_dir.join(".claude");

    let memstack_db = {
        let p = claude_dir.join(V2_DB_RELATIVE);
        if p.exists() {
            Some(p)
        } else {
            None
        }
    };

    let settings_value: Option<Value> = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path)
            .with_context(|| format!("reading {}", settings_path.display()))?;
        Some(
            serde_json::from_str(&raw)
                .with_context(|| format!("parsing {}", settings_path.display()))?,
        )
    } else {
        None
    };

    let v2_hook_count = settings_value
        .as_ref()
        .map(count_v2_hook_entries)
        .unwrap_or(0);

    let (automem_present, automem_endpoint) = settings_value
        .as_ref()
        .map(detect_automem)
        .unwrap_or((false, None));
    let automem_api_key_env_set = std::env::var("AUTOMEM_API_KEY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    let managed_skills = present_subset(&claude_dir.join("skills"), MANAGED_SKILLS, true);
    let managed_rules = present_subset(&claude_dir.join("rules"), MANAGED_RULES, false);
    let managed_commands = present_subset(&claude_dir.join("commands"), MANAGED_COMMANDS, false);

    let memstack_md = claude_dir.join("MEMSTACK.md").exists();
    let config_local_json = claude_dir.join("config.local.json").exists();

    let managed_hooks_on_disk = MANAGED_HOOK_SCRIPTS
        .iter()
        .map(|n| home_dir.join(".claude/hooks").join(n))
        .filter(|p| p.exists())
        .collect();

    let already_migrated = WhetstoneManifest::load(&WhetstoneManifest::path_for(project_dir))
        .ok()
        .flatten()
        .and_then(|m| m.migration_id().map(String::from));

    Ok(Detection {
        project_dir: project_dir.to_path_buf(),
        home_dir: home_dir.to_path_buf(),
        settings_path,
        memstack_db,
        v2_hook_count,
        automem_present,
        automem_endpoint,
        automem_api_key_env_set,
        managed_skills,
        managed_rules,
        managed_commands,
        memstack_md,
        config_local_json,
        managed_hooks_on_disk,
        already_migrated,
    })
}

fn count_v2_hook_entries(settings: &Value) -> usize {
    let Some(hooks) = settings.get("hooks").and_then(|h| h.as_object()) else {
        return 0;
    };
    hooks
        .values()
        .filter_map(|v| v.as_array())
        .flatten()
        .filter(|entry| entry_is_whetstone_managed(entry))
        .count()
}

fn detect_automem(settings: &Value) -> (bool, Option<String>) {
    let Some(server) = settings
        .get("mcpServers")
        .and_then(|m| m.get("memory"))
        .and_then(|m| m.as_object())
    else {
        return (false, None);
    };

    let is_automem = server
        .get("command")
        .and_then(|c| c.as_str())
        .map(|cmd| cmd.contains("verygoodplugins/mcp-automem") || cmd.contains("mcp-automem"))
        .unwrap_or(false)
        || server
            .get("args")
            .and_then(|a| a.as_array())
            .map(|args| {
                args.iter()
                    .filter_map(|x| x.as_str())
                    .any(|s| s.contains("verygoodplugins/mcp-automem") || s.contains("mcp-automem"))
            })
            .unwrap_or(false);

    let endpoint = server
        .get("env")
        .and_then(|e| e.get("AUTOMEM_ENDPOINT"))
        .and_then(|v| v.as_str())
        .map(String::from);

    (is_automem, endpoint)
}

fn present_subset(dir: &Path, allowed: &[&str], expect_subdir: bool) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let on_disk: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            if expect_subdir {
                e.file_type().map(|t| t.is_dir()).unwrap_or(false)
            } else {
                e.file_type().map(|t| t.is_file()).unwrap_or(false)
            }
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();

    allowed
        .iter()
        .filter(|name| on_disk.iter().any(|n| n == *name))
        .map(|s| s.to_string())
        .collect()
}

// ----------------------------------------------------------------------------
// Migration archive layout.
// ----------------------------------------------------------------------------

fn archive_root(project_dir: &Path) -> PathBuf {
    project_dir.join(".whetstone")
}

fn archive_dir(project_dir: &Path, migration_id: &str) -> PathBuf {
    archive_root(project_dir).join(format!("migration-{migration_id}"))
}

fn sentinel_path(archive: &Path) -> PathBuf {
    archive.join(".whetstone-migration-completed")
}

#[derive(Debug, Serialize, Deserialize)]
struct RollbackManifest {
    migration_id: String,
    archived_at: chrono::DateTime<Utc>,
    settings_backup: Option<PathBuf>,
    memstack_backup: Option<PathBuf>,
    automem_backup: Option<PathBuf>,
    removed_files: Vec<RemovedFile>,
    removed_hook_indices: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RemovedFile {
    original: PathBuf,
    archived: PathBuf,
}

fn rollback_manifest_path(archive: &Path) -> PathBuf {
    archive.join("rollback-manifest.json")
}

// ----------------------------------------------------------------------------
// Public entry points.
// ----------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct MigrateOptions {
    pub dry_run: bool,
    pub yes: bool,
}

pub fn run(opts: MigrateOptions) -> Result<()> {
    let det = detect()?;

    ui::section("whetstone migrate");
    eprintln!("{}", det.render());

    if let Some(id) = &det.already_migrated {
        ui::ok(&format!("already migrated ({id}); nothing to do"));
        return Ok(());
    }

    if !det.needs_migration() {
        ui::ok("no v2 markers detected; nothing to do");
        return Ok(());
    }

    if opts.dry_run {
        ui::info("--dry-run: no files written, no commands executed");
        print_dry_run_plan(&det);
        return Ok(());
    }

    if !opts.yes && !ui::confirm("proceed with migration?", true) {
        ui::warn("migration cancelled by user");
        return Ok(());
    }

    let migration_id = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let archive = archive_dir(&det.project_dir, &migration_id);
    fs::create_dir_all(&archive).with_context(|| format!("creating {}", archive.display()))?;
    ui::ok(&format!("archive: {}", archive.display()));

    let mut rb = RollbackManifest {
        migration_id: migration_id.clone(),
        archived_at: Utc::now(),
        settings_backup: None,
        memstack_backup: None,
        automem_backup: None,
        removed_files: Vec::new(),
        removed_hook_indices: Vec::new(),
    };

    backup_settings(&det, &archive, &mut rb)?;
    backup_memstack(&det, &archive, &mut rb)?;
    export_memstack(&det, &archive)?;
    export_automem(&det, &archive)?;

    if det.automem_present {
        teardown_automem(&det, &archive, &mut rb)?;
    }

    if det.memstack_db.is_some() {
        import_memstack_into_icm(&det, &archive, &migration_id)?;
    }

    cleanup_managed(&det, &archive, &mut rb)?;

    // Persist rollback manifest before re-init so a re-init failure is still recoverable.
    fs::write(
        rollback_manifest_path(&archive),
        serde_json::to_string_pretty(&rb).context("serialising rollback manifest")?,
    )
    .with_context(|| format!("writing {}", rollback_manifest_path(&archive).display()))?;

    reinit_v3(&det, &migration_id)?;

    fs::write(sentinel_path(&archive), &migration_id)
        .with_context(|| format!("writing sentinel for {migration_id}"))?;

    ui::summary_ok(&format!(
        "migration {migration_id} complete; archive: {}",
        archive.display()
    ));
    Ok(())
}

pub fn rollback(migration_id: &str) -> Result<()> {
    let project_dir = std::env::current_dir().context("reading current dir")?;
    let archive = archive_dir(&project_dir, migration_id);
    if !archive.is_dir() {
        bail!("no archive at {}", archive.display());
    }
    let raw = fs::read_to_string(rollback_manifest_path(&archive))
        .with_context(|| format!("reading {}", rollback_manifest_path(&archive).display()))?;
    let rb: RollbackManifest =
        serde_json::from_str(&raw).context("parsing rollback-manifest.json")?;

    ui::section(&format!("whetstone migrate --rollback {migration_id}"));

    if let Some(backup) = &rb.settings_backup {
        let home = dirs::home_dir().context("locating home dir")?;
        let dest = home.join(".claude/settings.json");
        fs::copy(backup, &dest).with_context(|| format!("restoring {}", dest.display()))?;
        ui::ok(&format!("restored {}", dest.display()));
    }

    if let Some(backup) = &rb.memstack_backup {
        let dest = project_dir.join(".claude").join(V2_DB_RELATIVE);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::copy(backup, &dest).with_context(|| format!("restoring {}", dest.display()))?;
        // Drop the .v2bak sibling so the v2 layout is once again canonical.
        let bak = dest.with_extension("db.v2bak");
        let _ = fs::remove_file(&bak);
        ui::ok(&format!("restored {}", dest.display()));
    }

    for entry in &rb.removed_files {
        if let Some(parent) = entry.original.parent() {
            fs::create_dir_all(parent).ok();
        }
        if entry.archived.is_dir() {
            copy_dir_recursive(&entry.archived, &entry.original)?;
        } else {
            fs::copy(&entry.archived, &entry.original)
                .with_context(|| format!("restoring {}", entry.original.display()))?;
        }
    }
    if !rb.removed_files.is_empty() {
        ui::ok(&format!(
            "restored {} managed files/directories",
            rb.removed_files.len()
        ));
    }

    // Drop the sentinel so re-running migrate is allowed again.
    let _ = fs::remove_file(sentinel_path(&archive));

    // Clear migration_id marker on the manifest.
    let manifest_p = WhetstoneManifest::path_for(&project_dir);
    if let Ok(Some(mut m)) = WhetstoneManifest::load(&manifest_p) {
        m.clear_migration_id();
        m.touch_and_save(&manifest_p).ok();
    }

    ui::summary_ok(&format!("rolled back migration {migration_id}"));
    ui::info("note: external AutoMem service (Railway/Docker) is not re-deployed");
    Ok(())
}

/// Called from `whetstone setup` / `whetstone update`. Returns whether
/// the migration ran (so the caller can skip duplicate work).
pub fn detect_and_offer(non_interactive_default: bool) -> Result<bool> {
    let det = detect()?;
    if !det.needs_migration() {
        return Ok(false);
    }

    ui::warn("v2 install detected — recommend running `whetstone migrate`");
    eprintln!("{}", det.render());

    let should_run = if non_interactive_default {
        true
    } else {
        ui::confirm("run migration now?", true)
    };

    if !should_run {
        ui::info("skipped; run `whetstone migrate` later");
        return Ok(false);
    }

    run(MigrateOptions {
        dry_run: false,
        yes: non_interactive_default,
    })?;
    Ok(true)
}

// ----------------------------------------------------------------------------
// 3.2 — Backup + export archive.
// ----------------------------------------------------------------------------

fn backup_settings(det: &Detection, archive: &Path, rb: &mut RollbackManifest) -> Result<()> {
    if !det.settings_path.exists() {
        return Ok(());
    }
    let dest = archive.join("settings.json.backup");
    fs::copy(&det.settings_path, &dest)
        .with_context(|| format!("backing up {}", det.settings_path.display()))?;
    rb.settings_backup = Some(dest.clone());
    ui::ok(&format!("backed up settings.json → {}", dest.display()));
    Ok(())
}

fn backup_memstack(det: &Detection, archive: &Path, rb: &mut RollbackManifest) -> Result<()> {
    let Some(src) = &det.memstack_db else {
        return Ok(());
    };
    let renamed = src.with_extension("db.v2bak");
    if src.exists() && !renamed.exists() {
        fs::rename(src, &renamed)
            .with_context(|| format!("renaming {} → {}", src.display(), renamed.display()))?;
        ui::ok(&format!(
            "renamed {} → {}",
            src.display(),
            renamed.display()
        ));
    }
    let archived = archive.join("memstack.db");
    if renamed.exists() {
        fs::copy(&renamed, &archived)
            .with_context(|| format!("archiving {}", renamed.display()))?;
    } else if src.exists() {
        fs::copy(src, &archived).with_context(|| format!("archiving {}", src.display()))?;
    }
    rb.memstack_backup = Some(archived);
    Ok(())
}

fn export_memstack(det: &Detection, archive: &Path) -> Result<()> {
    let Some(db) = pick_memstack_source(det) else {
        return Ok(());
    };
    let conn = Connection::open(&db).with_context(|| format!("opening {}", db.display()))?;

    let mut jsonl = String::new();
    let mut md = String::from("# MemStack export\n\n");

    md.push_str("## Sessions\n\n");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT project, date, accomplished, decisions, next_steps FROM sessions ORDER BY date",
    ) {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0).unwrap_or_default(),
                    r.get::<_, String>(1).unwrap_or_default(),
                    r.get::<_, Option<String>>(2)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(3)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(4)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                ))
            })
            .context("querying sessions")?;
        for row in rows.flatten() {
            jsonl.push_str(&serde_json::to_string(&json!({
                "kind": "session",
                "project": row.0,
                "date": row.1,
                "accomplished": row.2,
                "decisions": row.3,
                "next_steps": row.4,
            }))?);
            jsonl.push('\n');
            md.push_str(&format!("### {} — {}\n\n", row.0, row.1));
            if !row.2.is_empty() {
                md.push_str(&format!("**Accomplished**\n\n{}\n\n", row.2));
            }
            if !row.3.is_empty() {
                md.push_str(&format!("**Decisions**\n\n{}\n\n", row.3));
            }
            if !row.4.is_empty() {
                md.push_str(&format!("**Next steps**\n\n{}\n\n", row.4));
            }
        }
    }

    md.push_str("## Insights\n\n");
    if let Ok(mut stmt) =
        conn.prepare("SELECT project, type, content FROM insights ORDER BY project, type")
    {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, String>(1).unwrap_or_else(|_| "decision".into()),
                    r.get::<_, String>(2).unwrap_or_default(),
                ))
            })
            .context("querying insights")?;
        for row in rows.flatten() {
            jsonl.push_str(&serde_json::to_string(&json!({
                "kind": "insight",
                "project": row.0,
                "type": row.1,
                "content": row.2,
            }))?);
            jsonl.push('\n');
            md.push_str(&format!("- [{}/{}] {}\n", row.0, row.1, row.2));
        }
        md.push('\n');
    }

    md.push_str("## Project context\n\n");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT project, status, current_branch, architecture_decisions, known_issues, backlog \
         FROM project_context",
    ) {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0).unwrap_or_default(),
                    r.get::<_, Option<String>>(1)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(2)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(3)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(4)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(5)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                ))
            })
            .context("querying project_context")?;
        for row in rows.flatten() {
            jsonl.push_str(&serde_json::to_string(&json!({
                "kind": "context",
                "project": row.0,
                "status": row.1,
                "current_branch": row.2,
                "architecture_decisions": row.3,
                "known_issues": row.4,
                "backlog": row.5,
            }))?);
            jsonl.push('\n');
            md.push_str(&format!("### {}\n\n", row.0));
            md.push_str(&format!("- status: {}\n- branch: {}\n", row.1, row.2));
        }
        md.push('\n');
    }

    md.push_str("## Plans\n\n");
    if let Ok(mut stmt) = conn.prepare(
        "SELECT project, task_number, description, status FROM plans ORDER BY project, task_number",
    ) {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0).unwrap_or_default(),
                    r.get::<_, Option<i64>>(1).ok().flatten().unwrap_or(0),
                    r.get::<_, String>(2).unwrap_or_default(),
                    r.get::<_, String>(3).unwrap_or_default(),
                ))
            })
            .context("querying plans")?;
        for row in rows.flatten() {
            md.push_str(&format!(
                "- [{}] {} #{}: {} ({})\n",
                if row.3 == "completed" { "x" } else { " " },
                row.0,
                row.1,
                row.2,
                row.3
            ));
        }
    }

    fs::write(archive.join("memstack-export.jsonl"), jsonl)
        .context("writing memstack-export.jsonl")?;
    fs::write(archive.join("memstack-export.md"), md).context("writing memstack-export.md")?;
    ui::ok("exported memstack.db → JSONL + Markdown");
    Ok(())
}

fn pick_memstack_source(det: &Detection) -> Option<PathBuf> {
    if let Some(p) = &det.memstack_db {
        if p.exists() {
            return Some(p.clone());
        }
        let renamed = p.with_extension("db.v2bak");
        if renamed.exists() {
            return Some(renamed);
        }
    }
    None
}

fn export_automem(det: &Detection, archive: &Path) -> Result<()> {
    let (Some(endpoint), Ok(api_key)) = (
        det.automem_endpoint.as_ref(),
        std::env::var("AUTOMEM_API_KEY"),
    ) else {
        return Ok(());
    };
    if api_key.is_empty() {
        return Ok(());
    }

    let url = format!("{}/recall?q=&limit=1000", endpoint.trim_end_matches('/'));
    let resp = match ureq::get(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .timeout(std::time::Duration::from_secs(10))
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            ui::warn(&format!("AutoMem export skipped: {e}"));
            return Ok(());
        }
    };

    let body = match resp.into_string() {
        Ok(b) => b,
        Err(e) => {
            ui::warn(&format!("AutoMem export skipped: {e}"));
            return Ok(());
        }
    };

    fs::write(archive.join("automem-export.jsonl"), body)
        .context("writing automem-export.jsonl")?;
    ui::ok("exported AutoMem memories");
    Ok(())
}

// ----------------------------------------------------------------------------
// 3.3 — AutoMem teardown.
// ----------------------------------------------------------------------------

fn teardown_automem(det: &Detection, archive: &Path, rb: &mut RollbackManifest) -> Result<()> {
    if !det.settings_path.exists() {
        return Ok(());
    }
    let raw = fs::read_to_string(&det.settings_path)?;
    let mut settings: Value = serde_json::from_str(&raw)?;

    let removed = settings
        .get_mut("mcpServers")
        .and_then(|m| m.as_object_mut())
        .and_then(|m| m.remove("memory"));

    if let Some(removed_value) = removed {
        let automem_backup = archive.join("automem-mcp-entry.json");
        fs::write(
            &automem_backup,
            serde_json::to_string_pretty(&removed_value)?,
        )?;
        rb.automem_backup = Some(automem_backup);

        if settings
            .get("mcpServers")
            .and_then(|m| m.as_object())
            .map(|m| m.is_empty())
            .unwrap_or(false)
        {
            settings.as_object_mut().unwrap().remove("mcpServers");
        }

        fs::write(&det.settings_path, serde_json::to_string_pretty(&settings)?)?;

        ui::ok("removed mcpServers.memory (AutoMem) from settings.json");
        ui::info(
            "external AutoMem service (Railway/Docker) is yours to decommission — \
             see https://github.com/verygoodplugins/mcp-automem for shutdown steps",
        );
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// 3.4 — MemStack → ICM.
// ----------------------------------------------------------------------------

fn import_memstack_into_icm(det: &Detection, archive: &Path, migration_id: &str) -> Result<()> {
    let Some(db) = pick_memstack_source(det) else {
        return Ok(());
    };
    if which::which("icm").is_err() {
        ui::warn(
            "icm binary not found — skipping data import. \
             Install icm and re-run `whetstone migrate`.",
        );
        return Ok(());
    }

    let conn = Connection::open(&db)?;
    let records = collect_icm_records(&conn, migration_id)?;
    if records.is_empty() {
        ui::info("no insights/sessions/context to import");
        return Ok(());
    }

    let bulk_path = archive.join("icm-import.jsonl");
    let mut bulk = String::new();
    for r in &records {
        bulk.push_str(&serde_json::to_string(&r.as_json())?);
        bulk.push('\n');
    }
    fs::write(&bulk_path, &bulk).context("writing icm-import.jsonl")?;

    let bulk_ok = try_icm_bulk_import(&bulk_path).unwrap_or(false);
    if bulk_ok {
        ui::ok(&format!("icm import: {} records (bulk)", records.len()));
        return Ok(());
    }

    ui::info("falling back to per-record `icm store`");
    let mut stored = 0usize;
    let mut failed = 0usize;
    for r in &records {
        match icm_store_one(r) {
            Ok(()) => stored += 1,
            Err(e) => {
                failed += 1;
                ui::warn(&format!("icm store failed for {:?}: {e}", r.tags));
            }
        }
    }
    ui::ok(&format!("icm store: {stored} stored, {failed} failed"));
    Ok(())
}

#[derive(Debug)]
struct IcmRecord {
    content: String,
    topic: String,
    importance: &'static str,
    keywords: Vec<String>,
    tags: Vec<String>,
}

impl IcmRecord {
    fn as_json(&self) -> Value {
        json!({
            "content": self.content,
            "topic": self.topic,
            "importance": self.importance,
            "keywords": self.keywords,
            "tags": self.tags,
        })
    }
}

fn collect_icm_records(conn: &Connection, migration_id: &str) -> Result<Vec<IcmRecord>> {
    let tag_source = "source=whetstone-migration".to_string();
    let tag_id = format!("migration-id={migration_id}");
    let mut out = Vec::new();

    if let Ok(mut stmt) = conn.prepare("SELECT project, type, content FROM insights") {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, String>(1).unwrap_or_else(|_| "decision".into()),
                    r.get::<_, String>(2).unwrap_or_default(),
                ))
            })
            .context("reading insights")?;
        for (project, kind, content) in rows.flatten() {
            if content.is_empty() {
                continue;
            }
            let importance = match kind.as_str() {
                "architecture" => "critical",
                "decision" => "high",
                "pattern" | "tool" | "bug-fix" => "normal",
                _ => "normal",
            };
            out.push(IcmRecord {
                content: format!("[{kind}] {content}"),
                topic: project_topic(&project),
                importance,
                keywords: vec![kind.clone()],
                tags: vec![tag_source.clone(), tag_id.clone(), kind.clone()],
            });
        }
    }

    if let Ok(mut stmt) =
        conn.prepare("SELECT project, date, accomplished, decisions, next_steps FROM sessions")
    {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0).unwrap_or_default(),
                    r.get::<_, String>(1).unwrap_or_default(),
                    r.get::<_, Option<String>>(2)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(3)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(4)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                ))
            })
            .context("reading sessions")?;
        for (project, date, accomplished, decisions, next_steps) in rows.flatten() {
            let content = format!(
                "Session {date} — accomplished: {accomplished}\n\
                 decisions: {decisions}\n\
                 next: {next_steps}"
            );
            out.push(IcmRecord {
                content,
                topic: project_topic(&project),
                importance: "normal",
                keywords: vec!["session".into(), date.clone()],
                tags: vec![
                    tag_source.clone(),
                    tag_id.clone(),
                    "session".to_string(),
                    format!("date={date}"),
                ],
            });
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT project, architecture_decisions, known_issues, backlog FROM project_context",
    ) {
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0).unwrap_or_default(),
                    r.get::<_, Option<String>>(1)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(2)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    r.get::<_, Option<String>>(3)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                ))
            })
            .context("reading project_context")?;
        for (project, arch, issues, backlog) in rows.flatten() {
            let content = format!(
                "Project context: architecture: {arch}\nknown issues: {issues}\nbacklog: {backlog}"
            );
            out.push(IcmRecord {
                content,
                topic: project_topic(&project),
                importance: "high",
                keywords: vec!["context".into()],
                tags: vec![tag_source.clone(), tag_id.clone(), "context".to_string()],
            });
        }
    }

    Ok(out)
}

fn project_topic(project: &str) -> String {
    if project.is_empty() {
        "whetstone-migration".into()
    } else {
        format!("project-{project}")
    }
}

fn try_icm_bulk_import(path: &Path) -> Result<bool> {
    let out = Command::new("icm")
        .args(["import", "--format", "auto"])
        .arg(path)
        .output()
        .context("spawning `icm import`")?;
    Ok(out.status.success())
}

fn icm_store_one(r: &IcmRecord) -> Result<()> {
    let mut cmd = Command::new("icm");
    cmd.args(["store", "--topic", &r.topic, "--importance", r.importance]);
    for kw in &r.keywords {
        cmd.args(["-k", kw]);
    }
    cmd.arg(&r.content);
    let out = cmd.output().context("spawning `icm store`")?;
    if !out.status.success() {
        bail!(
            "icm store exited {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// 3.5 — Cleanup managed v2 files.
// ----------------------------------------------------------------------------

fn cleanup_managed(det: &Detection, archive: &Path, rb: &mut RollbackManifest) -> Result<()> {
    let mut archive_seq: BTreeMap<&str, usize> = BTreeMap::new();
    let mut next_archive_path = |kind: &'static str, original: &Path| -> PathBuf {
        let n = archive_seq.entry(kind).or_insert(0);
        *n += 1;
        let name = original
            .file_name()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{kind}-{n}.bin"));
        archive.join("files").join(kind).join(name)
    };

    if det.settings_path.exists() && det.v2_hook_count > 0 {
        let raw = fs::read_to_string(&det.settings_path)?;
        let mut settings: Value = serde_json::from_str(&raw)?;
        if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            for (event, entries) in hooks.iter_mut() {
                if let Some(arr) = entries.as_array_mut() {
                    let before = arr.len();
                    arr.retain(|e| !entry_is_whetstone_managed(e));
                    let removed = before - arr.len();
                    if removed > 0 {
                        rb.removed_hook_indices.push(format!("{event}:-{removed}"));
                    }
                }
            }
        }
        fs::write(&det.settings_path, serde_json::to_string_pretty(&settings)?)?;
        ui::ok("removed v2 whetstone hook entries from settings.json");
    }

    let referenced = hook_script_references(&det.settings_path);
    for script in &det.managed_hooks_on_disk {
        let name = script.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if referenced.iter().any(|r| r.contains(name)) {
            ui::warn(&format!(
                "leaving {} — still referenced by settings.json",
                script.display()
            ));
            continue;
        }
        let dest = next_archive_path("hooks", script);
        archive_and_remove(script, &dest, rb)?;
    }

    let skills_dir = det.project_dir.join(".claude/skills");
    for name in &det.managed_skills {
        let path = skills_dir.join(name);
        if !path.exists() {
            continue;
        }
        let dest = next_archive_path("skills", &path);
        archive_dir_and_remove(&path, &dest, rb)?;
    }

    let rules_dir = det.project_dir.join(".claude/rules");
    for name in &det.managed_rules {
        let path = rules_dir.join(name);
        if !path.exists() {
            continue;
        }
        let dest = next_archive_path("rules", &path);
        archive_and_remove(&path, &dest, rb)?;
    }
    let commands_dir = det.project_dir.join(".claude/commands");
    for name in &det.managed_commands {
        let path = commands_dir.join(name);
        if !path.exists() {
            continue;
        }
        let dest = next_archive_path("commands", &path);
        archive_and_remove(&path, &dest, rb)?;
    }

    let memstack_md = det.project_dir.join(".claude/MEMSTACK.md");
    if memstack_md.exists() {
        let dest = next_archive_path("misc", &memstack_md);
        archive_and_remove(&memstack_md, &dest, rb)?;
    }

    let cfg_local = det.project_dir.join(".claude/config.local.json");
    if cfg_local.exists() {
        let dest = next_archive_path("misc", &cfg_local);
        archive_and_remove(&cfg_local, &dest, rb)?;
    }

    Ok(())
}

fn hook_script_references(settings_path: &Path) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(settings_path) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(hooks) = v.get("hooks").and_then(|h| h.as_object()) {
        for entries in hooks.values() {
            if let Some(arr) = entries.as_array() {
                for entry in arr {
                    if let Some(inner) = entry.get("hooks").and_then(|h| h.as_array()) {
                        for h in inner {
                            if let Some(cmd) = h.get("command").and_then(|c| c.as_str()) {
                                out.push(cmd.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn archive_and_remove(src: &Path, dest: &Path, rb: &mut RollbackManifest) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dest).with_context(|| format!("archiving {}", src.display()))?;
    fs::remove_file(src).with_context(|| format!("removing {}", src.display()))?;
    rb.removed_files.push(RemovedFile {
        original: src.to_path_buf(),
        archived: dest.to_path_buf(),
    });
    Ok(())
}

fn archive_dir_and_remove(src: &Path, dest: &Path, rb: &mut RollbackManifest) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    copy_dir_recursive(src, dest)?;
    fs::remove_dir_all(src).with_context(|| format!("removing {}", src.display()))?;
    rb.removed_files.push(RemovedFile {
        original: src.to_path_buf(),
        archived: dest.to_path_buf(),
    });
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// 3.6 — Re-init v3.
// ----------------------------------------------------------------------------

fn reinit_v3(det: &Detection, migration_id: &str) -> Result<()> {
    ui::info("re-initialising v3 (rtk init → icm init → doctor)");
    integrations::run_all(MemoryProvider::Icm)?;

    if let Err(e) = doctor::run() {
        ui::warn(&format!("doctor reported issues: {e:#}"));
    }

    let manifest_p = WhetstoneManifest::path_for(&det.project_dir);
    let mut manifest = match WhetstoneManifest::load(&manifest_p)
        .context("loading whetstone.json after re-init")?
    {
        Some(m) => m,
        None => WhetstoneManifest::new(MemoryProvider::Icm, config::ToolVersions::default()),
    };
    manifest.set_migration_id(migration_id);
    manifest.touch_and_save(&manifest_p)?;
    ui::ok(&format!("stamped {}", manifest_p.display()));
    Ok(())
}

// ----------------------------------------------------------------------------
// Dry-run printout.
// ----------------------------------------------------------------------------

fn print_dry_run_plan(det: &Detection) {
    println!("---");
    println!("# whetstone migrate — dry-run plan");
    println!();
    println!("## Backups");
    if det.settings_path.exists() {
        println!(
            "- copy {} → .whetstone/migration-<ts>/settings.json.backup",
            det.settings_path.display()
        );
    }
    if let Some(db) = &det.memstack_db {
        println!("- rename {} → {}.v2bak", db.display(), db.display());
        println!("- copy snapshot into archive");
    }
    println!();
    println!("## AutoMem");
    if det.automem_present {
        println!("- remove mcpServers.memory from settings.json (config only)");
        println!("- print decommission notes for external service");
    } else {
        println!("- (nothing to remove)");
    }
    println!();
    println!("## MemStack → ICM");
    if det.memstack_db.is_some() {
        println!("- attempt `icm import --format auto <archive>/icm-import.jsonl`");
        println!("- fall back to per-record `icm store` if bulk fails");
    } else {
        println!("- (no memstack.db)");
    }
    println!();
    println!("## Cleanup");
    println!(
        "- strip {} v2 hook entries from settings.json",
        det.v2_hook_count
    );
    println!(
        "- archive + remove {} skills, {} rules, {} commands",
        det.managed_skills.len(),
        det.managed_rules.len(),
        det.managed_commands.len()
    );
    println!(
        "- archive + remove {} hook scripts",
        det.managed_hooks_on_disk.len()
    );
    println!("- archive + remove MEMSTACK.md: {}", det.memstack_md);
    println!(
        "- archive + remove config.local.json: {}",
        det.config_local_json
    );
    println!();
    println!("## Re-init");
    println!("- `rtk init`");
    println!("- `icm init --mode standard`");
    println!("- `whetstone doctor`");
    println!("- write whetstone.json with migration_id");
    println!("---");
}

// ----------------------------------------------------------------------------
// Tests.
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"").unwrap();
    }

    fn touch_dir(p: &Path) {
        fs::create_dir_all(p).unwrap();
    }

    fn empty_rb(id: &str) -> RollbackManifest {
        RollbackManifest {
            migration_id: id.into(),
            archived_at: Utc::now(),
            settings_backup: None,
            memstack_backup: None,
            automem_backup: None,
            removed_files: Vec::new(),
            removed_hook_indices: Vec::new(),
        }
    }

    #[test]
    fn detection_reports_no_migration_on_empty_project() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let det = detect_at(project.path(), home.path()).unwrap();
        assert!(!det.needs_migration());
        assert_eq!(det.v2_hook_count, 0);
        assert!(!det.automem_present);
        assert!(det.managed_skills.is_empty());
    }

    #[test]
    fn detection_finds_managed_skills_and_rules() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        touch_dir(&project.path().join(".claude/skills/diary"));
        touch_dir(&project.path().join(".claude/skills/user-authored"));
        touch(&project.path().join(".claude/rules/diary.md"));
        touch(&project.path().join(".claude/rules/my-custom.md"));

        let det = detect_at(project.path(), home.path()).unwrap();
        assert_eq!(det.managed_skills, vec!["diary".to_string()]);
        assert_eq!(det.managed_rules, vec!["diary.md".to_string()]);
        assert!(det.needs_migration());
    }

    #[test]
    fn detection_counts_v2_hook_entries() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let settings = json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [{
                            "type": "command",
                            "command": "/Users/x/.claude/hooks/whetstone-pre-push.sh"
                        }]
                    },
                    {
                        "matcher": "Bash",
                        "hooks": [{ "type": "command", "command": "/usr/bin/something-else" }]
                    }
                ],
                "SessionStart": [
                    {
                        "hooks": [{
                            "type": "command",
                            "command": "bash ~/.claude/hooks/whetstone-session-start.sh"
                        }]
                    }
                ]
            }
        });
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        fs::write(
            home.path().join(".claude/settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let det = detect_at(project.path(), home.path()).unwrap();
        assert_eq!(det.v2_hook_count, 2);
        assert!(det.needs_migration());
    }

    #[test]
    fn detection_finds_automem_mcp_entry() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let settings = json!({
            "mcpServers": {
                "memory": {
                    "command": "npx",
                    "args": ["-y", "@verygoodplugins/mcp-automem"],
                    "env": { "AUTOMEM_ENDPOINT": "https://example.com" }
                }
            }
        });
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        fs::write(
            home.path().join(".claude/settings.json"),
            serde_json::to_string(&settings).unwrap(),
        )
        .unwrap();

        let det = detect_at(project.path(), home.path()).unwrap();
        assert!(det.automem_present);
        assert_eq!(det.automem_endpoint.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn entry_is_whetstone_managed_matches_known_scripts() {
        let entry = json!({
            "matcher": "Bash",
            "hooks": [{ "type": "command", "command": "bash /home/x/.claude/hooks/whetstone-session-start.sh" }]
        });
        assert!(entry_is_whetstone_managed(&entry));

        let unrelated = json!({
            "matcher": "Bash",
            "hooks": [{ "type": "command", "command": "rtk pre-bash" }]
        });
        assert!(!entry_is_whetstone_managed(&unrelated));
    }

    #[test]
    fn import_records_assign_high_importance_to_decisions() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../assets/db/schema.sql"))
            .unwrap();
        conn.execute(
            "INSERT INTO insights (project, type, content) VALUES (?1, ?2, ?3)",
            rusqlite::params!["whetstone", "decision", "ship v3 as orchestrator"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO insights (project, type, content) VALUES (?1, ?2, ?3)",
            rusqlite::params!["whetstone", "architecture", "single binary"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO insights (project, type, content) VALUES (?1, ?2, ?3)",
            rusqlite::params!["whetstone", "pattern", "idempotent setup"],
        )
        .unwrap();

        let recs = collect_icm_records(&conn, "20260101-000000").unwrap();
        let dec = recs
            .iter()
            .find(|r| r.content.contains("decision"))
            .unwrap();
        assert_eq!(dec.importance, "high");
        let arch = recs
            .iter()
            .find(|r| r.content.contains("architecture"))
            .unwrap();
        assert_eq!(arch.importance, "critical");
        let pat = recs.iter().find(|r| r.content.contains("pattern")).unwrap();
        assert_eq!(pat.importance, "normal");

        for r in &recs {
            assert!(r.tags.iter().any(|t| t == "source=whetstone-migration"));
            assert!(r.tags.iter().any(|t| t.starts_with("migration-id=")));
        }
    }

    #[test]
    fn cleanup_archives_only_managed_skills_and_keeps_user_skills() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let managed = project.path().join(".claude/skills/diary");
        let user = project.path().join(".claude/skills/my-skill");
        touch_dir(&managed);
        fs::write(managed.join("SKILL.md"), "v2 diary").unwrap();
        touch_dir(&user);
        fs::write(user.join("SKILL.md"), "user wrote this").unwrap();

        fs::create_dir_all(home.path().join(".claude")).unwrap();
        fs::write(home.path().join(".claude/settings.json"), "{}").unwrap();

        let det = detect_at(project.path(), home.path()).unwrap();
        let archive = archive_dir(project.path(), "test");
        fs::create_dir_all(&archive).unwrap();
        let mut rb = empty_rb("test");
        cleanup_managed(&det, &archive, &mut rb).unwrap();

        assert!(!managed.exists(), "managed skill should be removed");
        assert!(user.exists(), "user skill should survive");
        assert_eq!(rb.removed_files.len(), 1);
    }

    #[test]
    fn cleanup_strips_v2_hook_entries_only() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let settings = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "/home/x/.claude/hooks/whetstone-pre-push.sh" }] },
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "rtk pre-bash" }] }
                ]
            }
        });
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let sp = home.path().join(".claude/settings.json");
        fs::write(&sp, serde_json::to_string_pretty(&settings).unwrap()).unwrap();

        let det = detect_at(project.path(), home.path()).unwrap();
        let archive = archive_dir(project.path(), "test");
        fs::create_dir_all(&archive).unwrap();
        let mut rb = empty_rb("test");
        cleanup_managed(&det, &archive, &mut rb).unwrap();

        let after: Value = serde_json::from_str(&fs::read_to_string(&sp).unwrap()).unwrap();
        let arr = after["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(rb
            .removed_hook_indices
            .iter()
            .any(|s| s.contains("PreToolUse")));
    }

    #[test]
    fn export_writes_jsonl_and_markdown() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let db_path = project.path().join(".claude").join(V2_DB_RELATIVE);
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(include_str!("../assets/db/schema.sql"))
            .unwrap();
        conn.execute(
            "INSERT INTO sessions (project, date, accomplished) VALUES (?1, ?2, ?3)",
            rusqlite::params!["whetstone", "2026-06-07", "wrote phase 3"],
        )
        .unwrap();
        drop(conn);

        let det = detect_at(project.path(), home.path()).unwrap();
        let archive = archive_dir(project.path(), "test");
        fs::create_dir_all(&archive).unwrap();
        export_memstack(&det, &archive).unwrap();

        let jsonl = fs::read_to_string(archive.join("memstack-export.jsonl")).unwrap();
        assert!(jsonl.contains("\"kind\":\"session\""));
        assert!(jsonl.contains("phase 3"));
        let md = fs::read_to_string(archive.join("memstack-export.md")).unwrap();
        assert!(md.contains("Sessions"));
    }

    #[test]
    fn project_topic_handles_empty_and_named() {
        assert_eq!(project_topic(""), "whetstone-migration");
        assert_eq!(project_topic("blurt"), "project-blurt");
    }

    #[test]
    fn backup_settings_skips_when_absent() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let det = detect_at(project.path(), home.path()).unwrap();
        let archive = archive_dir(project.path(), "test");
        fs::create_dir_all(&archive).unwrap();
        let mut rb = empty_rb("test");
        backup_settings(&det, &archive, &mut rb).unwrap();
        assert!(rb.settings_backup.is_none());
    }

    #[test]
    fn backup_memstack_renames_to_v2bak() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let db = project.path().join(".claude/db/memstack.db");
        fs::create_dir_all(db.parent().unwrap()).unwrap();
        fs::write(&db, b"fake-sqlite").unwrap();

        let det = detect_at(project.path(), home.path()).unwrap();
        let archive = archive_dir(project.path(), "test");
        fs::create_dir_all(&archive).unwrap();
        let mut rb = empty_rb("test");
        backup_memstack(&det, &archive, &mut rb).unwrap();

        assert!(!db.exists());
        assert!(db.with_extension("db.v2bak").exists());
        assert!(rb.memstack_backup.is_some());
        let archived = rb.memstack_backup.unwrap();
        assert_eq!(fs::read(archived).unwrap(), b"fake-sqlite");
    }

    #[test]
    fn detection_skipped_when_migration_id_already_stamped() {
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        touch_dir(&project.path().join(".claude/skills/diary"));

        // Stamp a fake whetstone.json with migration_id.
        let mp = WhetstoneManifest::path_for(project.path());
        fs::create_dir_all(mp.parent().unwrap()).unwrap();
        let mut m = WhetstoneManifest::new(MemoryProvider::Icm, config::ToolVersions::default());
        m.set_migration_id("20260101-000000");
        m.save(&mp).unwrap();

        let det = detect_at(project.path(), home.path()).unwrap();
        assert!(!det.needs_migration());
        assert_eq!(det.already_migrated.as_deref(), Some("20260101-000000"));
    }

    #[test]
    fn rollback_manifest_round_trips_through_json() {
        // Rollback safety leans entirely on this manifest. Serialize → parse →
        // compare every field so a future field addition doesn't silently get
        // dropped on disk.
        let archived_at: chrono::DateTime<Utc> = "2026-06-07T15:30:12Z".parse().unwrap();
        let rb = RollbackManifest {
            migration_id: "20260607-153012".into(),
            archived_at,
            settings_backup: Some(PathBuf::from(
                ".whetstone/migration-20260607-153012/settings.json.bak",
            )),
            memstack_backup: Some(PathBuf::from(
                ".whetstone/migration-20260607-153012/memstack.db.bak",
            )),
            automem_backup: Some(PathBuf::from(
                ".whetstone/migration-20260607-153012/automem.json.bak",
            )),
            removed_files: vec![RemovedFile {
                original: PathBuf::from(".claude/skills/diary"),
                archived: PathBuf::from(".whetstone/migration-20260607-153012/skills/diary"),
            }],
            removed_hook_indices: vec!["PreToolUse[0]".into(), "Stop[1]".into()],
        };

        let json = serde_json::to_string(&rb).unwrap();
        let parsed: RollbackManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.migration_id, rb.migration_id);
        assert_eq!(parsed.archived_at, rb.archived_at);
        assert_eq!(parsed.settings_backup, rb.settings_backup);
        assert_eq!(parsed.memstack_backup, rb.memstack_backup);
        assert_eq!(parsed.automem_backup, rb.automem_backup);
        assert_eq!(parsed.removed_files.len(), 1);
        assert_eq!(
            parsed.removed_files[0].original,
            rb.removed_files[0].original
        );
        assert_eq!(
            parsed.removed_files[0].archived,
            rb.removed_files[0].archived
        );
        assert_eq!(parsed.removed_hook_indices, rb.removed_hook_indices);
    }

    #[test]
    fn entry_is_whetstone_managed_rejects_unmanaged_scripts() {
        // The negative case keeps cleanup honest: a user-authored hook with
        // an unrelated absolute path must not be classified as managed.
        let entry = json!({
            "matcher": "Bash",
            "hooks": [{
                "type": "command",
                "command": "/home/user/.local/bin/my-custom-hook.sh"
            }]
        });
        assert!(!entry_is_whetstone_managed(&entry));
    }

    #[test]
    fn cleanup_managed_is_idempotent_on_second_run() {
        // Running cleanup over already-cleaned state must be a no-op — the
        // rollback story depends on cleanup being safely re-runnable.
        let project = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let settings = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "/home/x/.claude/hooks/whetstone-pre-push.sh" }] },
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "/usr/local/bin/keep-me.sh" }] }
                ]
            }
        });
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let sp = home.path().join(".claude/settings.json");
        fs::write(&sp, serde_json::to_string_pretty(&settings).unwrap()).unwrap();

        // First pass strips the v2 entry.
        let det1 = detect_at(project.path(), home.path()).unwrap();
        let archive = archive_dir(project.path(), "test");
        fs::create_dir_all(&archive).unwrap();
        let mut rb1 = empty_rb("test");
        cleanup_managed(&det1, &archive, &mut rb1).unwrap();
        let after_first = fs::read_to_string(&sp).unwrap();
        assert!(!rb1.removed_hook_indices.is_empty());

        // Second pass on the same project/home detects nothing to clean and
        // leaves settings.json byte-identical.
        let det2 = detect_at(project.path(), home.path()).unwrap();
        let mut rb2 = empty_rb("test");
        cleanup_managed(&det2, &archive, &mut rb2).unwrap();
        let after_second = fs::read_to_string(&sp).unwrap();
        assert_eq!(
            after_first, after_second,
            "settings.json must be unchanged on the idempotent second cleanup pass"
        );
        assert!(
            rb2.removed_hook_indices.is_empty(),
            "second pass should record no removed hook indices, got {:?}",
            rb2.removed_hook_indices
        );
    }
}
