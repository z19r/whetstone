use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::process::Command;

use crate::ui;
use crate::version;

const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org/@anthropic-ai/claude-code/latest";

#[derive(Deserialize)]
struct NpmPackage {
    version: String,
}

pub fn latest_remote_version() -> Option<String> {
    let resp = ureq::get(NPM_REGISTRY_URL).call().ok()?;
    let body = resp.into_string().ok()?;
    let parsed: NpmPackage = serde_json::from_str(&body).ok()?;
    Some(parsed.version)
}

pub fn installed_version() -> Option<String> {
    let output = Command::new("claude").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    version::extract_semver(&raw)
}

pub fn update() -> Result<ui::ComponentStatus> {
    let Some(old_ver) = installed_version() else {
        return Ok(ui::ComponentStatus::NotInstalled);
    };

    let mut sp = ui::spinner(&format!("updating claude code from {old_ver}"));

    // `claude update` silently no-ops in non-interactive (piped stdio) mode.
    // Use the official install script which always works.
    let output = Command::new("sh")
        .arg("-c")
        .arg("curl -fsSL https://claude.ai/install.sh | sh")
        .output()
        .context("failed to run claude code installer")?;

    sp.finish_and_clear();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("claude code installer failed: {stderr}");
    }

    let new_ver = installed_version().unwrap_or_else(|| old_ver.clone());
    if new_ver != old_ver {
        Ok(ui::ComponentStatus::Updated(old_ver, new_ver))
    } else {
        Ok(ui::ComponentStatus::UpToDate(old_ver))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_npm_registry_response() {
        let json = r#"{"name":"@anthropic-ai/claude-code","version":"2.1.153"}"#;
        let parsed: NpmPackage = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.version, "2.1.153");
    }

    #[test]
    fn extract_version_from_cli_output() {
        assert_eq!(
            version::extract_semver("2.1.153 (Claude Code)"),
            Some("2.1.153".into())
        );
    }

    #[test]
    fn extract_version_bare() {
        assert_eq!(version::extract_semver("2.1.153"), Some("2.1.153".into()));
    }
}
