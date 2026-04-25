use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

pub fn detect_profile() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let shell = std::env::var("SHELL").unwrap_or_default();

    if shell.ends_with("zsh") {
        let p = home.join(".zshrc");
        if p.exists() {
            return Some(p);
        }
    }
    if shell.ends_with("bash") {
        let p = home.join(".bashrc");
        if p.exists() {
            return Some(p);
        }
    }

    for name in [".zshrc", ".bashrc", ".profile"] {
        let p = home.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

pub fn ensure_in_profile(line: &str) -> Result<()> {
    let Some(profile) = detect_profile() else {
        crate::ui::warn("could not detect shell profile — set ANTHROPIC_BASE_URL manually");
        return Ok(());
    };

    let contents =
        fs::read_to_string(&profile).with_context(|| format!("reading {}", profile.display()))?;

    if contents.contains(line) {
        return Ok(());
    }

    let mut appended = contents;
    if !appended.ends_with('\n') {
        appended.push('\n');
    }
    appended.push_str(line);
    appended.push('\n');

    fs::write(&profile, appended).with_context(|| format!("writing to {}", profile.display()))?;

    crate::ui::ok(&format!("appended to {}", profile.display()));
    Ok(())
}

pub fn ensure_path_contains_local_bin() -> Result<()> {
    ensure_in_profile("export PATH=\"$HOME/.local/bin:$PATH\"")
}

pub fn set_anthropic_base_url(url: &str) -> Result<()> {
    std::env::set_var("ANTHROPIC_BASE_URL", url);
    ensure_in_profile(&format!("export ANTHROPIC_BASE_URL={url}"))
}
