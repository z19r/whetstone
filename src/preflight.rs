use anyhow::{bail, Result};
use std::process::Command;

use crate::ui;
use crate::version;

pub fn check_all() -> Result<()> {
    check_git_repo()?;
    check_python()?;
    check_git()?;
    check_curl()?;
    check_uv()?;
    Ok(())
}

fn check_git_repo() -> Result<()> {
    let ok = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !ok {
        bail!("not inside a git repository — run whetstone setup from a project root");
    }
    ui::ok("inside git repository");
    Ok(())
}

fn check_python() -> Result<()> {
    let output = Command::new("python3")
        .args([
            "-c",
            "import sys; v=sys.version_info; print(f'{v.major}.{v.minor}')",
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let parts: Vec<&str> = ver.split('.').collect();
            let major: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let minor: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            if major < 3 || (major == 3 && minor < 10) {
                bail!("Python 3.10+ required, found {ver}");
            }
            ui::ok(&format!("python3 {ver}"));
            Ok(())
        }
        _ => bail!("python3 not found — install Python 3.10+"),
    }
}

fn check_git() -> Result<()> {
    let output = Command::new("git").arg("--version").output();
    match output {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let ver = version::extract_semver(&raw).unwrap_or_else(|| "unknown".into());
            ui::ok(&format!("git {ver}"));
            Ok(())
        }
        _ => bail!("git not found"),
    }
}

fn check_curl() -> Result<()> {
    if which::which("curl").is_ok() {
        ui::ok("curl");
        Ok(())
    } else {
        bail!("curl not found")
    }
}

fn check_uv() -> Result<()> {
    if which::which("uv").is_ok() {
        ui::ok("uv");
        Ok(())
    } else {
        bail!("uv not found — install from https://docs.astral.sh/uv/")
    }
}
