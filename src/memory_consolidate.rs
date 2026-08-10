//! One-time consolidation of stray per-project `.headroom` memory stores into
//! the global `~/.headroom` root.
//!
//! Whetstone runs Headroom as a single shared proxy on a fixed port, but
//! Headroom's default memory root is `{cwd}/.headroom`. Because the proxy is
//! launched from whichever project starts it first, that one project's
//! `.headroom` accumulates the per-project memory DBs of *every* project that
//! routes through the proxy (GH-style cross-project litter). This module drains
//! such a stray store into the global root, keyed by Headroom's already-unique
//! `<basename>-<hash>` project folder names, and never overwrites existing
//! global data.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Top-level legacy/seed DB files Headroom writes at the store root.
const SEED_DB_FILES: &[&str] =
    &["memory.db", "memory_graph.db", "memory_vectors.db"];

/// Top-level entries that are safe to discard when draining a stray store.
const IGNORABLE_TOP: &[&str] = &[".keep", "memories"];

/// The outcome of a consolidation pass, used for reporting and tests.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ConsolidationReport {
    /// Per-project folder names moved into the global root.
    pub moved_projects: Vec<String>,
    /// Top-level seed DB files moved into the global root.
    pub moved_seeds: Vec<String>,
    /// Entries skipped because the destination already existed (no overwrite).
    pub conflicts: Vec<String>,
    /// Whether the drained stray `.headroom` dir was (or would be) removed.
    pub removed_stray: bool,
}

impl ConsolidationReport {
    /// Nothing was moved, skipped, or removed — the common steady state.
    pub fn is_noop(&self) -> bool {
        self.moved_projects.is_empty()
            && self.moved_seeds.is_empty()
            && self.conflicts.is_empty()
            && !self.removed_stray
    }

    /// Whether this pass actually changed anything on disk. Distinguishes real
    /// progress from a stale conflict-only report, so the auto path can stay
    /// quiet once the litter is gone but a legacy seed DB lingers.
    pub fn changed_anything(&self) -> bool {
        !self.moved_projects.is_empty()
            || !self.moved_seeds.is_empty()
            || self.removed_stray
    }

    /// A one-line-per-item human summary, or `None` for a no-op.
    pub fn describe(&self, stray_root: &Path) -> Option<String> {
        if self.is_noop() {
            return None;
        }
        let mut lines = vec![format!(
            "consolidated stray Headroom memory at {}",
            stray_root.display()
        )];
        if !self.moved_projects.is_empty() {
            lines.push(format!(
                "  moved {} project store(s): {}",
                self.moved_projects.len(),
                self.moved_projects.join(", ")
            ));
        }
        if !self.moved_seeds.is_empty() {
            lines.push(format!(
                "  moved seed DB(s): {}",
                self.moved_seeds.join(", ")
            ));
        }
        if !self.conflicts.is_empty() {
            lines.push(format!(
                "  left {} in place (already present globally): {}",
                self.conflicts.len(),
                self.conflicts.join(", ")
            ));
        }
        if self.removed_stray {
            lines.push(
                "  removed the now-empty stray .headroom dir".to_string(),
            );
        }
        Some(lines.join("\n"))
    }
}

/// Consolidate a stray `.headroom` store at `stray_root` into `global_root`.
///
/// Conservative and idempotent:
/// - no-op if `stray_root` is missing or *is* `global_root`;
/// - a project/seed is moved only when its destination doesn't already exist;
///   otherwise it's left in place and reported as a conflict (never
///   overwritten — no memory is destroyed);
/// - the stray dir is removed only once fully drained, and never if it holds
///   entries we don't recognize.
///
/// When `dry_run` is true nothing is written; the report describes what *would*
/// happen.
pub fn consolidate(
    stray_root: &Path,
    global_root: &Path,
    dry_run: bool,
) -> Result<ConsolidationReport> {
    let mut report = ConsolidationReport::default();

    if !stray_root.exists() {
        return Ok(report);
    }
    // Never consolidate the global store into itself (e.g. cwd == $HOME).
    if same_path(stray_root, global_root) {
        return Ok(report);
    }

    move_project_stores(stray_root, global_root, dry_run, &mut report)?;
    move_seed_dbs(stray_root, global_root, dry_run, &mut report)?;

    // Remove the stray dir only when everything moved cleanly and nothing
    // unfamiliar is left behind.
    let fully_drained =
        report.conflicts.is_empty() && !has_foreign_entries(stray_root);
    if fully_drained {
        if !dry_run {
            fs::remove_dir_all(stray_root).with_context(|| {
                format!("removing {}", stray_root.display())
            })?;
        }
        report.removed_stray = true;
    }

    Ok(report)
}

/// Move each `memories/projects/<name>/` store into the global root.
fn move_project_stores(
    stray_root: &Path,
    global_root: &Path,
    dry_run: bool,
    report: &mut ConsolidationReport,
) -> Result<()> {
    let stray_projects = stray_root.join("memories").join("projects");
    let global_projects = global_root.join("memories").join("projects");
    if !stray_projects.is_dir() {
        return Ok(());
    }

    let mut names: Vec<PathBuf> = fs::read_dir(&stray_projects)
        .with_context(|| format!("reading {}", stray_projects.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    names.sort(); // deterministic order for reporting/tests

    for src in names {
        let name = src.file_name().unwrap_or_default();
        let label = name.to_string_lossy().to_string();
        let dest = global_projects.join(name);
        if dest.exists() {
            report.conflicts.push(label);
            continue;
        }
        if !dry_run {
            fs::create_dir_all(&global_projects)?;
            move_path(&src, &dest)?;
        }
        report.moved_projects.push(label);
    }
    Ok(())
}

/// Move top-level legacy/seed DB files into the global root, all-or-nothing.
///
/// The three seed DBs are one coherent store (a `memory.db` plus its graph and
/// vector shards). If *any* already exists globally we skip the whole set —
/// migrating only some shards would splice them onto a different global
/// `memory.db` and corrupt the store. Skipped files are recorded as conflicts
/// so the stray dir is retained rather than deleted with data still in it.
fn move_seed_dbs(
    stray_root: &Path,
    global_root: &Path,
    dry_run: bool,
    report: &mut ConsolidationReport,
) -> Result<()> {
    let present: Vec<&str> = SEED_DB_FILES
        .iter()
        .copied()
        .filter(|f| stray_root.join(f).is_file())
        .collect();
    if present.is_empty() {
        return Ok(());
    }

    if present.iter().any(|f| global_root.join(f).exists()) {
        report
            .conflicts
            .extend(present.iter().map(|f| f.to_string()));
        return Ok(());
    }

    for file in present {
        if !dry_run {
            fs::create_dir_all(global_root)?;
            move_path(&stray_root.join(file), &global_root.join(file))?;
        }
        report.moved_seeds.push(file.to_string());
    }
    Ok(())
}

/// Whether `stray_root` holds entries we don't recognize — if so, removing it
/// could destroy data we didn't consolidate, so we leave it in place.
fn has_foreign_entries(stray_root: &Path) -> bool {
    let top_foreign = dir_has_unexpected(stray_root, |name| {
        IGNORABLE_TOP.contains(&name) || SEED_DB_FILES.contains(&name)
    });
    let memories = stray_root.join("memories");
    let mem_foreign = memories.is_dir()
        && dir_has_unexpected(&memories, |name| name == "projects");
    top_foreign || mem_foreign
}

/// True if `dir` contains any entry whose name `allowed` rejects.
fn dir_has_unexpected(dir: &Path, allowed: impl Fn(&str) -> bool) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !allowed(&name.to_string_lossy()) {
            return true;
        }
    }
    false
}

/// Move a file or directory, falling back to copy+remove across filesystems.
fn move_path(src: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(src, dest).is_ok() {
        return Ok(());
    }
    // Cross-device rename fails with EXDEV; copy then delete instead.
    if src.is_dir() {
        copy_dir_recursive(src, dest)?;
        fs::remove_dir_all(src)?;
    } else {
        fs::copy(src, dest)?;
        fs::remove_file(src)?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

/// Compare two paths for identity, tolerating not-yet-existing globals.
fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

/// The global Headroom store root whetstone pins memory to (`~/.headroom`).
pub fn global_root() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".headroom"))
}

/// The global memory DB path whetstone hands Headroom via
/// `HEADROOM_MEMORY_DB_PATH`, so every project's per-project store lands under
/// one home-dir root instead of littering each project's working directory.
pub fn global_memory_db_path() -> Option<PathBuf> {
    Some(global_root()?.join("memory.db"))
}

/// Auto-consolidate the current directory's stray `.headroom` store into the
/// global root. Conservative + idempotent; a silent no-op when nothing's stray,
/// and a soft warning (never fatal) on error — launch must not be blocked.
pub fn auto_consolidate_cwd() {
    let (Some(cwd), Some(global)) =
        (std::env::current_dir().ok(), global_root())
    else {
        return;
    };
    let stray = cwd.join(".headroom");
    match consolidate(&stray, &global, false) {
        // Only speak up when we actually moved something — a stale seed-DB
        // conflict alone shouldn't nag on every launch.
        Ok(report) if report.changed_anything() => {
            if let Some(summary) = report.describe(&stray) {
                crate::ui::info(&summary);
            }
        }
        Ok(_) => {}
        Err(e) => crate::ui::warn(&format!(
            "memory store consolidation skipped: {e:#}"
        )),
    }
}

/// `whetstone memory consolidate [--dry-run]`: explicitly drain the current
/// project's stray `.headroom` store into the global root.
pub fn run_command(dry_run: bool) -> Result<()> {
    let cwd = std::env::current_dir().context("determining current dir")?;
    let global = global_root()
        .context("could not determine home directory for ~/.headroom")?;
    let stray = cwd.join(".headroom");

    let report = consolidate(&stray, &global, dry_run)?;
    match report.describe(&stray) {
        Some(summary) => {
            if dry_run {
                crate::ui::info(&format!(
                    "[dry-run] would consolidate:\n{summary}"
                ));
            } else {
                crate::ui::ok(&summary);
            }
        }
        None => crate::ui::info(&format!(
            "no stray Headroom memory to consolidate at {}",
            stray.display()
        )),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn project_store(root: &Path, name: &str, body: &str) {
        write(
            &root
                .join("memories")
                .join("projects")
                .join(name)
                .join("memory.db"),
            body,
        );
    }

    #[test]
    fn missing_stray_is_noop() {
        let tmp = TempDir::new().unwrap();
        let report = consolidate(
            &tmp.path().join("nope/.headroom"),
            &tmp.path().join("global/.headroom"),
            false,
        )
        .unwrap();
        assert!(report.is_noop());
    }

    #[test]
    fn stray_equal_to_global_is_noop() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".headroom");
        project_store(&root, "whetstone-abc", "x");
        let report = consolidate(&root, &root, false).unwrap();
        assert!(report.is_noop());
        // Global-store contents must be untouched.
        assert!(root
            .join("memories/projects/whetstone-abc/memory.db")
            .exists());
    }

    #[test]
    fn moves_foreign_project_stores_into_global() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("whetstone/.headroom");
        let global = tmp.path().join("home/.headroom");
        project_store(&stray, "blurt-111", "blurt-mem");
        project_store(&stray, "waybar-222", "waybar-mem");

        let report = consolidate(&stray, &global, false).unwrap();

        assert_eq!(report.moved_projects, vec!["blurt-111", "waybar-222"]);
        assert!(report.conflicts.is_empty());
        assert!(report.removed_stray);
        assert!(!stray.exists(), "drained stray should be removed");
        assert_eq!(
            fs::read_to_string(
                global.join("memories/projects/blurt-111/memory.db")
            )
            .unwrap(),
            "blurt-mem"
        );
    }

    #[test]
    fn conflict_is_skipped_not_overwritten() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("whetstone/.headroom");
        let global = tmp.path().join("home/.headroom");
        project_store(&stray, "polycule-abc", "stray-version");
        project_store(&global, "polycule-abc", "global-version");

        let report = consolidate(&stray, &global, false).unwrap();

        assert_eq!(report.conflicts, vec!["polycule-abc"]);
        assert!(report.moved_projects.is_empty());
        assert!(!report.removed_stray, "stray retained on conflict");
        assert!(stray.exists());
        // Existing global memory is preserved, stray copy untouched.
        assert_eq!(
            fs::read_to_string(
                global.join("memories/projects/polycule-abc/memory.db")
            )
            .unwrap(),
            "global-version"
        );
        assert!(stray
            .join("memories/projects/polycule-abc/memory.db")
            .exists());
    }

    #[test]
    fn moves_seed_dbs_when_absent_globally() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("proj/.headroom");
        let global = tmp.path().join("home/.headroom");
        write(&stray.join("memory.db"), "seed");
        write(&stray.join("memory_vectors.db"), "vec");

        let report = consolidate(&stray, &global, false).unwrap();

        assert_eq!(report.moved_seeds, vec!["memory.db", "memory_vectors.db"]);
        assert!(report.removed_stray);
        assert_eq!(
            fs::read_to_string(global.join("memory.db")).unwrap(),
            "seed"
        );
    }

    #[test]
    fn seed_conflict_skips_whole_set_all_or_nothing() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("proj/.headroom");
        let global = tmp.path().join("home/.headroom");
        // Stray has the full seed set; global already owns memory.db only.
        write(&stray.join("memory.db"), "stray");
        write(&stray.join("memory_graph.db"), "graph");
        write(&stray.join("memory_vectors.db"), "vectors");
        write(&global.join("memory.db"), "headroom-owned");

        let report = consolidate(&stray, &global, false).unwrap();

        // No shard is spliced onto the foreign global memory.db.
        assert!(report.moved_seeds.is_empty());
        assert_eq!(
            report.conflicts,
            vec!["memory.db", "memory_graph.db", "memory_vectors.db"]
        );
        assert!(!report.removed_stray);
        assert_eq!(
            fs::read_to_string(global.join("memory.db")).unwrap(),
            "headroom-owned"
        );
        assert!(!global.join("memory_graph.db").exists());
        assert!(stray.join("memory_graph.db").exists());
    }

    #[test]
    fn projects_migrate_even_when_seed_set_conflicts() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("proj/.headroom");
        let global = tmp.path().join("home/.headroom");
        project_store(&stray, "blurt-1", "m");
        write(&stray.join("memory.db"), "stray");
        write(&global.join("memory.db"), "headroom-owned");

        let report = consolidate(&stray, &global, false).unwrap();

        // The cross-project litter is consolidated regardless of the seed DB.
        assert_eq!(report.moved_projects, vec!["blurt-1"]);
        assert!(report.conflicts.contains(&"memory.db".to_string()));
        assert!(!report.removed_stray, "legacy seed DB retained");
        assert!(report.changed_anything());
        assert!(global.join("memories/projects/blurt-1/memory.db").exists());
    }

    #[test]
    fn conflict_only_report_reports_no_change() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("proj/.headroom");
        let global = tmp.path().join("home/.headroom");
        write(&stray.join("memory.db"), "stray");
        write(&global.join("memory.db"), "headroom-owned");

        let report = consolidate(&stray, &global, false).unwrap();

        assert!(!report.is_noop(), "there is a conflict to report");
        assert!(!report.changed_anything(), "but nothing moved on disk");
    }

    #[test]
    fn foreign_entries_block_removal() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("proj/.headroom");
        let global = tmp.path().join("home/.headroom");
        project_store(&stray, "x-1", "m");
        write(&stray.join("mystery.txt"), "keep me");

        let report = consolidate(&stray, &global, false).unwrap();

        assert_eq!(report.moved_projects, vec!["x-1"]);
        assert!(!report.removed_stray, "unknown file blocks dir removal");
        assert!(stray.join("mystery.txt").exists());
    }

    #[test]
    fn keep_and_empty_dirs_do_not_block_removal() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("proj/.headroom");
        let global = tmp.path().join("home/.headroom");
        project_store(&stray, "x-1", "m");
        write(&stray.join(".keep"), "");

        let report = consolidate(&stray, &global, false).unwrap();

        assert!(report.removed_stray);
        assert!(!stray.exists());
    }

    #[test]
    fn dry_run_writes_nothing_but_reports() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("proj/.headroom");
        let global = tmp.path().join("home/.headroom");
        project_store(&stray, "x-1", "m");

        let report = consolidate(&stray, &global, true).unwrap();

        assert_eq!(report.moved_projects, vec!["x-1"]);
        assert!(report.removed_stray, "would remove after draining");
        // But nothing actually changed on disk.
        assert!(stray.join("memories/projects/x-1/memory.db").exists());
        assert!(!global.exists());
    }

    #[test]
    fn idempotent_second_run_is_noop() {
        let tmp = TempDir::new().unwrap();
        let stray = tmp.path().join("proj/.headroom");
        let global = tmp.path().join("home/.headroom");
        project_store(&stray, "x-1", "m");

        consolidate(&stray, &global, false).unwrap();
        let second = consolidate(&stray, &global, false).unwrap();
        assert!(second.is_noop());
    }
}
