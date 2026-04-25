use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{config, db, headroom, hooks, preflight, rtk, shell, ui};

const DEFAULT_PROXY: &str = "http://127.0.0.1:8787";

pub fn resolve_assets_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("WHETSTONE_ASSETS") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Ok(p);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            let relative = bin_dir.join("../assets");
            if relative.is_dir() {
                return Ok(relative.canonicalize()?);
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
    ui::info("whetstone setup");

    let assets = resolve_assets_dir()?;
    ui::ok(&format!("assets at {}", assets.display()));

    ui::info("checking dependencies");
    preflight::check_all()?;

    ui::info("step 1/7 — headroom");
    headroom::install(headroom_extras, full)?;

    ui::info("step 2/7 — rtk");
    rtk::install(full)?;

    ui::info("step 3/7 — rtk hook");
    rtk::configure()?;

    ui::info("step 4/7 — shell profile");
    shell::set_anthropic_base_url(DEFAULT_PROXY)?;
    shell::ensure_path_contains_local_bin()?;

    ui::info("step 5/7 — install whetstone binary");
    self_install()?;

    let install_memstack = prompt_memstack(full)?;

    if install_memstack {
        ui::info("step 6/7 — memstack");
        install_memstack_assets(&assets, full, headroom_extras)?;

        ui::info("step 7/7 — hooks + settings.json");
        let claude_dir = dirs::home_dir()
            .context("could not determine home directory")?
            .join(".claude");
        let hooks_dir = claude_dir.join("hooks");
        let settings_path = claude_dir.join("settings.json");

        hooks::copy_hook_scripts(&assets.join("hooks"), &hooks_dir)?;
        hooks::merge_settings_json(&settings_path, &hooks_dir)?;

        generate_stack_setup()?;
    } else {
        ui::info("skipped memstack skills, hooks, and STACK-SETUP.md");
    }

    ui::ok("whetstone setup complete");
    Ok(())
}

fn prompt_memstack(full: bool) -> Result<bool> {
    let project_dir = std::env::current_dir()?;
    let has_existing = project_dir.join(".claude/skills").is_dir()
        || project_dir.join(".claude/MEMSTACK.md").exists();

    if full {
        if has_existing {
            ui::info("full update: refreshing existing memstack install");
            return Ok(true);
        }
        ui::info("full update: no memstack install found — skipping");
        return Ok(false);
    }

    Ok(ui::confirm(
        "Install MemStack skills and hooks for this project?",
        true,
    ))
}

fn install_memstack_assets(assets: &Path, full: bool, headroom_extras: &str) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let claude_dir = project_dir.join(".claude");

    copy_skills(assets, &claude_dir, full)?;
    copy_subdirs(assets, &claude_dir, full)?;
    copy_memstack_md(assets, &claude_dir, full)?;

    let config_path = claude_dir.join("config.local.json");
    let cfg = config::WhetstoneConfig::new_for_project(&project_dir, headroom_extras);
    cfg.write_to(&config_path)?;
    ui::ok("created config.local.json");

    let db_dir = claude_dir.join("db");
    fs::create_dir_all(&db_dir)?;
    let db_path = db_dir.join("memstack.db");
    if !db_path.exists() {
        db::dispatch(crate::cli::DbCommand::Init)?;
        ui::ok("database initialized");
    } else {
        ui::ok("database already exists");
    }

    Ok(())
}

fn copy_skills(assets: &Path, claude_dir: &Path, force: bool) -> Result<()> {
    let src = assets.join("skills");
    if !src.is_dir() {
        ui::warn("no bundled skills found — skipping");
        return Ok(());
    }

    let dest = claude_dir.join("skills");

    if force {
        ui::info("refreshing skills...");
        copy_dir_recursive(&src, &dest)?;
        ui::ok("skills refreshed");
    } else if dest.is_dir() && has_subdirs(&dest) {
        ui::ok("skills already installed");
    } else {
        ui::info("copying skills...");
        copy_dir_recursive(&src, &dest)?;
        ui::ok("skills copied");
    }
    Ok(())
}

fn copy_subdirs(assets: &Path, claude_dir: &Path, force: bool) -> Result<()> {
    for subdir in ["rules", "commands"] {
        let src = assets.join(subdir);
        if !src.is_dir() {
            continue;
        }
        let dest = claude_dir.join(subdir);
        if force || !dest.is_dir() {
            copy_dir_recursive(&src, &dest)?;
        }
    }
    Ok(())
}

fn copy_memstack_md(assets: &Path, claude_dir: &Path, force: bool) -> Result<()> {
    let src = assets.join("MEMSTACK.md");
    if !src.exists() {
        return Ok(());
    }
    let dest = claude_dir.join("MEMSTACK.md");
    if force || !dest.exists() {
        fs::copy(&src, &dest)?;
    }
    Ok(())
}

fn generate_stack_setup() -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let dest = project_dir.join("STACK-SETUP.md");
    fs::write(&dest, STACK_SETUP_CONTENT)?;
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

fn has_subdirs(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|entries| entries.filter_map(|e| e.ok()).any(|e| e.path().is_dir()))
        .unwrap_or(false)
}

const STACK_SETUP_CONTENT: &str = r#"# Whetstone (Claude Code stack)

This project was set up with Whetstone: Headroom, RTK, and MemStack for
token-efficient Claude Code sessions.

## Quick Start

```bash
whetstone              # Start Claude with Headroom proxy
whetstone claude       # Same as above
```

## Tools

| Tool | Purpose | Savings |
|------|---------|---------|
| Headroom | HTTP proxy compresses context before API | 50-90% |
| RTK | Hook rewrites CLI output before entering context | 60-90% |
| MemStack | Skills, SQLite memory, session hooks | efficiency |

## Hooks

| Event | Hook | Tool |
|-------|------|------|
| Before Bash | RTK rewrites command | RTK |
| Before Write/Edit/Bash | TTS notification | MemStack |
| Before `git push` | Build check + secrets scan | MemStack |
| After `git commit` | Debug artifact scan | MemStack |
| Session start | Headroom auto-start + indexing | MemStack |
| Session end | Session reporting | MemStack |

## Configuration

| File | Purpose |
|------|---------|
| `~/.claude/settings.json` | Hook registrations (global) |
| `.claude/config.local.json` | Project config |
| `.claude/db/memstack.db` | SQLite database |

## Database CLI

```bash
whetstone db stats
whetstone db search "query"
whetstone db get-sessions
whetstone db export-md
```

## Uninstall

Per-project: `whetstone uninstall`
"#;

fn self_install() -> Result<()> {
    let current_exe = std::env::current_exe()
        .context("could not determine current executable path")?;

    let home = dirs::home_dir().context("could not determine home directory")?;
    let bin_dir = home.join(".local").join("bin");
    fs::create_dir_all(&bin_dir)?;

    let dest = bin_dir.join("whetstone");

    if dest.exists() && same_file(&current_exe, &dest) {
        ui::ok("whetstone binary already in place");
        return Ok(());
    }

    fs::copy(&current_exe, &dest)
        .with_context(|| format!("copying binary to {}", dest.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
    }

    ui::ok(&format!("installed to {}", dest.display()));
    Ok(())
}

fn same_file(a: &PathBuf, b: &PathBuf) -> bool {
    let Ok(a_canon) = fs::canonicalize(a) else { return false };
    let Ok(b_canon) = fs::canonicalize(b) else { return false };
    a_canon == b_canon
}
