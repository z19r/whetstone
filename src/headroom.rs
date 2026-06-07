use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::process::Command;

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

    let new_ver = installed_version().unwrap_or_else(|| old_ver.clone());
    if new_ver != old_ver {
        Ok(ui::ComponentStatus::Updated(old_ver, new_ver))
    } else {
        Ok(ui::ComponentStatus::UpToDate(old_ver))
    }
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
}
