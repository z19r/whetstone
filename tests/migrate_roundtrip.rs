//! Integration test — drives the actual compiled `whetstone migrate` binary
//! against a v2 fixture project on disk.
//!
//! Scope:
//! - `migrate --dry-run` over a v2 fixture detects every marker and writes nothing.
//! - `migrate -y` over a no-v2-markers project short-circuits cleanly.
//! - `migrate --rollback <unknown-id>` errors with a clean diagnostic.
//!
//! Full `migrate -y` + `--rollback` round-trip needs `icm` + `rtk` on PATH and
//! belongs in a nightly CI job, not this fast unit-test pass.
//!
//! Windows path: the migration flow assumes Unix `.sh` hook scripts and a
//! single `$HOME` resolution. On Windows we only assert the binary doesn't
//! crash on a clean project — covered by `windows_clean_project_no_op` below.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn whetstone_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_whetstone"))
}

fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn run_migrate(project: &Path, home: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new(whetstone_bin());
    cmd.arg("migrate").args(args);
    cmd.current_dir(project);
    cmd.env("HOME", home);
    cmd.env("WHETSTONE_ASSETS", assets_dir());
    cmd.env("NO_COLOR", "1");
    cmd.output().expect("whetstone migrate failed to spawn")
}

#[cfg(not(target_os = "windows"))]
fn seed_v2_fixture(project: &Path, home: &Path) {
    // Project-side: memstack DB + MEMSTACK.md + a managed skill dir.
    let memstack_dir = project.join(".claude/memstack/db");
    std::fs::create_dir_all(&memstack_dir).unwrap();
    std::fs::write(memstack_dir.join("memstack.db"), b"sqlite-shaped-bytes").unwrap();
    std::fs::write(project.join(".claude/MEMSTACK.md"), b"# memstack v2").unwrap();
    let skills_diary = project.join(".claude/skills/diary");
    std::fs::create_dir_all(&skills_diary).unwrap();
    std::fs::write(skills_diary.join("SKILL.md"), b"# diary").unwrap();

    // Home-side: a v2 settings.json with whetstone-managed hook scripts and an
    // AutoMem mcpServers block.
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(claude_dir.join("hooks")).unwrap();
    let hook_script = claude_dir.join("hooks/pre-tool-notify.sh");
    std::fs::write(&hook_script, b"#!/bin/sh\n").unwrap();

    let settings = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": hook_script.display().to_string()
                }]
            }]
        },
        "mcpServers": {
            "memory": {
                "command": "npx",
                "args": ["@verygoodplugins/mcp-automem"],
                "env": {}
            }
        }
    });
    std::fs::write(
        claude_dir.join("settings.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .unwrap();
}

#[cfg(not(target_os = "windows"))]
#[test]
fn dry_run_over_v2_fixture_reports_markers_and_writes_nothing() {
    let project = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    seed_v2_fixture(project.path(), home.path());

    let out = run_migrate(project.path(), home.path(), &["--dry-run"]);
    assert!(
        out.status.success(),
        "migrate --dry-run failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("v2 markers detected"),
        "expected detection banner, got:\n{combined}"
    );
    assert!(
        combined.contains("dry-run") || combined.contains("--dry-run"),
        "expected dry-run notice, got:\n{combined}"
    );

    // Dry-run must not touch the project's archive directory or the manifest.
    assert!(
        !project.path().join(".whetstone").exists(),
        ".whetstone/ should not exist after dry-run"
    );
    assert!(
        !project.path().join(".claude/whetstone.json").exists(),
        ".claude/whetstone.json should not exist after dry-run"
    );
    let after = std::fs::read_to_string(home.path().join(".claude/settings.json")).unwrap();
    assert!(
        after.contains("mcp-automem"),
        "settings.json was mutated during dry-run"
    );
}

#[cfg(not(target_os = "windows"))]
#[test]
fn migrate_on_clean_project_is_a_no_op() {
    let project = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();

    let out = run_migrate(project.path(), home.path(), &["-y"]);
    assert!(
        out.status.success(),
        "migrate on a clean project should succeed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("no v2 markers") || combined.contains("nothing to do"),
        "expected no-op message, got:\n{combined}"
    );
    assert!(
        !project.path().join(".whetstone").exists(),
        ".whetstone/ should not be created on a clean project"
    );
}

#[cfg(not(target_os = "windows"))]
#[test]
fn rollback_with_unknown_id_errors_cleanly() {
    let project = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();

    let out = run_migrate(
        project.path(),
        home.path(),
        &["--rollback", "19990101-000000"],
    );
    assert!(
        !out.status.success(),
        "rollback of an unknown migration id should fail"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("no archive") || combined.contains("19990101-000000"),
        "expected diagnostic mentioning the missing archive, got:\n{combined}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_clean_project_no_op() {
    // On Windows the migration assumes Unix-y hooks ($HOME/.claude/hooks/*.sh)
    // and POSIX paths in settings.json, so we only check the binary doesn't
    // crash on a clean project; full migration is gated to Unix in
    // `setup`/`update` upstream of this code path.
    let project = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();

    let out = run_migrate(project.path(), home.path(), &["-y"]);
    assert!(
        out.status.success(),
        "windows: migrate on a clean project must not crash: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
