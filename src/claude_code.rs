use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::ui;
use crate::version;

const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org/@anthropic-ai/claude-code/latest";

#[derive(Debug, PartialEq, Eq)]
pub enum InstallMethod {
    NativeBinary,
    Npm,
    Unknown,
}

#[derive(Deserialize)]
struct NpmLatest {
    version: String,
}

pub fn installed_version() -> Option<String> {
    let output = Command::new("claude").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_claude_version(&raw)
}

fn parse_claude_version(raw: &str) -> Option<String> {
    let first_token = raw.split_whitespace().next()?;
    version::extract_semver(first_token)
}

pub fn latest_npm_version() -> Option<String> {
    let resp = ureq::get(NPM_REGISTRY_URL)
        .set("Accept", "application/json")
        .call()
        .ok()?;
    let body = resp.into_string().ok()?;
    parse_npm_response(&body)
}

fn parse_npm_response(body: &str) -> Option<String> {
    let parsed: NpmLatest = serde_json::from_str(body).ok()?;
    version::extract_semver(&parsed.version)
}

pub fn install_method() -> InstallMethod {
    detect_install_method_from_path(resolve_claude_binary())
}

fn resolve_claude_binary() -> Option<PathBuf> {
    let output = Command::new("which").arg("claude").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = PathBuf::from(&path_str);
    fs::canonicalize(&path).ok().or(Some(path))
}

fn detect_install_method_from_path(binary_path: Option<PathBuf>) -> InstallMethod {
    let Some(path) = binary_path else {
        return InstallMethod::Unknown;
    };
    let path_str = path.to_string_lossy();

    if path_str.contains(".local/share/claude/versions/") {
        return InstallMethod::NativeBinary;
    }

    if path_str.contains("node_modules")
        || path_str.contains("/node/")
        || path_str.contains("/nodejs/")
        || path_str.contains("/mise/installs/node/")
        || path_str.contains("/nvm/versions/node/")
        || path_str.contains("/fnm/node-versions/")
        || path_str.contains("/.volta/")
    {
        return InstallMethod::Npm;
    }

    InstallMethod::Unknown
}

pub fn update() -> Result<ui::ComponentStatus> {
    let Some(old_ver) = installed_version() else {
        return Ok(ui::ComponentStatus::NotInstalled);
    };

    let method = install_method();

    match method {
        InstallMethod::NativeBinary => update_from_native(&old_ver),
        InstallMethod::Npm | InstallMethod::Unknown => update_via_npm(&old_ver),
    }
}

fn update_from_native(old_ver: &str) -> Result<ui::ComponentStatus> {
    ui::info("native binary detected — installing via npm to fix stale update channel");

    run_npm_install()?;
    cleanup_native_install();

    let new_ver = installed_version().unwrap_or_else(|| old_ver.to_string());
    if new_ver != old_ver {
        Ok(ui::ComponentStatus::Updated(old_ver.to_string(), new_ver))
    } else {
        Ok(ui::ComponentStatus::UpToDate(old_ver.to_string()))
    }
}

fn update_via_npm(old_ver: &str) -> Result<ui::ComponentStatus> {
    run_npm_install()?;

    let new_ver = installed_version().unwrap_or_else(|| old_ver.to_string());
    if new_ver != old_ver {
        Ok(ui::ComponentStatus::Updated(old_ver.to_string(), new_ver))
    } else {
        Ok(ui::ComponentStatus::UpToDate(old_ver.to_string()))
    }
}

fn run_npm_install() -> Result<()> {
    let output = Command::new("npm")
        .args(["install", "-g", "@anthropic-ai/claude-code"])
        .output()
        .context("failed to run npm install")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("npm install -g @anthropic-ai/claude-code failed: {stderr}");
    }

    Ok(())
}

fn cleanup_native_install() {
    let Some(home) = dirs::home_dir() else {
        return;
    };

    let symlink = home.join(".local/bin/claude");
    let versions_dir = home.join(".local/share/claude");

    if symlink.is_symlink() || symlink.exists() {
        if let Ok(target) = fs::read_link(&symlink) {
            if target
                .to_string_lossy()
                .contains(".local/share/claude/versions/")
            {
                ui::info("removing stale native binary symlink (~/.local/bin/claude)");
                let _ = fs::remove_file(&symlink);
            }
        }
    }

    if versions_dir.is_dir() {
        ui::info("removing stale native versions directory (~/.local/share/claude/)");
        let _ = fs::remove_dir_all(&versions_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_claude_version_standard_format() {
        assert_eq!(
            parse_claude_version("2.1.153 (Claude Code)"),
            Some("2.1.153".into()),
        );
    }

    #[test]
    fn parse_claude_version_bare() {
        assert_eq!(parse_claude_version("2.1.172"), Some("2.1.172".into()));
    }

    #[test]
    fn parse_claude_version_empty() {
        assert_eq!(parse_claude_version(""), None);
    }

    #[test]
    fn parse_claude_version_garbage() {
        assert_eq!(parse_claude_version("not a version"), None);
    }

    #[test]
    fn parse_npm_response_valid() {
        let body = r#"{"name":"@anthropic-ai/claude-code","version":"2.1.172"}"#;
        assert_eq!(parse_npm_response(body), Some("2.1.172".into()));
    }

    #[test]
    fn parse_npm_response_invalid_json() {
        assert_eq!(parse_npm_response("{broken"), None);
    }

    #[test]
    fn parse_npm_response_missing_version() {
        assert_eq!(parse_npm_response(r#"{"name":"foo"}"#), None);
    }

    #[test]
    fn detect_native_binary_path() {
        let path = Some(PathBuf::from(
            "/home/user/.local/share/claude/versions/2.1.153",
        ));
        assert_eq!(
            detect_install_method_from_path(path),
            InstallMethod::NativeBinary,
        );
    }

    #[test]
    fn detect_npm_via_mise() {
        let path = Some(PathBuf::from(
            "/home/user/.local/share/mise/installs/node/24.11.1/bin/claude",
        ));
        assert_eq!(detect_install_method_from_path(path), InstallMethod::Npm);
    }

    #[test]
    fn detect_npm_via_nvm() {
        let path = Some(PathBuf::from(
            "/home/user/.nvm/versions/node/v22.0.0/bin/claude",
        ));
        assert_eq!(detect_install_method_from_path(path), InstallMethod::Npm);
    }

    #[test]
    fn detect_npm_via_node_modules() {
        let path = Some(PathBuf::from(
            "/usr/lib/node_modules/@anthropic-ai/claude-code/bin/claude",
        ));
        assert_eq!(detect_install_method_from_path(path), InstallMethod::Npm);
    }

    #[test]
    fn detect_npm_via_volta() {
        let path = Some(PathBuf::from("/home/user/.volta/bin/claude"));
        assert_eq!(detect_install_method_from_path(path), InstallMethod::Npm);
    }

    #[test]
    fn detect_unknown_when_no_binary() {
        assert_eq!(
            detect_install_method_from_path(None),
            InstallMethod::Unknown
        );
    }

    #[test]
    fn detect_unknown_for_unrecognized_path() {
        let path = Some(PathBuf::from("/opt/custom/bin/claude"));
        assert_eq!(
            detect_install_method_from_path(path),
            InstallMethod::Unknown,
        );
    }
}
