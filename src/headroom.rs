use anyhow::{bail, Result};
use std::process::Command;

use crate::ui;
use crate::version;

const MIN_VERSION: &str = "0.14.0";

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

fn installed_version() -> Option<String> {
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

fn run_uv_install(spec: &str, upgrade: bool) -> Result<()> {
    let mut args = vec!["tool", "install"];
    if upgrade {
        args.push("--upgrade");
    }
    args.push(spec);

    let status = Command::new("uv").args(&args).status()?;

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
}
