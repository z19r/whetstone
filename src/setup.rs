use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::memory::MemoryProvider;
use crate::{config, headroom, hooks, preflight, rtk, shell, ui};

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

    let provider = prompt_memory_provider(full)?;

    if provider != MemoryProvider::Skip {
        ui::info("step 6/8 — skills, rules, commands");
        install_general_assets(&assets, full, headroom_extras)?;

        ui::info(&format!("step 7/8 — {} provider", provider.name()));
        install_provider(provider)?;

        ui::info("step 8/8 — hooks + settings.json");
        let claude_dir = dirs::home_dir()
            .context("could not determine home directory")?
            .join(".claude");
        let hooks_dir = claude_dir.join("hooks");
        let settings_path = claude_dir.join("settings.json");

        hooks::copy_hook_scripts(&assets.join("hooks"), &hooks_dir)?;
        hooks::merge_settings_json(&settings_path, &hooks_dir, provider)?;

        generate_stack_setup(provider)?;
    } else {
        ui::info("skipped memory provider, skills, hooks, and STACK-SETUP.md");
    }

    ui::ok("whetstone setup complete");
    Ok(())
}

fn prompt_memory_provider(full: bool) -> Result<MemoryProvider> {
    let project_dir = std::env::current_dir()?;
    let has_existing = project_dir.join(".claude/skills").is_dir()
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

fn detect_installed_provider() -> Result<MemoryProvider> {
    let settings_path = dirs::home_dir()
        .context("home directory")?
        .join(".claude/settings.json");

    if !settings_path.exists() {
        return Ok(MemoryProvider::Icm);
    }

    let content = fs::read_to_string(&settings_path).unwrap_or_default();
    if content.contains("icm hook") || content.contains("icm serve") {
        Ok(MemoryProvider::Icm)
    } else if content.contains("mcp-automem") {
        Ok(MemoryProvider::AutoMem)
    } else {
        Ok(MemoryProvider::Icm)
    }
}

fn install_general_assets(assets: &Path, full: bool, headroom_extras: &str) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let claude_dir = project_dir.join(".claude");

    copy_skills(assets, &claude_dir, full)?;
    copy_subdirs(assets, &claude_dir, full)?;
    copy_memstack_md(assets, &claude_dir, full)?;

    let config_path = claude_dir.join("config.local.json");
    let cfg = config::WhetstoneConfig::new_for_project(&project_dir, headroom_extras);
    cfg.write_to(&config_path)?;
    ui::ok("created config.local.json");

    Ok(())
}

fn install_provider(provider: MemoryProvider) -> Result<()> {
    match provider {
        MemoryProvider::Icm => install_icm(),
        MemoryProvider::AutoMem => install_automem(),
        MemoryProvider::Skip => Ok(()),
    }
}

fn install_icm() -> Result<()> {
    if which::which("icm").is_ok() {
        let output = std::process::Command::new("icm").arg("--version").output();
        if let Ok(o) = output {
            if o.status.success() {
                let ver = String::from_utf8_lossy(&o.stdout).trim().to_string();
                ui::ok(&format!("icm already installed ({ver})"));
                run_icm_init()?;
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

    run_icm_init()?;
    ui::ok("ICM installed and configured");
    Ok(())
}

fn run_icm_init() -> Result<()> {
    let status = std::process::Command::new("icm")
        .args(["init", "--mode", "standard"])
        .status()
        .context("failed to run icm init")?;

    if !status.success() {
        ui::warn("icm init returned non-zero — hooks may need manual setup");
    }
    Ok(())
}

fn install_automem() -> Result<()> {
    preflight::check_npm()?;

    ui::info("installing AutoMem...");
    let status = std::process::Command::new("npx")
        .args(["-y", "@verygoodplugins/mcp-automem", "claude-code"])
        .status()
        .context("failed to run AutoMem installer")?;

    if !status.success() {
        bail!("AutoMem installation failed");
    }

    ui::ok("AutoMem installed");
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

fn generate_stack_setup(provider: MemoryProvider) -> Result<()> {
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

fn has_subdirs(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|entries| entries.filter_map(|e| e.ok()).any(|e| e.path().is_dir()))
        .unwrap_or(false)
}

fn stack_setup_content(provider: MemoryProvider) -> String {
    let provider_row = match provider {
        MemoryProvider::Icm => {
            "| ICM | Embedded SQLite memory, zero dependencies | persistent context |"
        }
        MemoryProvider::AutoMem => {
            "| AutoMem | Graph memory via MCP (FalkorDB + Qdrant) | persistent context |"
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
| `~/.claude/settings.json` | Hook registrations (global) |
| `.claude/config.local.json` | Project config |

## Uninstall

Per-project: `whetstone uninstall`
"#,
        provider = provider.name(),
        provider_row = provider_row,
    )
}

fn self_install() -> Result<()> {
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
    let Ok(a_canon) = fs::canonicalize(a) else {
        return false;
    };
    let Ok(b_canon) = fs::canonicalize(b) else {
        return false;
    };
    a_canon == b_canon
}
