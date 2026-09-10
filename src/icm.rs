use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::process::Command;

use crate::ui;
use crate::version;

/// ICM publishes GitHub releases tagged `icm-vX.Y.Z` (e.g. `icm-v0.10.61`).
/// `extract_semver` pulls the `X.Y.Z` out of that tag regardless of prefix.
const GITHUB_LATEST_URL: &str =
    "https://api.github.com/repos/rtk-ai/icm/releases/latest";

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

pub fn latest_remote_version() -> Option<String> {
    let resp = ureq::get(GITHUB_LATEST_URL)

pub fn latest_remote_version() -> Option<String> {
    let resp = ureq::get(GITHUB_LATEST_URL)
        .header("User-Agent", "whetstone")
        .call()
        .ok()?;
    let body = resp.into_body().read_to_string().ok()?;
    let release: GithubRelease = serde_json::from_str(&body).ok()?;
    version::extract_semver(&release.tag_name)
}
++ b/src/rtk.rs

pub fn latest_remote_version() -> Option<String> {
    let resp = ureq::get(GITHUB_LATEST_URL)
        .header("User-Agent", "whetstone")
        .call()
        .ok()?;
    let body = resp.into_body().read_to_string().ok()?;
    let release: GithubRelease = serde_json::from_str(&body).ok()?;
    let tag = release.tag_name.trim_start_matches('v');
    version::extract_semver(tag)
++ b/src/migrate.rs

const INSTALL_URL: &str =
    "https://raw.githubusercontent.com/rtk-ai/icm/main/install.sh";

/// Install ICM via its own install script. `force` reinstalls even when a
/// working binary is already on `PATH` (used by `whetstone install-tools
/// --force` and `setup --full`).
pub fn install(force: bool) -> Result<()> {
    if !force {
        if let Some(ver) = installed_version() {
            ui::ok(&format!("icm already installed ({ver})"));
            return Ok(());
        }
    }

    ui::info("installing ICM...");
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("curl -fsSL {INSTALL_URL} | sh"))
        .status()
        .context("failed to run ICM install script")?;

    if !status.success() {
        bail!("ICM installation failed");
    }

    if which::which("icm").is_err() {
        bail!("ICM binary not found after installation — check your PATH");
    }

    ui::ok("ICM installed");
    Ok(())
}

/// Upgrade ICM via its own self-updater (`icm upgrade --apply`). ICM owns its
/// install/upgrade path, so we delegate rather than re-run the install script.
pub fn update() -> Result<ui::ComponentStatus> {
    let Some(old_ver) = installed_version() else {
        return Ok(ui::ComponentStatus::NotInstalled);
    };

    let output = Command::new("icm")
        .arg("upgrade")
        .arg("--apply")
        .output()
        .context("failed to run icm upgrade --apply")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("icm upgrade failed: {stderr}");
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
    fn parse_prefixed_icm_release_tag() {
        // ICM tags releases `icm-vX.Y.Z`, not a bare `vX.Y.Z` — the extractor
        // must still recover the semver from the prefixed tag.
        let json = r#"{"tag_name":"icm-v0.10.61"}"#;
        let release: GithubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "icm-v0.10.61");
        assert_eq!(
            version::extract_semver(&release.tag_name),
            Some("0.10.61".into()),
        );
    }
}
    let url =
        format!("{}/recall?q=&limit=1000", endpoint.trim_end_matches('/'));
    let resp = match ureq::get(&url)
        .header("Authorization", &format!("Bearer {api_key}"))
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .call()
    {
        Ok(r) => r,
        }
    };

    let body = match resp.into_body().read_to_string() {
        Ok(b) => b,
        Err(e) => {
            ui::warn(&format!("AutoMem export skipped: {e}"));
++ b/src/stats.rs

fn fetch_stats() -> Result<HeadroomStats> {
    let body = ureq::get(HEADROOM_STATS_URL)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(3)))
        .build()
        .call()
        .context("headroom proxy not reachable at localhost:8787")?
        .into_body()
        .read_to_string()
        .context("failed to read headroom stats")?;

    serde_json::from_str(&body).context("failed to parse headroom stats JSON")

fn proxy_is_running() -> bool {
    ureq::get(HEADROOM_HEALTH_URL)
        .config()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .call()
        .is_ok()
}
++ b/src/update.rs
    let body = ureq::get(REMOTE_VERSION_URL)
        .call()
        .context("fetching remote VERSION")?
        .into_body()
        .read_to_string()
        .context("reading remote VERSION body")?;

    version::extract_semver(body.trim())
        .with_context(|| format!("downloading {url}"))?;

    let mut compressed = Vec::new();
    resp.into_body()
        .into_reader()
        .read_to_end(&mut compressed)
        .context("reading release tarball")?;

++ b/src/wrapper.rs

fn probe_port(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/health");
    ureq::get(&url)
        .config()
        .timeout_global(Some(PROXY_PROBE_TIMEOUT))
        .build()
        .call()
        .is_ok()
}

fn probe_proxy() -> bool {
