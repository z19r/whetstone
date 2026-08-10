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
        .set("User-Agent", "whetstone")
        .call()
        .ok()?;
    let body = resp.into_string().ok()?;
    let release: GithubRelease = serde_json::from_str(&body).ok()?;
    version::extract_semver(&release.tag_name)
}

pub fn installed_version() -> Option<String> {
    let output = Command::new("icm").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    version::extract_semver(&raw)
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
