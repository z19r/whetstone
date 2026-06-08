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

/// Arg shape for `rtk init`. Pinned by `docs/interface-contract.md` §0.2;
/// changing it is an integration-version bump.
pub(crate) const RTK_INIT_ARGS: [&str; 2] = ["init", "--auto-patch"];

/// Arg shape for `icm init`. The `--mode` value is filled in at call time
/// from [`IcmMode::as_arg`].
pub(crate) fn icm_init_args(mode: IcmMode) -> [&'static str; 3] {
    ["init", "--mode", mode.as_arg()]
}

/// Run `rtk init --auto-patch` so RTK installs its own Claude Code hook
/// (PreToolUse Bash) and merges itself into `~/.claude/settings.json`.
pub fn rtk_init() -> Result<()> {
    require_binary("rtk")?;

    ui::info("running `rtk init --auto-patch`");
    let output = Command::new("rtk")
        .args(RTK_INIT_ARGS)
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
        .args(icm_init_args(mode))
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

    #[test]
    fn rtk_init_args_pinned_to_interface_contract() {
        // docs/interface-contract.md §0.2 pins `rtk init --auto-patch`. If this
        // ever drifts, bump the integration version on the same PR.
        assert_eq!(RTK_INIT_ARGS, ["init", "--auto-patch"]);
    }

    #[test]
    fn icm_init_args_pinned_to_interface_contract_for_each_mode() {
        // Same contract anchor as above — both modes must shape exactly the
        // documented invocation.
        assert_eq!(
            icm_init_args(IcmMode::Standard),
            ["init", "--mode", "standard"]
        );
        assert_eq!(icm_init_args(IcmMode::All), ["init", "--mode", "all"]);
    }

    #[test]
    fn require_binary_errors_for_missing_command() {
        // Pick a name no sane system has installed. The error message must
        // include the binary name so the user can act on it.
        let err = require_binary("definitely-not-a-real-binary-xyz123").unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("definitely-not-a-real-binary-xyz123"),
            "error message should name the missing binary, got: {msg}"
        );
        assert!(
            msg.contains("not found on PATH"),
            "error message should mention PATH lookup, got: {msg}"
        );
    }

    #[cfg(unix)]
    fn synth_output(code: i32, stdout: &str, stderr: &str) -> Output {
        use std::os::unix::process::ExitStatusExt;
        Output {
            status: std::process::ExitStatus::from_raw((code & 0xff) << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn finish_returns_ok_on_zero_exit() {
        let output = synth_output(0, "hello from init\n", "");
        assert!(finish("test init", &output).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn finish_errors_on_nonzero_exit_and_surfaces_label() {
        let output = synth_output(2, "", "boom\n");
        let err = finish("rtk init", &output).unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("rtk init failed"),
            "error should mention the label, got: {msg}"
        );
    }
}
