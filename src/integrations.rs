//! Thin orchestration layer over each tool's own `init` command.
//!
//! Whetstone v3 stops hand-writing Claude Code hooks. Instead, it delegates
//! to `rtk init` and `icm init`, which know their own integration shape best
//! and stay in sync with their own releases. This module captures stdout/
//! stderr from those commands and normalizes errors into `anyhow::Result`.
//!
//! The interface contract for these commands is recorded in
//! `docs/interface-contract.md` (Phase 0 deliverable).

use anyhow::{bail, Context, Result};
use std::process::{Command, Output};

use crate::memory::MemoryProvider;
use crate::ui;

/// Mode passed to `icm init --mode`. Matches the verified surface in
/// `docs/interface-contract.md` §0.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcmMode {
    /// `standard` — cli + skill + hook (no MCP). The default.
    Standard,
    /// `all` — everything including MCP server. Opt-in only.
    #[allow(dead_code)] // wired by setup once `--mode all` is exposed in cli.
    All,
}

impl IcmMode {
    fn as_arg(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::All => "all",
        }
    }
}

/// Result of running the tools' own inits. The actual hook entries land in
/// `~/.claude/settings.json` via the tools themselves; whetstone never writes
/// the hook JSON directly in v3.
#[derive(Debug, Default)]
pub struct IntegrationReport {
    pub rtk_ran: bool,
    pub icm_ran: bool,
}

/// Run `rtk init --auto-patch` so RTK installs its own Claude Code hook
/// (PreToolUse Bash) and merges itself into `~/.claude/settings.json`.
pub fn rtk_init() -> Result<()> {
    require_binary("rtk")?;

    ui::info("running `rtk init --auto-patch`");
    let output = Command::new("rtk")
        .args(["init", "--auto-patch"])
        .output()
        .context("failed to spawn `rtk init`")?;

    finish("rtk init", &output)
}

/// Run `icm init --mode <mode>` so ICM installs its own slash commands,
/// CLAUDE.md additions, and Claude Code hooks.
pub fn icm_init(mode: IcmMode) -> Result<()> {
    require_binary("icm")?;

    ui::info(&format!("running `icm init --mode {}`", mode.as_arg()));
    let output = Command::new("icm")
        .args(["init", "--mode", mode.as_arg()])
        .output()
        .context("failed to spawn `icm init`")?;

    finish("icm init", &output)
}

/// Orchestrate every tool's init in the order whetstone v3 expects:
/// RTK first (so its PreToolUse hook is in place), then the memory provider.
pub fn run_all(provider: MemoryProvider) -> Result<IntegrationReport> {
    let mut report = IntegrationReport::default();

    rtk_init()?;
    report.rtk_ran = true;

    match provider {
        MemoryProvider::Icm => {
            icm_init(IcmMode::Standard)?;
            report.icm_ran = true;
        }
        MemoryProvider::Skip => {
            ui::info("memory provider skipped");
        }
    }

    Ok(report)
}

fn require_binary(name: &str) -> Result<()> {
    which::which(name).with_context(|| format!("`{name}` not found on PATH"))?;
    Ok(())
}

fn finish(label: &str, output: &Output) -> Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stdout.trim().is_empty() {
        for line in stdout.lines() {
            ui::info(line);
        }
    }

    if !output.status.success() {
        if !stderr.trim().is_empty() {
            for line in stderr.lines() {
                ui::warn(line);
            }
        }
        bail!("{label} failed (exit {:?})", output.status.code());
    }

    ui::ok(&format!("{label} completed"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icm_mode_serializes_as_lowercase() {
        assert_eq!(IcmMode::Standard.as_arg(), "standard");
        assert_eq!(IcmMode::All.as_arg(), "all");
    }

    #[test]
    fn icm_default_mode_matches_interface_contract() {
        // Phase 2.3 regression: the interface contract pins
        // `icm init --mode standard` as the v3 default invocation.
        // See docs/interface-contract.md §0.2. The Phase 2 prompt's claim
        // that `--mode standard` was invalid turned out to be stale — the
        // contract verifies `standard` is in fact the documented default.
        let default_invocation = ["init", "--mode", IcmMode::Standard.as_arg()];
        assert_eq!(default_invocation, ["init", "--mode", "standard"]);
    }

    #[test]
    fn integration_report_defaults_false() {
        let r = IntegrationReport::default();
        assert!(!r.rtk_ran);
        assert!(!r.icm_ran);
    }
}
