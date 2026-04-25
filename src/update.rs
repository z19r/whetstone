use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ui;
use crate::version;

const REMOTE_VERSION_URL: &str =
    "https://raw.githubusercontent.com/zackkitzmiller/whetstone/main/VERSION";
const CACHE_TTL_SECS: u64 = 12 * 60 * 60;

fn cache_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    let dir = home.join(".cache").join("whetstone");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("update-check"))
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_cache() -> Option<(String, u64)> {
    let path = cache_path().ok()?;
    let content = fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    let ver = lines.next()?.trim().to_string();
    let ts: u64 = lines.next()?.trim().parse().ok()?;
    Some((ver, ts))
}

fn write_cache(version: &str) {
    if let Ok(path) = cache_path() {
        let content = format!("{version}\n{}", now_epoch());
        let _ = fs::write(path, content);
    }
}

fn fetch_remote_version() -> Result<String> {
    let body = ureq::get(REMOTE_VERSION_URL)
        .call()
        .context("fetching remote VERSION")?
        .into_string()
        .context("reading remote VERSION body")?;

    version::extract_semver(body.trim()).context("no valid semver in remote VERSION")
}

pub fn run(full: bool) -> Result<()> {
    let current = version::current().to_string();
    ui::info(&format!("current version: {current}"));

    let remote = if let Some((cached_ver, ts)) = read_cache() {
        if now_epoch() - ts < CACHE_TTL_SECS {
            ui::info("using cached version check");
            cached_ver
        } else {
            let ver = fetch_remote_version()?;
            write_cache(&ver);
            ver
        }
    } else {
        let ver = fetch_remote_version()?;
        write_cache(&ver);
        ver
    };

    ui::info(&format!("latest version: {remote}"));

    if version::is_older(&current, &remote) {
        ui::info(&format!("update available: {current} -> {remote}"));
        ui::info("run: curl -fsSL <install-url> | sh");

        if full {
            ui::info("--full passed: re-running setup after update");
            ui::warn("self-update via binary download not yet implemented");
        }
    } else {
        ui::ok("already up to date");
    }

    Ok(())
}
