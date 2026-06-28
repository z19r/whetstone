use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{ToolVersions, WhetstoneManifest};
use crate::memory::MemoryProvider;
use crate::{config, doctor, headroom, integrations, migrate, preflight, rtk, shell, ui, update};

pub(crate) const DEFAULT_PROXY: &str = "http://127.0.0.1:8787";

pub fn resolve_assets_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("WHETSTONE_ASSETS") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Ok(p);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            for candidate in ["../assets", "../../assets"] {
                let relative = bin_dir.join(candidate);
                if relative.is_dir() {
                    return Ok(relative.canonicalize()?);
                }
            }
        }
    }

    let home = dirs::home_dir().context("could not determine home directory")?;
    let fallback = home.join(".whetstone").join("assets");
    if fallback.is_dir() {
        return Ok(fallback);
    }

    bail!(
        "could not locate whetstone assets — set WHETSTONE_ASSETS or install to ~/.whetstone/assets/"
    );
}

pub fn run(full: bool, headroom_extras: &str) -> Result<()> {
    maybe_self_update(full, headroom_extras);

    let migrated = migrate::detect_and_offer(false)?;

    if ui::is_interactive() {
        return crate::wizard::run(full, headroom_extras, migrated);
    }
    run_sequential(full, headroom_extras, migrated)
}

fn maybe_self_update(full: bool, headroom_extras: &str) {
    if std::env::var("WHETSTONE_SETUP_UPDATED").is_ok() {
        return;
    }

    let remote = match update::fetch_remote_version() {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Ok(ui::ComponentStatus::Updated(from, to)) = update::self_update(&remote) {
        ui::ok(&format!(
            "updated whetstone {from} → {to}, restarting setup…"
        ));
        re_exec_setup(full, headroom_extras);
    }
}

fn re_exec_setup(full: bool, headroom_extras: &str) -> ! {
    let exe = std::env::current_exe().expect("current exe path");
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("setup");
    if full {
        cmd.arg("--full");
    }
    cmd.args(["--headroom-extras", headroom_extras]);
    cmd.env("WHETSTONE_SETUP_UPDATED", "1");

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        ui::fail(&format!("failed to restart setup: {err}"));
    }

    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(e) => ui::fail(&format!("failed to restart setup: {e}")),
        }
    }
}

fn run_sequential(full: bool, headroom_extras: &str, migrated: bool) -> Result<()> {
    ui::info("whetstone setup");

    let assets = resolve_assets_dir()?;
    ui::ok(&format!("assets at {}", assets.display()));

    ui::info("checking dependencies");
    preflight::check_all()?;

    ui::info("step 1/7 — headroom");
    headroom::install(headroom_extras, full)?;

    ui::info("step 2/7 — rtk");
    rtk::install(full)?;

    ui::info("step 3/7 — shell profile");
    shell::set_anthropic_base_url(DEFAULT_PROXY)?;
    shell::ensure_path_contains_local_bin()?;

    ui::info("step 4/7 — install whetstone binary");
    self_install()?;

    let provider = if migrated {
        ui::info("step 5/7 — memory provider (configured during migration)");
        detect_installed_provider()?
    } else {
        prompt_memory_provider(full)?
    };

    if provider != MemoryProvider::Skip {
        complete_setup(provider, &assets, full)?;
    } else {
        ui::info("skipped memory provider, skills, integrations, manifest");
    }

    ui::ok("whetstone setup complete");
    Ok(())
}

/// Shared finishing sequence used by both the headless `setup::run` path and
/// the interactive `wizard::run` path. Assumes binaries (headroom, rtk,
/// whetstone) are already installed.
pub(crate) fn complete_setup(provider: MemoryProvider, assets: &Path, full: bool) -> Result<()> {
    install_general_assets(assets, full)?;
    install_provider_binary(provider)?;
    integrations::run_all(provider)?;
    let _report = doctor::run()?;
    write_manifest(provider)?;
    generate_stack_setup(provider)?;
    Ok(())
}

pub(crate) fn prompt_memory_provider(full: bool) -> Result<MemoryProvider> {
    let project_dir = std::env::current_dir()?;
    let has_existing = project_dir.join(".claude/whetstone.json").exists()
        || project_dir.join(".claude/commands").is_dir()
        || project_dir.join(".claude/skills").is_dir()
        || project_dir.join(".claude/MEMSTACK.md").exists();

    if full {
        if has_existing {
            ui::info("full update: refreshing existing install");
            return detect_installed_provider();
        }
        ui::info("full update: no existing install found — skipping");
        return Ok(MemoryProvider::Skip);
    }

    let choices = MemoryProvider::CHOICES;
    let idx = ui::select("Choose a memory provider:", &choices, 0);
    Ok(choices[idx])
}

pub(crate) fn detect_installed_provider() -> Result<MemoryProvider> {
    // Prefer the v3 manifest if present.
    let project_dir = std::env::current_dir()?;
    let manifest_path = WhetstoneManifest::path_for(&project_dir);
    if let Some(manifest) = WhetstoneManifest::load(&manifest_path)? {
        return Ok(manifest.provider.into());
    }

    // Fall back to a shallow scan of settings.json for an ICM hint.
    let settings_path = dirs::home_dir()
        .context("home directory")?
        .join(".claude/settings.json");

    if !settings_path.exists() {
        return Ok(MemoryProvider::Icm);
    }

    // No manifest yet: default to ICM. Phase 3 migration is where AutoMem
    // installs get detected and translated.
    Ok(MemoryProvider::Icm)
}

pub(crate) fn install_general_assets(assets: &Path, full: bool) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let claude_dir = project_dir.join(".claude");

    copy_subdirs(assets, &claude_dir, full)?;

    Ok(())
}

/// Force-refresh the project's slash commands from bundled assets.
///
/// Used by `whetstone update` (Phase 4.1) when the project's recorded
/// integration-version is behind the binary's bundled
/// [`crate::config::INTEGRATION_VERSION`].
pub(crate) fn refresh_managed_subdirs(assets: &Path) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let claude_dir = project_dir.join(".claude");
    copy_subdirs(assets, &claude_dir, true)
}

/// Force-refresh all bundled project assets.
///
/// Triggered by `whetstone update --full`. v3 only ships slash commands,
/// so this is the same as [`refresh_managed_subdirs`] today.
pub(crate) fn refresh_all_assets(assets: &Path) -> Result<()> {
    refresh_managed_subdirs(assets)
}

/// Expose `icm --version` parsing so callers outside `setup.rs`
/// (e.g. `whetstone update`'s per-project refresh) can update
/// [`crate::config::ToolVersions`] without duplicating the spawn.
pub(crate) fn current_icm_version() -> Option<String> {
    installed_icm_version()
}

/// Install the memory provider's binary only. Integration (init) happens in
/// `integrations::run_all` after this returns.
pub(crate) fn install_provider_binary(provider: MemoryProvider) -> Result<()> {
    match provider {
        MemoryProvider::Icm => ensure_icm_installed(),
        MemoryProvider::Skip => Ok(()),
    }
}

fn ensure_icm_installed() -> Result<()> {
    if which::which("icm").is_ok() {
        let output = std::process::Command::new("icm").arg("--version").output();
        if let Ok(o) = output {
            if o.status.success() {
                let ver = String::from_utf8_lossy(&o.stdout).trim().to_string();
                ui::ok(&format!("icm already installed ({ver})"));
                return Ok(());
            }
        }
    }

    ui::info("installing ICM...");
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg("curl -fsSL https://raw.githubusercontent.com/rtk-ai/icm/main/install.sh | sh")
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

fn write_manifest(provider: MemoryProvider) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let manifest_path = WhetstoneManifest::path_for(&project_dir);
    let tools = ToolVersions {
        rtk: rtk::installed_version(),
        icm: installed_icm_version(),
        headroom: headroom::installed_version(),
    };
    let mut manifest = config::WhetstoneManifest::new(provider, tools);

    if let Ok(Some(existing)) = WhetstoneManifest::load(&manifest_path) {
        if let Some(mid) = existing.migration_id() {
            manifest.set_migration_id(mid);
        }
    }

    manifest.save(&manifest_path)?;
    ui::ok(&format!("wrote manifest to {}", manifest_path.display()));
    Ok(())
}

fn installed_icm_version() -> Option<String> {
    let output = std::process::Command::new("icm")
        .arg("--version")
        .output()
        .ok()?;
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    crate::version::extract_semver(&raw)
}

fn copy_subdirs(assets: &Path, claude_dir: &Path, force: bool) -> Result<()> {
    let src = assets.join("commands");
    if !src.is_dir() {
        return Ok(());
    }
    let dest = claude_dir.join("commands");
    if force || !dest.is_dir() {
        copy_dir_recursive(&src, &dest)?;
    }
    Ok(())
}

pub(crate) fn generate_stack_setup(provider: MemoryProvider) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let dest = project_dir.join("STACK-SETUP.md");
    let content = stack_setup_content(provider);
    fs::write(&dest, content)?;
    ui::ok("generated STACK-SETUP.md");
    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

fn stack_setup_content(provider: MemoryProvider) -> String {
    let provider_row = match provider {
        MemoryProvider::Icm => {
            "| ICM | Embedded SQLite memory, zero dependencies | persistent context |"
        }
        MemoryProvider::Skip => "| — | No memory provider installed | — |",
    };

    format!(
        r#"# Whetstone (Claude Code stack)

This project was set up with Whetstone: Headroom, RTK, and {provider} for
token-efficient Claude Code sessions.

## Quick Start

```bash
whetstone              # Start Claude with Headroom proxy
whetstone claude       # Same as above
whetstone doctor       # Inspect ~/.claude/settings.json
```

## Tools

| Tool | Purpose | Savings |
|------|---------|---------|
| Headroom | HTTP proxy compresses context before API | 50-90% |
| RTK | Hook rewrites CLI output before entering context | 60-90% |
{provider_row}

## Configuration

| File | Purpose |
|------|---------|
| `~/.claude/settings.json` | Hook registrations (written by rtk init / icm init) |
| `.claude/whetstone.json` | Project manifest (whetstone, integration, tool versions) |

## Uninstall

Per-project: `whetstone uninstall`
"#,
        provider = provider.name(),
        provider_row = provider_row,
    )
}

pub(crate) fn self_install() -> Result<()> {
    let current_exe =
        std::env::current_exe().context("could not determine current executable path")?;

    let home = dirs::home_dir().context("could not determine home directory")?;
    let bin_dir = home.join(".local").join("bin");
    fs::create_dir_all(&bin_dir)?;

    let dest = bin_dir.join("whetstone");

    if dest.exists() && same_file(&current_exe, &dest) {
        ui::ok("whetstone binary already in place");
        return Ok(());
    }

    if dest.exists() || dest.symlink_metadata().is_ok() {
        fs::remove_file(&dest).with_context(|| format!("removing old {}", dest.display()))?;
    }

    install_link_or_copy(&current_exe, &dest)?;

    Ok(())
}

#[cfg(unix)]
fn install_link_or_copy(src: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dest)
        .with_context(|| format!("symlinking {} → {}", dest.display(), src.display()))?;
    ui::ok(&format!("symlinked to {}", dest.display()));
    Ok(())
}

#[cfg(not(unix))]
fn install_link_or_copy(src: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    fs::copy(src, dest).with_context(|| format!("copying binary to {}", dest.display()))?;
    ui::ok(&format!("installed to {}", dest.display()));
    Ok(())
}

fn same_file(a: &PathBuf, b: &PathBuf) -> bool {
    let Ok(a_canon) = fs::canonicalize(a) else {
        return false;
    };
    let Ok(b_canon) = fs::canonicalize(b) else {
        return false;
    };
    a_canon == b_canon
}
