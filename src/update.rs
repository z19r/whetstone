use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::memory::MemoryProvider;
use crate::{headroom, rtk, setup, ui, version};

const REMOTE_VERSION_URL: &str = "https://raw.githubusercontent.com/z19r/whetstone/main/VERSION";
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

fn read_configured_extras() -> String {
    let config_path = std::env::current_dir()
        .ok()
        .map(|d| d.join(".claude/config.local.json"));

    if let Some(path) = config_path {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(extras) = val
                    .get("headroom")
                    .and_then(|h| h.get("required_extras"))
                    .and_then(|e| e.as_array())
                {
                    let joined: Vec<String> = extras
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.trim_matches(|c| c == '[' || c == ']').to_string())
                        .collect();
                    if !joined.is_empty() {
                        return joined.join(",");
                    }
                }
            }
        }
    }

    "all".to_string()
}

pub fn run(full: bool) -> Result<()> {
    let current = version::current().to_string();
    ui::info(&format!("current whetstone version: {current}"));

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

    ui::info(&format!("latest whetstone version: {remote}"));

    if version::is_older(&current, &remote) {
        ui::warn(&format!(
            "whetstone update available: {current} -> {remote}"
        ));
        ui::info(
            "run: curl -fsSL https://raw.githubusercontent.com/z19r/whetstone/main/install.sh | bash",
        );
    } else {
        ui::ok("whetstone up to date");
    }

    update_components(full)?;

    Ok(())
}

fn update_components(full: bool) -> Result<()> {
    let extras = read_configured_extras();

    ui::info("checking headroom...");
    headroom::install(&extras, full)?;

    ui::info("checking rtk...");
    rtk::install(full)?;

    let provider = setup::detect_installed_provider()?;
    if provider != MemoryProvider::Skip {
        ui::info(&format!("checking {}...", provider.name()));
        setup::install_provider(provider)?;
    }

    if full {
        if let Ok(assets) = setup::resolve_assets_dir() {
            ui::info("refreshing bundled assets...");
            setup::install_general_assets(&assets, true, &extras)?;
        }
    }

    ui::ok("all components checked");
    Ok(())
}
