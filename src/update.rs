use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{headroom, migrate, rtk, ui, version};

const REMOTE_VERSION_URL: &str = "https://raw.githubusercontent.com/z19r/whetstone/main/VERSION";
const RELEASE_URL_BASE: &str = "https://github.com/z19r/whetstone/releases/download";
const CACHE_TTL_SECS: u64 = 12 * 60 * 60;

#[derive(Debug, Serialize, Deserialize)]
struct VersionCache {
    whetstone_latest: String,
    #[serde(default)]
    rtk_latest: Option<String>,
    #[serde(default)]
    rtk_current: Option<String>,
    #[serde(default)]
    headroom_latest: Option<String>,
    #[serde(default)]
    headroom_current: Option<String>,
    timestamp: u64,
}

pub struct OutdatedComponent {
    pub name: &'static str,
    pub current: String,
    pub latest: String,
}

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

fn read_cache() -> Option<VersionCache> {
    let path = cache_path().ok()?;
    let content = fs::read_to_string(&path).ok()?;

    if let Ok(cache) = serde_json::from_str::<VersionCache>(&content) {
        return Some(cache);
    }

    let mut lines = content.lines();
    let ver = lines.next()?.trim().to_string();
    let ts: u64 = lines.next()?.trim().parse().ok()?;
    Some(VersionCache {
        whetstone_latest: ver,
        rtk_latest: None,
        rtk_current: None,
        headroom_latest: None,
        headroom_current: None,
        timestamp: ts,
    })
}

fn write_cache(cache: &VersionCache) {
    if let Ok(path) = cache_path() {
        if let Ok(json) = serde_json::to_string(cache) {
            let _ = fs::write(path, json);
        }
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

pub fn check_cached_upgrade() -> Vec<OutdatedComponent> {
    let mut outdated = Vec::new();

    let Some(cache) = read_cache() else {
        return outdated;
    };
    if now_epoch().saturating_sub(cache.timestamp) > CACHE_TTL_SECS {
        return outdated;
    }

    let current_whetstone = version::current().to_string();
    if version::is_older(&current_whetstone, &cache.whetstone_latest) {
        outdated.push(OutdatedComponent {
            name: "whetstone",
            current: current_whetstone,
            latest: cache.whetstone_latest.clone(),
        });
    }

    if let (Some(current), Some(latest)) = (&cache.rtk_current, &cache.rtk_latest) {
        if version::is_older(current, latest) {
            outdated.push(OutdatedComponent {
                name: "rtk",
                current: current.clone(),
                latest: latest.clone(),
            });
        }
    }

    if let (Some(current), Some(latest)) = (&cache.headroom_current, &cache.headroom_latest) {
        if version::is_older(current, latest) {
            outdated.push(OutdatedComponent {
                name: "headroom",
                current: current.clone(),
                latest: latest.clone(),
            });
        }
    }

    outdated
}

fn detect_target() -> Option<&'static str> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    match (os, arch) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        _ => None,
    }
}

fn self_update(latest: &str) -> Result<ui::ComponentStatus> {
    let current = version::current().to_string();

    if !version::is_older(&current, latest) {
        return Ok(ui::ComponentStatus::UpToDate(current));
    }

    let target = detect_target().context("unsupported platform for self-update")?;
    let url = format!("{RELEASE_URL_BASE}/v{latest}/whetstone-{target}.tar.gz");

    let mut sp = ui::spinner(&format!("downloading whetstone {latest}"));

    let resp = ureq::get(&url)
        .call()
        .with_context(|| format!("downloading {url}"))?;

    let mut compressed = Vec::new();
    resp.into_reader()
        .read_to_end(&mut compressed)
        .context("reading release tarball")?;

    sp.finish_and_clear();

    let decoder = flate2::read::GzDecoder::new(compressed.as_slice());
    let mut archive = tar::Archive::new(decoder);

    let current_exe = std::env::current_exe().context("locating current binary")?;
    let parent = current_exe
        .parent()
        .context("no parent dir for current binary")?;
    let staging = parent.join(".whetstone-update");

    for entry in archive.entries().context("reading tar entries")? {
        let mut entry = entry.context("reading tar entry")?;
        let path = entry.path().context("reading entry path")?;
        if path.file_name().and_then(|n| n.to_str()) == Some("whetstone") {
            entry
                .unpack(&staging)
                .context("extracting whetstone binary")?;
            break;
        }
    }

    if !staging.exists() {
        bail!("whetstone binary not found in release tarball");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&staging, fs::Permissions::from_mode(0o755)) {
            let _ = fs::remove_file(&staging);
            return Err(e).context("setting permissions on new binary");
        }
    }

    let mut backup_name = current_exe
        .file_name()
        .context("no file name on current_exe")?
        .to_os_string();
    backup_name.push(".old");
    let backup = current_exe.with_file_name(backup_name);

    if let Err(e) = fs::rename(&current_exe, &backup) {
        let _ = fs::remove_file(&staging);
        return Err(e).context("backing up current binary");
    }

    if let Err(e) = fs::rename(&staging, &current_exe) {
        let _ = fs::rename(&backup, &current_exe);
        let _ = fs::remove_file(&staging);
        return Err(e).context("replacing binary with new version");
    }

    let _ = fs::remove_file(&backup);

    Ok(ui::ComponentStatus::Updated(current, latest.to_string()))
}

/// Decision for a single managed dependency (rtk / headroom).
///
/// Pure function so we can pin the `full` semantics in a unit test without
/// touching the network or shelling out.
#[derive(Debug, PartialEq, Eq)]
enum DependencyDecision {
    UpToDate(String),
    Refresh,
    NotInstalled,
}

fn dependency_decision(
    installed: Option<&str>,
    remote_latest: Option<&str>,
    full: bool,
) -> DependencyDecision {
    match (installed, remote_latest) {
        (None, _) => DependencyDecision::NotInstalled,
        (Some(cur), Some(latest)) if !version::is_older(cur, latest) && !full => {
            DependencyDecision::UpToDate(cur.to_string())
        }
        (Some(_), _) => DependencyDecision::Refresh,
    }
}

/// Whetstone self-update + dependency refresh.
///
/// `full` plumbs the user's `--full` flag through. Today (Phase 2.4) it:
///   - logs an explicit "full mode" banner, and
///   - forces a refresh of `rtk` / `headroom` even when their installed
///     version already satisfies the remote latest. Without `full`, an
///     up-to-date component is left alone.
///
/// Phase 4 will extend this to also re-copy per-project assets
/// (`.claude/skills`, rules, manifest) and re-run `whetstone doctor`. The
/// plumbing lands now so callers don't drift; the per-project behaviour
/// stays in its phase.
pub fn run(full: bool) -> Result<()> {
    ui::section("whetstone update");

    // Phase 3.8: detect v2 markers and offer migration before pulling new
    // versions of v3 tools — migration is a precondition for v3 update.
    if migrate::detect_and_offer(false)? {
        return Ok(());
    }

    if full {
        ui::info(
            "full mode — forcing refresh of dependencies \
             (Phase 4 will also refresh project assets)",
        );
    }

    let mut sp = ui::spinner("checking for updates");
    let whetstone_latest = fetch_remote_version()?;
    let rtk_remote = rtk::latest_remote_version();
    let headroom_remote = headroom::latest_remote_version();
    sp.finish_and_clear();

    let current = version::current().to_string();
    let rtk_current = rtk::installed_version();
    let headroom_current = headroom::installed_version();
    let mut updated_count = 0u32;

    let whetstone_status = match self_update(&whetstone_latest) {
        Ok(status) => status,
        Err(e) => ui::ComponentStatus::Failed(format!("{e:#}")),
    };
    if matches!(&whetstone_status, ui::ComponentStatus::Updated(_, _)) {
        updated_count += 1;
    }
    ui::component_line("whetstone", &whetstone_status);

    let rtk_status = match dependency_decision(rtk_current.as_deref(), rtk_remote.as_deref(), full)
    {
        DependencyDecision::UpToDate(v) => ui::ComponentStatus::UpToDate(v),
        DependencyDecision::Refresh => match rtk::update() {
            Ok(status) => status,
            Err(e) => ui::ComponentStatus::Failed(format!("{e:#}")),
        },
        DependencyDecision::NotInstalled => ui::ComponentStatus::NotInstalled,
    };
    if matches!(&rtk_status, ui::ComponentStatus::Updated(_, _)) {
        updated_count += 1;
    }
    ui::component_line("rtk", &rtk_status);

    let headroom_status = match dependency_decision(
        headroom_current.as_deref(),
        headroom_remote.as_deref(),
        full,
    ) {
        DependencyDecision::UpToDate(v) => ui::ComponentStatus::UpToDate(v),
        DependencyDecision::Refresh => match headroom::update() {
            Ok(status) => status,
            Err(e) => ui::ComponentStatus::Failed(format!("{e:#}")),
        },
        DependencyDecision::NotInstalled => ui::ComponentStatus::NotInstalled,
    };
    if matches!(&headroom_status, ui::ComponentStatus::Updated(_, _)) {
        updated_count += 1;
    }
    ui::component_line("headroom", &headroom_status);

    let memory_status = ui::ComponentStatus::UpToDate("embedded".into());
    ui::component_line("memory (ICM)", &memory_status);

    write_cache(&VersionCache {
        whetstone_latest: whetstone_latest.clone(),
        rtk_current: rtk::installed_version(),
        rtk_latest: rtk_remote,
        headroom_current: headroom::installed_version(),
        headroom_latest: headroom_remote,
        timestamp: now_epoch(),
    });

    if updated_count > 0 {
        ui::summary_ok(&format!("Updated {updated_count} component(s)"));
        if matches!(&whetstone_status, ui::ComponentStatus::Updated(_, _)) {
            ui::info(&format!(
                "whetstone updated from {current} → {whetstone_latest} — \
                 restart your shell to use it"
            ));
        }
    } else {
        let has_failures = matches!(&whetstone_status, ui::ComponentStatus::Failed(_))
            || matches!(&rtk_status, ui::ComponentStatus::Failed(_))
            || matches!(&headroom_status, ui::ComponentStatus::Failed(_));

        if has_failures {
            ui::summary_info("Some components failed to update — see errors above");
        } else {
            ui::summary_ok("Everything is up to date");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_flag_forces_refresh_even_when_up_to_date() {
        // Phase 2.4 regression: previously `_full` was ignored. The user's
        // explicit `--full` must override "installed is already latest".
        assert_eq!(
            dependency_decision(Some("0.42.3"), Some("0.42.3"), true),
            DependencyDecision::Refresh,
        );
    }

    #[test]
    fn no_full_flag_skips_when_up_to_date() {
        assert_eq!(
            dependency_decision(Some("0.42.3"), Some("0.42.3"), false),
            DependencyDecision::UpToDate("0.42.3".into()),
        );
    }

    #[test]
    fn refresh_when_installed_is_older() {
        assert_eq!(
            dependency_decision(Some("0.41.0"), Some("0.42.3"), false),
            DependencyDecision::Refresh,
        );
    }

    #[test]
    fn not_installed_short_circuits_regardless_of_full_flag() {
        assert_eq!(
            dependency_decision(None, Some("0.42.3"), true),
            DependencyDecision::NotInstalled,
        );
        assert_eq!(
            dependency_decision(None, None, false),
            DependencyDecision::NotInstalled,
        );
    }
}
