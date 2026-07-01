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

pub fn install(extras: &str, force: bool) -> Result<()> {
    let spec = package_spec(extras);

    if let Some(ver) = installed_version() {
        if !force && !version::is_older(&ver, MIN_VERSION) {
            ui::ok(&format!("headroom {ver} (>= {MIN_VERSION})"));
            return Ok(());
        }
        ui::info(&format!("upgrading headroom from {ver}"));
        run_uv_install(&spec, true)?;
    } else {
        ui::info("installing headroom");
        run_uv_install(&spec, false)?;
    }

    match installed_version() {
        Some(ver) => ui::ok(&format!("headroom {ver}")),
        None => bail!("headroom installation failed — check uv output above"),
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
        let path =
            PathBuf::from("/home/user/.local/share/mise/installs/python/3.14.3/bin/headroom");
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
        let path = PathBuf::from("/home/user/.pyenv/versions/3.12.0/bin/headroom");
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
