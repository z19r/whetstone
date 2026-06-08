//! `whetstone doctor` — inspect and normalize the Claude Code settings after
//! the tools' own `init` commands have run.
//!
//! Whetstone v3 never writes hooks into `~/.claude/settings.json` directly
//! (that is `rtk init` and `icm init`'s job). What it does do is sanity-check
//! the result and apply two tightly-scoped normalizations:
//!
//! 1. RTK's `PreToolUse` Bash hook must sit **last** in the `PreToolUse`
//!    array — every other Bash hook should observe the unrewritten command.
//! 2. ICM's hooks must be present and well-formed.
//!
//! Anything else found in `settings.json` (custom user hooks, other MCP
//! servers, model preferences, etc.) is left alone.

use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ui;

/// Status of a single check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Everything as expected.
    Ok,
    /// Found a problem and fixed it in-place.
    Normalized(String),
    /// Found a problem we cannot or should not fix automatically.
    Warning(String),
}

/// Result of inspecting one aspect of settings.json.
#[derive(Debug, Clone)]
pub struct Finding {
    pub label: String,
    pub status: Status,
}

/// Aggregate result of a `whetstone doctor` pass.
#[derive(Debug, Default)]
pub struct DoctorReport {
    pub findings: Vec<Finding>,
    pub mutated: bool,
}

impl DoctorReport {
    pub fn green(&self) -> bool {
        self.findings
            .iter()
            .all(|f| !matches!(f.status, Status::Warning(_)))
    }

    fn push(&mut self, label: &str, status: Status) {
        if matches!(status, Status::Normalized(_)) {
            self.mutated = true;
        }
        self.findings.push(Finding {
            label: label.into(),
            status,
        });
    }
}

/// Entry point for `whetstone doctor`. Returns the report so callers (setup,
/// tests, future TUI) can examine it; also prints a human-readable summary.
pub fn run() -> Result<DoctorReport> {
    let settings_path = settings_path()?;
    let report = inspect_and_normalize(&settings_path)?;
    print_report(&report);
    Ok(report)
}

fn settings_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".claude").join("settings.json"))
}

/// Pure logic exposed for tests: read settings.json, run every check, write
/// back only if something was normalized.
pub(crate) fn inspect_and_normalize(settings_path: &Path) -> Result<DoctorReport> {
    let mut report = DoctorReport::default();

    if !settings_path.exists() {
        report.push(
            "settings.json",
            Status::Warning(format!(
                "missing at {} — run `whetstone setup` first",
                settings_path.display()
            )),
        );
        return Ok(report);
    }

    let raw = fs::read_to_string(settings_path)
        .with_context(|| format!("reading {}", settings_path.display()))?;
    let mut settings: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            report.push(
                "settings.json",
                Status::Warning(format!("invalid JSON: {e}")),
            );
            return Ok(report);
        }
    };

    check_rtk_hook(&mut settings, &mut report);
    check_icm_hooks(&settings, &mut report);

    if report.mutated {
        backup_then_write(settings_path, &settings)?;
    }

    Ok(report)
}

/// Ensure RTK's `PreToolUse` Bash hook is present and is the LAST entry in
/// the `PreToolUse` array. If RTK's entry is present but not last, reorder
/// in place and record a `Normalized` finding.
fn check_rtk_hook(settings: &mut Value, report: &mut DoctorReport) {
    let pre = match settings
        .get_mut("hooks")
        .and_then(|h| h.get_mut("PreToolUse"))
        .and_then(|p| p.as_array_mut())
    {
        Some(arr) => arr,
        None => {
            report.push(
                "rtk PreToolUse hook",
                Status::Warning("hooks.PreToolUse missing — did `rtk init` run?".into()),
            );
            return;
        }
    };

    let rtk_indices: Vec<usize> = pre
        .iter()
        .enumerate()
        .filter(|(_, entry)| is_rtk_entry(entry))
        .map(|(i, _)| i)
        .collect();

    match rtk_indices.as_slice() {
        [] => report.push(
            "rtk PreToolUse hook",
            Status::Warning("not found — `rtk init --auto-patch` may have failed".into()),
        ),
        &[idx] => {
            let last = pre.len() - 1;
            if idx == last {
                report.push("rtk PreToolUse hook", Status::Ok);
            } else {
                let entry = pre.remove(idx);
                pre.push(entry);
                report.push(
                    "rtk PreToolUse hook",
                    Status::Normalized(format!("moved from index {idx} to {last}")),
                );
            }
        }
        many => {
            // Dedupe: keep one, drop the rest, ensure survivor sits last.
            let dup_count = many.len();
            for idx in many.iter().rev().skip(1) {
                pre.remove(*idx);
            }
            let new_idx = pre
                .iter()
                .position(is_rtk_entry)
                .expect("RTK entry survived dedupe");
            let entry = pre.remove(new_idx);
            pre.push(entry);
            report.push(
                "rtk PreToolUse hook",
                Status::Normalized(format!("found {dup_count} duplicates, kept one at end")),
            );
        }
    }
}

/// Confirm at least one hook entry references `icm` somewhere. Surface a
/// warning otherwise so the operator knows to re-run `icm init`.
fn check_icm_hooks(settings: &Value, report: &mut DoctorReport) {
    let hooks = match settings.get("hooks") {
        Some(h) => h,
        None => {
            report.push(
                "icm hooks",
                Status::Warning("hooks block missing entirely".into()),
            );
            return;
        }
    };

    let mut events_with_icm: Vec<String> = Vec::new();
    if let Some(obj) = hooks.as_object() {
        for (event, entries) in obj {
            if let Some(arr) = entries.as_array() {
                if arr.iter().any(entry_mentions_icm) {
                    events_with_icm.push(event.clone());
                }
            }
        }
    }

    if events_with_icm.is_empty() {
        report.push(
            "icm hooks",
            Status::Warning(
                "no entries reference icm — `icm init --mode standard` may be needed".into(),
            ),
        );
    } else {
        events_with_icm.sort();
        report.push("icm hooks", Status::Ok);
        ui::info(&format!(
            "  icm wired across: {}",
            events_with_icm.join(", ")
        ));
    }
}

fn is_rtk_entry(entry: &Value) -> bool {
    if entry.get("matcher").and_then(Value::as_str) != Some("Bash") {
        return false;
    }
    let hooks = match entry.get("hooks").and_then(Value::as_array) {
        Some(h) => h,
        None => return false,
    };
    hooks.iter().any(hook_command_is_rtk)
}

fn hook_command_is_rtk(hook: &Value) -> bool {
    if hook.get("type").and_then(Value::as_str) != Some("command") {
        return false;
    }
    let cmd = match hook.get("command").and_then(Value::as_str) {
        Some(c) => c,
        None => return false,
    };
    let stripped = match cmd.strip_suffix(" hook claude") {
        Some(s) => s,
        None => return false,
    };
    let program = stripped.trim().trim_matches('"');
    Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == rtk_binary_name())
}

fn rtk_binary_name() -> &'static str {
    if cfg!(windows) {
        "rtk.exe"
    } else {
        "rtk"
    }
}

fn entry_mentions_icm(entry: &Value) -> bool {
    let hooks = match entry.get("hooks").and_then(Value::as_array) {
        Some(h) => h,
        None => return false,
    };
    hooks.iter().any(|h| {
        h.get("command")
            .and_then(Value::as_str)
            .is_some_and(command_invokes_icm)
    })
}

fn command_invokes_icm(cmd: &str) -> bool {
    cmd.split_whitespace().any(|tok| {
        let unquoted = tok.trim_matches('"').trim_matches('\'');
        Path::new(unquoted)
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "icm" || n == "icm.exe")
    })
}

fn backup_then_write(settings_path: &Path, settings: &Value) -> Result<()> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup = settings_path.with_file_name(format!("settings.json.bak.{ts}"));
    fs::copy(settings_path, &backup)
        .with_context(|| format!("backing up {}", settings_path.display()))?;
    let pretty = serde_json::to_string_pretty(settings).context("serializing settings.json")?;
    fs::write(settings_path, pretty)
        .with_context(|| format!("writing {}", settings_path.display()))?;
    Ok(())
}

fn print_report(report: &DoctorReport) {
    ui::section("whetstone doctor");
    for finding in &report.findings {
        match &finding.status {
            Status::Ok => ui::ok(&finding.label),
            Status::Normalized(detail) => {
                ui::ok(&format!("{}: normalized ({detail})", finding.label));
            }
            Status::Warning(detail) => {
                ui::warn(&format!("{}: {detail}", finding.label));
            }
        }
    }
    if report.green() {
        ui::ok("doctor: green");
    } else {
        ui::warn("doctor: warnings present — see above");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_settings(v: Value) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(serde_json::to_string(&v).unwrap().as_bytes())
            .unwrap();
        f
    }

    fn rtk_entry() -> Value {
        json!({
            "matcher": "Bash",
            "hooks": [{
                "type": "command",
                "command": "/home/u/.local/bin/rtk hook claude",
            }]
        })
    }

    fn other_bash_entry() -> Value {
        json!({
            "matcher": "Bash",
            "hooks": [{
                "type": "command",
                "command": "/usr/local/bin/some-other hook pretool",
            }]
        })
    }

    #[test]
    fn missing_settings_is_warning() {
        let path = Path::new("/nonexistent-whetstone-test/settings.json");
        let report = inspect_and_normalize(path).unwrap();
        assert!(matches!(report.findings[0].status, Status::Warning(_)));
    }

    #[test]
    fn rtk_already_last_reports_ok() {
        let f = write_settings(json!({
            "hooks": {
                "PreToolUse": [other_bash_entry(), rtk_entry()]
            }
        }));
        let report = inspect_and_normalize(f.path()).unwrap();
        assert!(report
            .findings
            .iter()
            .any(|x| x.label == "rtk PreToolUse hook" && matches!(x.status, Status::Ok)));
        assert!(!report.mutated);
    }

    #[test]
    fn rtk_first_gets_moved_to_last() {
        let f = write_settings(json!({
            "hooks": {
                "PreToolUse": [rtk_entry(), other_bash_entry()]
            }
        }));
        let report = inspect_and_normalize(f.path()).unwrap();
        assert!(report.mutated);
        let rewritten: Value =
            serde_json::from_str(&fs::read_to_string(f.path()).unwrap()).unwrap();
        let pre = rewritten["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert!(is_rtk_entry(&pre[1]));
        assert!(!is_rtk_entry(&pre[0]));
    }

    #[test]
    fn missing_rtk_is_warning() {
        let f = write_settings(json!({
            "hooks": {
                "PreToolUse": [other_bash_entry()]
            }
        }));
        let report = inspect_and_normalize(f.path()).unwrap();
        let rtk_finding = report
            .findings
            .iter()
            .find(|x| x.label == "rtk PreToolUse hook")
            .unwrap();
        assert!(matches!(rtk_finding.status, Status::Warning(_)));
    }

    #[test]
    fn duplicate_rtk_entries_get_deduped() {
        let f = write_settings(json!({
            "hooks": {
                "PreToolUse": [rtk_entry(), other_bash_entry(), rtk_entry()]
            }
        }));
        let report = inspect_and_normalize(f.path()).unwrap();
        assert!(report.mutated);
        let rewritten: Value =
            serde_json::from_str(&fs::read_to_string(f.path()).unwrap()).unwrap();
        let pre = rewritten["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert!(is_rtk_entry(&pre[1]));
    }

    #[test]
    fn icm_hooks_detected_when_present() {
        let f = write_settings(json!({
            "hooks": {
                "PreToolUse": [rtk_entry()],
                "SessionStart": [{
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/local/bin/icm hook session-start"
                    }]
                }]
            }
        }));
        let report = inspect_and_normalize(f.path()).unwrap();
        let icm_finding = report
            .findings
            .iter()
            .find(|x| x.label == "icm hooks")
            .unwrap();
        assert!(matches!(icm_finding.status, Status::Ok));
    }

    #[test]
    fn missing_icm_is_warning() {
        let f = write_settings(json!({
            "hooks": {
                "PreToolUse": [rtk_entry()]
            }
        }));
        let report = inspect_and_normalize(f.path()).unwrap();
        let icm_finding = report
            .findings
            .iter()
            .find(|x| x.label == "icm hooks")
            .unwrap();
        assert!(matches!(icm_finding.status, Status::Warning(_)));
    }

    #[test]
    fn unrelated_keys_left_alone() {
        let f = write_settings(json!({
            "model": "claude-opus-4-7",
            "mcpServers": {"foo": {"command": "bar"}},
            "hooks": {
                "PreToolUse": [other_bash_entry(), rtk_entry()]
            }
        }));
        inspect_and_normalize(f.path()).unwrap();
        let rewritten: Value =
            serde_json::from_str(&fs::read_to_string(f.path()).unwrap()).unwrap();
        assert_eq!(rewritten["model"], "claude-opus-4-7");
        assert!(rewritten["mcpServers"].is_object());
    }

    #[test]
    fn green_when_no_warnings() {
        let mut r = DoctorReport::default();
        r.push("a", Status::Ok);
        r.push("b", Status::Normalized("did stuff".into()));
        assert!(r.green());
    }

    #[test]
    fn not_green_when_warning_present() {
        let mut r = DoctorReport::default();
        r.push("a", Status::Ok);
        r.push("b", Status::Warning("bad".into()));
        assert!(!r.green());
    }

    #[test]
    fn malformed_settings_json_is_warning_not_panic() {
        // A truncated/corrupt settings.json must not panic the doctor.
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"{not valid json").unwrap();
        let report = inspect_and_normalize(f.path()).unwrap();
        assert!(report
            .findings
            .iter()
            .any(|x| matches!(x.status, Status::Warning(_))));
    }

    #[test]
    fn settings_json_is_array_is_treated_as_warning() {
        // settings.json being a JSON array (not object) is malformed-but-parseable;
        // the doctor should warn rather than crash trying to look up hooks.
        let f = write_settings(json!(["unexpected", "shape"]));
        let report = inspect_and_normalize(f.path()).unwrap();
        assert!(report
            .findings
            .iter()
            .any(|x| matches!(x.status, Status::Warning(_))));
    }

    #[test]
    fn rtk_moved_to_last_among_three_entries() {
        // Three Bash entries with rtk first: normalization must place rtk last
        // and leave the relative order of the other two intact.
        let f = write_settings(json!({
            "hooks": {
                "PreToolUse": [rtk_entry(), other_bash_entry(), json!({
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "/opt/x/foo"}]
                })]
            }
        }));
        let report = inspect_and_normalize(f.path()).unwrap();
        assert!(report.mutated);
        let rewritten: Value =
            serde_json::from_str(&fs::read_to_string(f.path()).unwrap()).unwrap();
        let pre = rewritten["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 3);
        assert!(is_rtk_entry(&pre[2]));
        assert!(!is_rtk_entry(&pre[0]));
        assert!(!is_rtk_entry(&pre[1]));
    }

    /// Phase 2.6 regression: confirms the v2-era stdin contract is gone.
    ///
    /// v2 shipped five hook scripts under `assets/hooks/` that read
    /// `$CLAUDE_TOOL_INPUT` from the environment to gate behaviour. Those
    /// scripts (and `src/hooks.rs`) were deleted in Phase 1. This test
    /// audits the repo to make sure no resurrected code path is silently
    /// re-introducing the broken env-var gate.
    ///
    /// Why this lives here: `whetstone doctor` is the Phase 1 watchdog for
    /// the surviving hook contract. Pinning the inverse — that no piece of
    /// whetstone *itself* talks about the broken token — fits in the same
    /// module. Docs (the plan, the phase brief) reference the token
    /// historically and are explicitly excluded.
    #[test]
    fn no_source_references_claude_tool_input_env_var() {
        use std::collections::HashSet;

        let cargo_manifest_dir = env!("CARGO_MANIFEST_DIR");
        let scan_roots = ["src", "assets"];
        let needle = "CLAUDE_TOOL_INPUT";

        let mut offenders = HashSet::new();
        for root in scan_roots {
            let dir = Path::new(cargo_manifest_dir).join(root);
            scan_for(&dir, needle, &mut offenders);
        }

        // This file mentions the token in the test comment above; that's
        // documentation about the regression itself, not gating logic.
        // Strip it from the offender set.
        offenders.retain(|p| !p.ends_with("src/doctor.rs"));

        assert!(
            offenders.is_empty(),
            "Phase 2.6 regression: source/asset tree must not reference \
             `$CLAUDE_TOOL_INPUT`. Offending files: {offenders:#?}",
        );
    }

    fn scan_for(dir: &Path, needle: &str, out: &mut std::collections::HashSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_for(&path, needle, out);
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            if content.contains(needle) {
                out.insert(path.display().to_string());
            }
        }
    }
}
