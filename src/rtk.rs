use anyhow::{bail, Result};
use std::process::Command;

use crate::ui;
use crate::version;

const MIN_VERSION: &str = "0.7.0";
const INSTALL_URL: &str =
    "https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh";

fn installed_version() -> Option<String> {
    let output = Command::new("rtk").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    version::extract_semver(&raw)
}

fn is_rtk_ai() -> bool {
    Command::new("rtk")
        .arg("gain")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn install(force: bool) -> Result<()> {
    if let Some(ver) = installed_version() {
        if is_rtk_ai() {
            if !force && !version::is_older(&ver, MIN_VERSION) {
                ui::ok(&format!("rtk {ver} (>= {MIN_VERSION})"));
                return Ok(());
            }
            ui::info(&format!("upgrading rtk from {ver}"));
        } else {
            ui::warn("found rtk binary but it's not rtk-ai (Rust Type Kit collision?)");
            ui::info("installing rtk-ai alongside existing binary");
        }
    } else {
        ui::info("installing rtk");
    }

    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("curl -fsSL {INSTALL_URL} | sh"))
        .status()?;

    if !status.success() {
        bail!("rtk installation failed");
    }

    match installed_version() {
        Some(ver) if is_rtk_ai() => ui::ok(&format!("rtk {ver}")),
        _ => bail!("rtk installation completed but rtk-ai not detected"),
    }
    Ok(())
}

pub fn configure() -> Result<()> {
    ui::info("configuring rtk global hook");

    let home = dirs::home_dir().unwrap_or_default();
    let hooks_dir = home.join(".claude").join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    let status = Command::new("rtk")
        .args(["init", "-g", "--hook-only", "--auto-patch"])
        .status();

    match status {
        Ok(s) if s.success() => {
            ui::ok("rtk hook configured");
            Ok(())
        }
        _ => {
            ui::warn("rtk init -g failed — hook may need manual setup");
            Ok(())
        }
    }
}
