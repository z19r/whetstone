use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::ui;

pub fn run() -> Result<()> {
    ui::info("whetstone uninstall");
    let project_dir = std::env::current_dir()?;
    eprintln!("project dir: {}", project_dir.display());

    remove_bins();

    if ui::confirm("Remove RTK (global)?", false) {
        remove_rtk();
    } else {
        ui::warn("skipped RTK removal");
    }

    if ui::confirm("Remove Headroom package?", false) {
        remove_headroom();
    } else {
        ui::warn("skipped Headroom removal");
    }

    if ui::confirm("Remove MemStack from this project directory?", false) {
        remove_project_memstack(&project_dir);
    } else {
        ui::warn("skipped project MemStack removal");
    }

    ui::warn("review shell rc files and remove ANTHROPIC_BASE_URL if unwanted");
    ui::info("restore ~/.claude/settings.json from .bak.* backups if needed");
    ui::ok("whetstone uninstall finished");
    Ok(())
}

fn remove_bins() {
    ui::info("removing whetstone wrappers from ~/.local/bin...");
    let home = dirs::home_dir().unwrap_or_default();
    let bin = home.join(".local").join("bin");
    let _ = fs::remove_file(bin.join("whetstone"));
    let _ = fs::remove_file(bin.join("whetstone-rtk"));
    ui::ok("removed whetstone, whetstone-rtk (if present)");
}

fn remove_rtk() {
    let rtk_ok = Command::new("rtk")
        .arg("gain")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if rtk_ok {
        let _ = Command::new("rtk")
            .args(["init", "-g", "--uninstall"])
            .status();
    }

    let home = dirs::home_dir().unwrap_or_default();
    let _ = fs::remove_file(home.join(".local/bin/rtk"));
    let _ = fs::remove_dir_all(home.join(".local/share/rtk"));
    ui::ok("RTK removed (or was absent)");
}

fn remove_headroom() {
    if which::which("uv").is_ok() {
        let tried_pip = Command::new("uv")
            .args(["pip", "uninstall", "-y", "headroom-ai"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !tried_pip {
            let _ = Command::new("uv")
                .args(["tool", "uninstall", "headroom-ai"])
                .status();
        }
    }
    ui::ok("headroom uninstall attempted");
}

fn remove_project_memstack(project_dir: &Path) {
    let claude = project_dir.join(".claude");
    if !claude.join("skills").is_dir() {
        ui::info(&format!("no .claude/skills in {}", project_dir.display()));
        return;
    }

    for dir in ["skills", "db", "rules", "commands"] {
        let _ = fs::remove_dir_all(claude.join(dir));
    }
    for file in ["MEMSTACK.md", "config.local.json"] {
        let _ = fs::remove_file(claude.join(file));
    }
    let _ = fs::remove_file(project_dir.join("STACK-SETUP.md"));

    ui::ok("project MemStack files removed");
}
