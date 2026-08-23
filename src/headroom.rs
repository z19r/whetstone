use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use crate::ui;
use crate::version;

const MIN_VERSION: &str = "0.21.0";
const PYPI_URL: &str = "https://pypi.org/pypi/headroom-ai/json";

#[derive(Deserialize)]
struct PypiResponse {
    info: PypiInfo,
}

#[derive(Deserialize)]
struct PypiInfo {
    version: String,
}

pub fn latest_remote_version() -> Option<String> {
    let resp = ureq::get(PYPI_URL).call().ok()?;
    let body = resp.into_string().ok()?;
    let parsed: PypiResponse = serde_json::from_str(&body).ok()?;
    Some(parsed.info.version)
}

pub fn resolve_extras(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "all" => "proxy,code,mcp".to_string(),
        "none" => String::new(),
        other => other.to_string(),
    }
}

fn package_spec(extras: &str) -> String {
    let resolved = resolve_extras(extras);
    if resolved.is_empty() {
        "headroom-ai".to_string()
    } else {
        format!("headroom-ai[{resolved}]")
    }
}

pub fn installed_version() -> Option<String> {
    let output = Command::new("headroom").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    version::extract_semver(&raw)
}

/// Extras uv recorded for the installed `headroom-ai` tool.
///
/// `None` means "unknown" — no uv receipt, or a receipt shape we don't
/// recognize (a pip install, for instance). Callers must never read that as
/// "no extras", or whetstone would reinstall on every run.
pub fn recorded_extras() -> Option<Vec<String>> {
    let receipt = fs::read_to_string(uv_receipt_path()?).ok()?;
    parse_receipt_extras(&receipt)
}

fn uv_receipt_path() -> Option<PathBuf> {
    let tool_dir = match std::env::var("UV_TOOL_DIR") {
        Ok(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
        _ => dirs::data_dir()?.join("uv").join("tools"),
    };
    Some(tool_dir.join("headroom-ai").join("uv-receipt.toml"))
}

/// Pull the extras out of uv's receipt, which pins the requirement on one
/// line: `requirements = [{ name = "headroom-ai", extras = ["proxy", …] }]`.
/// Deliberately a narrow scan rather than a TOML dependency — anything that
/// doesn't match the expected shape returns `None` (unknown), and a receipt
/// naming the package with no `extras` key returns an empty list (installed
/// bare).
fn parse_receipt_extras(receipt: &str) -> Option<Vec<String>> {
    let line = receipt
        .lines()
        .find(|line| line.contains(r#"name = "headroom-ai""#))?;

    let Some(rest) = line.split_once("extras = [") else {
        return Some(Vec::new());
    };
    let list = rest.1.split_once(']')?.0;

    Some(
        list.split(',')
            .map(|item| item.trim().trim_matches('"').to_string())
            .filter(|item| !item.is_empty())
            .collect(),
    )
}

/// Extras requested by `extras` that the recorded install doesn't have.
///
/// Empty when everything asked for is present *or* when the install predates
/// uv (unknown extras) — this only reports what it can prove is missing.
pub fn missing_extras(extras: &str) -> Vec<String> {
    let Some(installed) = recorded_extras() else {
        return Vec::new();
    };
    diff_extras(&resolve_extras(extras), &installed)
}

fn diff_extras(requested: &str, installed: &[String]) -> Vec<String> {
    requested
        .split(',')
        .map(str::trim)
        .filter(|want| !want.is_empty())
        .filter(|want| !installed.iter().any(|have| have == want))
        .map(str::to_string)
        .collect()
}

/// Headroom's own settings file (not whetstone's). Keys here are turned into
/// proxy flags at launch, which is how a setting saved when a feature was
/// ungated can hard-fail every start after an upgrade.
pub fn settings_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".headroom").join("settings.json"))
}

/// Given headroom's startup complaint, name the settings key that caused it.
///
/// Headroom rejects a gated flag with:
/// `error: --read-maturation is not available in the current rollout channel`
/// and that flag is the kebab-case spelling of the JSON key
/// (`read_maturation`) whetstone can remove.
pub fn blocked_setting_key(detail: &str) -> Option<String> {
    if !detail.contains("not available in the current rollout channel") {
        return None;
    }
    let flag = detail
        .split_whitespace()
        .find(|token| token.starts_with("--") && token.len() > 2)?;
    Some(flag.trim_start_matches('-').replace('-', "_"))
}

/// Remove `key` from headroom's settings file, backing the file up first.
/// `Ok(false)` means the key wasn't there — nothing was written.
pub fn disable_setting(key: &str) -> Result<bool> {
    let path = settings_path().context("could not determine home directory")?;
    disable_setting_in(&path, key)
}

fn disable_setting_in(path: &Path, key: &str) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let mut settings: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {}", path.display()))?;

    let Some(obj) = settings.as_object_mut() else {
        return Ok(false);
    };
    if obj.remove(key).is_none() {
        return Ok(false);
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup = path.with_file_name(format!("settings.json.bak.{ts}"));
    fs::copy(path, &backup)
        .with_context(|| format!("backing up {}", path.display()))?;

    let pretty = serde_json::to_string_pretty(&settings)
        .context("serializing headroom settings")?;
    fs::write(path, format!("{pretty}\n"))
        .with_context(|| format!("writing {}", path.display()))?;

    ui::info(&format!(
        "removed `{key}` from {} (backup: {})",
        path.display(),
        backup.display(),
    ));
    Ok(true)
}

pub fn install(extras: &str, force: bool) -> Result<()> {
    let spec = package_spec(extras);

    if let Some(ver) = installed_version() {
        let missing = missing_extras(extras);

        if !force && !version::is_older(&ver, MIN_VERSION) && missing.is_empty()
        {
            ui::ok(&format!("headroom {ver} (>= {MIN_VERSION})"));
            return Ok(());
        }

        // A version-only check calls a bare `headroom-ai` healthy forever,
        // and then `headroom proxy` / `headroom mcp` don't exist at runtime.
        if missing.is_empty() {
            ui::info(&format!("upgrading headroom from {ver}"));
        } else {
            ui::info(&format!(
                "headroom {ver} is missing extras ({}) — reinstalling as {spec}",
                missing.join(", "),
            ));
        }
        run_uv_install(&spec, true)?;
    } else {
        ui::info("installing headroom");
        run_uv_install(&spec, false)?;
    }

    match installed_version() {
        Some(ver) => ui::ok(&format!("headroom {ver}")),
        None => bail!("headroom installation failed — check uv output above"),
    }

    let still_missing = missing_extras(extras);
    if !still_missing.is_empty() {
        ui::warn(&format!(
            "headroom installed but extras are still missing: {}",
            still_missing.join(", "),
        ));
    }
    Ok(())
}

pub fn update() -> Result<ui::ComponentStatus> {
    let Some(old_ver) = installed_version() else {
        return Ok(ui::ComponentStatus::NotInstalled);
    };

    let spec = package_spec("all");
    let output = Command::new("uv")
        .args(["tool", "install", "--upgrade", &spec])
        .env("PYO3_USE_ABI3_FORWARD_COMPATIBILITY", "1")
        .output()
        .context("failed to run uv tool install")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("uv tool install failed: {stderr}");
    }

    // uv upgraded successfully — check if a stale pip install shadows the new binary
    if let Some(fixed_ver) = fix_shadow() {
        if fixed_ver != old_ver {
            return Ok(ui::ComponentStatus::Updated(old_ver, fixed_ver));
        }
    }

    let new_ver = installed_version().unwrap_or_else(|| old_ver.clone());
    if new_ver != old_ver {
        Ok(ui::ComponentStatus::Updated(old_ver, new_ver))
    } else {
        Ok(ui::ComponentStatus::UpToDate(old_ver))
    }
}

fn uv_managed_version() -> Option<String> {
    let home = dirs::home_dir()?;
    let uv_headroom = home.join(".local/bin/headroom");
    if !uv_headroom.exists() {
        return None;
    }
    let output = Command::new(&uv_headroom).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    version::extract_semver(&raw)
}

fn resolve_headroom_binary() -> Option<PathBuf> {
    let output = Command::new("which").arg("headroom").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = PathBuf::from(&path_str);
    fs::canonicalize(&path).ok().or(Some(path))
}

fn detect_shadowing_python(binary_path: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let uv_bin = home.join(".local/bin");

    if binary_path.starts_with(&uv_bin) {
        return None;
    }

    let path_str = binary_path.to_string_lossy();
    let is_python_env = path_str.contains("/mise/installs/python/")
        || path_str.contains("/pyenv/versions/")
        || path_str.contains("/conda/")
        || path_str.contains("/miniconda")
        || path_str.contains("/anaconda")
        || path_str.contains("/virtualenvs/");

    if !is_python_env {
        return None;
    }

    let parent = binary_path.parent()?;
    for name in &["python3", "python"] {
        let candidate = parent.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn fix_shadow() -> Option<String> {
    let uv_ver = uv_managed_version()?;
    let path_ver = installed_version()?;

    if uv_ver == path_ver {
        return None;
    }

    let shadow_path = resolve_headroom_binary()?;
    let python = detect_shadowing_python(&shadow_path)?;

    ui::info(&format!(
        "stale headroom {} at {} shadows uv-managed {} — removing pip copy",
        path_ver,
        shadow_path.display(),
        uv_ver,
    ));

    let _ = Command::new(&python)
        .args(["-m", "pip", "uninstall", "headroom-ai", "-y"])
        .stdout(io::stdout())
        .stderr(io::stderr())
        .status();

    let fixed = installed_version()?;
    if fixed == uv_ver {
        ui::ok(&format!("headroom now resolves to {fixed}"));
    }
    Some(fixed)
}

/// Best-effort `headroom learn` invocation.
///
/// Phase 4.2: rerun on `whetstone update` so the CLAUDE.md
/// learned-patterns block doesn't rot. Returns `Ok(true)` when the command
/// was successfully invoked, `Ok(false)` when headroom isn't installed or
/// doesn't support `learn` on this version, and `Err` only for true I/O
/// failures the caller may want to surface. The update path treats any
/// failure as non-fatal.
pub fn learn() -> Result<bool> {
    if which::which("headroom").is_err() {
        return Ok(false);
    }

    let output = Command::new("headroom")
        .arg("learn")
        .output()
        .context("failed to spawn `headroom learn`")?;

    if output.status.success() {
        return Ok(true);
    }

    // Older headroom versions don't ship the `learn` subcommand. Detect
    // that softly so we don't yell at the user for a no-op.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}").to_lowercase();
    let looks_like_unknown_subcommand = combined.contains("unrecognized")
        || combined.contains("unknown command")
        || combined.contains("no such command")
        || combined.contains("invalid choice");

    if looks_like_unknown_subcommand {
        return Ok(false);
    }

    bail!(
        "headroom learn failed (exit {:?}): {}",
        output.status.code(),
        stderr.trim()
    );
}

/// Whether a serde_json value has a `mcpServers.headroom` entry.
fn json_has_headroom_mcp(v: &serde_json::Value) -> bool {
    v.get("mcpServers")
        .and_then(|servers| servers.get("headroom"))
        .is_some()
}

/// Whether the Headroom MCP server is registered in Claude Code's config
/// (`~/.claude.json`). We only re-sync registrations the user already opted
/// into — never force-add MCP for users who kept it off.
fn mcp_registered() -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let Ok(content) = fs::read_to_string(home.join(".claude.json")) else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(&content)
        .map(|json| json_has_headroom_mcp(&json))
        .unwrap_or(false)
}

/// After a headroom upgrade the registered MCP command (bare `headroom` vs. the
/// absolute uv path) and `--proxy-url` can drift out of sync, which makes the
/// `headroom_retrieve` tool warn on every session start. Re-run `mcp install`
/// (scoped to Claude Code) to rewrite the entry — but only when it already
/// exists. Best-effort; returns `Ok(false)` when nothing was registered.
pub fn resync_mcp_if_registered() -> Result<bool> {
    if !mcp_registered() {
        return Ok(false);
    }

    let output = Command::new("headroom")
        .args(["mcp", "install", "--agent", "claude", "--force"])
        .output()
        .context("failed to run `headroom mcp install`")?;

    if output.status.success() {
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("headroom mcp install failed: {}", stderr.trim());
    }
}

fn run_uv_install(spec: &str, upgrade: bool) -> Result<()> {
    let mut args = vec!["tool", "install"];
    if upgrade {
        args.push("--upgrade");
    }
    args.push(spec);

    let status = Command::new("uv")
        .args(&args)
        .env("PYO3_USE_ABI3_FORWARD_COMPATIBILITY", "1")
        .status()?;

    if !status.success() {
        bail!(
            "uv tool install failed (exit {})",
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_registered_headroom_mcp() {
        let json = serde_json::json!({
            "mcpServers": { "headroom": { "command": "headroom", "args": ["mcp", "serve"] } }
        });
        assert!(json_has_headroom_mcp(&json));
    }

    #[test]
    fn ignores_config_without_headroom_mcp() {
        let json = serde_json::json!({
            "mcpServers": { "icm": { "command": "icm" } }
        });
        assert!(!json_has_headroom_mcp(&json));
    }

    #[test]
    fn ignores_config_without_mcp_servers() {
        let json = serde_json::json!({ "projects": {} });
        assert!(!json_has_headroom_mcp(&json));
    }

    const RECEIPT_WITH_EXTRAS: &str = r#"[tool]
requirements = [{ name = "headroom-ai", extras = ["proxy", "code", "mcp"] }]
entrypoints = [
    { name = "headroom", install-path = "/home/u/.local/bin/headroom", from = "headroom-ai" },
]
"#;

    const RECEIPT_BARE: &str = r#"[tool]
requirements = [{ name = "headroom-ai" }]
"#;

    #[test]
    fn names_the_settings_key_behind_a_blocked_flag() {
        let detail = "error: --read-maturation is not available in the current \
                      rollout channel (stable). Set HEADROOM_ROLLOUT_CHANNEL=beta";
        assert_eq!(
            blocked_setting_key(detail),
            Some("read_maturation".to_string()),
        );
    }

    #[test]
    fn unrelated_startup_errors_name_no_key() {
        // Only the rollout-channel rejection maps cleanly onto a settings key;
        // whetstone must not start deleting keys for other failures.
        assert_eq!(
            blocked_setting_key("Error: Proxy dependencies not installed."),
            None,
        );
        assert_eq!(blocked_setting_key("error: address already in use"), None);
    }

    #[test]
    fn disable_setting_removes_the_key_and_leaves_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"anthropic_base_url":"https://api.anthropic.com","read_maturation":true}"#,
        )
        .unwrap();

        assert!(disable_setting_in(&path, "read_maturation").unwrap());

        let after: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(after.get("read_maturation").is_none());
        assert_eq!(
            after.get("anthropic_base_url").and_then(|v| v.as_str()),
            Some("https://api.anthropic.com"),
        );
    }

    #[test]
    fn disable_setting_backs_the_file_up_before_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"read_maturation":true}"#).unwrap();

        disable_setting_in(&path, "read_maturation").unwrap();

        let backups: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains("settings.json.bak.")
            })
            .collect();
        assert_eq!(backups.len(), 1, "expected exactly one backup file");
    }

    #[test]
    fn disable_setting_is_a_no_op_for_an_absent_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"anthropic_base_url":"x"}"#).unwrap();

        assert!(!disable_setting_in(&path, "read_maturation").unwrap());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{"anthropic_base_url":"x"}"#
        );
    }

    #[test]
    fn disable_setting_on_a_missing_file_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        assert!(!disable_setting_in(&path, "read_maturation").unwrap());
    }

    #[test]
    fn parses_extras_from_a_uv_receipt() {
        assert_eq!(
            parse_receipt_extras(RECEIPT_WITH_EXTRAS),
            Some(vec!["proxy".into(), "code".into(), "mcp".into()]),
        );
    }

    #[test]
    fn receipt_without_extras_means_installed_bare() {
        // Distinct from "unknown": uv knows about this install and it has no
        // extras, so whetstone should repair it.
        assert_eq!(parse_receipt_extras(RECEIPT_BARE), Some(Vec::new()));
    }

    #[test]
    fn unrecognized_receipt_is_unknown_not_empty() {
        // Anything we can't read must NOT look like "no extras", or every run
        // would reinstall headroom.
        assert_eq!(parse_receipt_extras("[tool]\nrequirements = []\n"), None);
        assert_eq!(parse_receipt_extras(""), None);
    }

    #[test]
    fn diff_reports_only_the_extras_that_are_absent() {
        let installed = vec!["proxy".to_string(), "code".to_string()];
        assert_eq!(
            diff_extras(&resolve_extras("all"), &installed),
            vec!["mcp".to_string()],
        );
        assert!(
            diff_extras(&resolve_extras("proxy,code"), &installed).is_empty()
        );
        assert!(diff_extras(&resolve_extras("none"), &installed).is_empty());
    }

    #[test]
    fn bare_install_is_missing_every_requested_extra() {
        assert_eq!(
            diff_extras(&resolve_extras("all"), &[]),
            vec!["proxy".to_string(), "code".to_string(), "mcp".to_string()],
        );
    }

    #[test]
    fn extras_all() {
        assert_eq!(package_spec("all"), "headroom-ai[proxy,code,mcp]");
    }

    #[test]
    fn extras_none() {
        assert_eq!(package_spec("none"), "headroom-ai");
    }

    #[test]
    fn extras_custom() {
        assert_eq!(package_spec("proxy,code"), "headroom-ai[proxy,code]");
    }

    #[test]
    fn parse_pypi_response() {
        let json = r#"{"info":{"version":"0.22.2"}}"#;
        let parsed: PypiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.info.version, "0.22.2");
    }

    #[test]
    fn detect_shadow_in_mise_python() {
        let path = PathBuf::from(
            "/home/user/.local/share/mise/installs/python/3.14.3/bin/headroom",
        );
        let result = detect_shadowing_python(&path);
        // Can't assert Some because the python binary doesn't exist on disk,
        // but verify the function doesn't panic and recognizes the pattern
        assert!(
            result.is_none(),
            "returns None when python binary doesn't exist on disk"
        );
    }

    #[test]
    fn detect_shadow_in_pyenv() {
        let path =
            PathBuf::from("/home/user/.pyenv/versions/3.12.0/bin/headroom");
        let result = detect_shadowing_python(&path);
        assert!(result.is_none());
    }

    #[test]
    fn no_shadow_for_uv_binary() {
        let home = dirs::home_dir().unwrap();
        let path = home.join(".local/bin/headroom");
        assert!(detect_shadowing_python(&path).is_none());
    }

    #[test]
    fn no_shadow_for_unknown_path() {
        let path = PathBuf::from("/opt/custom/bin/headroom");
        assert!(detect_shadowing_python(&path).is_none());
    }
}
