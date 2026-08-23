//! Managed-dependency inventory: what whetstone installs, whether it is still
//! there, and how to put it back.
//!
//! `whetstone setup` installs Headroom, RTK, Claude Code's memory provider
//! (ICM) and wires their hooks. Nothing kept those installs honest afterwards:
//! `uv tool uninstall headroom-ai`, a wiped `~/.local/bin`, or a Python
//! upgrade that orphans a shim all left the project silently half-broken —
//! `whetstone doctor` only ever looked at `~/.claude/settings.json`.
//!
//! This module is the missing half: a single place that knows the expected
//! tool set, classifies each tool as present/broken/missing, and can reinstall
//! and re-integrate the ones that went away. `whetstone doctor` consumes it
//! (prompting before it touches anything) and `whetstone install-tools` drives
//! it directly.

use anyhow::{bail, Context, Result};
use std::process::Command;

use crate::config::{ToolVersions, WhetstoneManifest};
use crate::memory::MemoryProvider;
use crate::{claude_code, headroom, icm, integrations, rtk, ui};

/// A dependency whetstone installs and is therefore responsible for repairing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Headroom,
    Rtk,
    ClaudeCode,
    Icm,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Headroom => "headroom",
            Self::Rtk => "rtk",
            Self::ClaudeCode => "claude code",
            Self::Icm => "memory (ICM)",
        }
    }

    /// Reverse of [`Tool::binary`] — used by the launch-time guard, which
    /// only knows the program name whetstone is about to exec.
    pub fn from_binary(binary: &str) -> Option<Self> {
        [Self::Headroom, Self::Rtk, Self::ClaudeCode, Self::Icm]
            .into_iter()
            .find(|tool| tool.binary() == binary)
    }

    /// Executable whetstone expects to find on `PATH`.
    pub fn binary(self) -> &'static str {
        match self {
            Self::Headroom => "headroom",
            Self::Rtk => "rtk",
            Self::ClaudeCode => "claude",
            Self::Icm => "icm",
        }
    }

    fn installed_version(self) -> Option<String> {
        match self {
            Self::Headroom => headroom::installed_version(),
            Self::Rtk => rtk::installed_version(),
            Self::ClaudeCode => claude_code::installed_version(),
            Self::Icm => icm::installed_version(),
        }
    }

    /// Classify the tool without changing anything. `extras` is the headroom
    /// extras set the caller expects (`"all"` for every default path) — a
    /// headroom whose uv receipt is missing one of them is `Incomplete`, not
    /// `Present`, because `headroom proxy`/`mcp` won't exist at runtime.
    pub fn presence(self, extras: &str) -> Presence {
        if which::which(self.binary()).is_err() {
            return Presence::Missing;
        }
        let Some(version) = self.installed_version() else {
            return Presence::Broken;
        };
        if self == Self::Headroom {
            let missing = headroom::missing_extras(extras);
            if !missing.is_empty() {
                return Presence::Incomplete { version, missing };
            }
        }
        Presence::Present(version)
    }

    /// (Re)install the tool. `force` upgrades even when a good-enough version
    /// is already present.
    pub fn install(self, extras: &str, force: bool) -> Result<()> {
        match self {
            Self::Headroom => headroom::install(extras, force),
            Self::Rtk => rtk::install(force),
            Self::Icm => icm::install(force),
            Self::ClaudeCode => claude_code::install(),
        }
    }

    /// Re-run the tool's own `init` so its Claude Code hooks come back after a
    /// reinstall. Tools that own no hooks are a no-op.
    pub fn reintegrate(self) -> Result<()> {
        match self {
            Self::Rtk => integrations::rtk_init(),
            Self::Icm => {
                integrations::icm_init(integrations::IcmMode::Standard)
            }
            Self::Headroom | Self::ClaudeCode => Ok(()),
        }
    }
}

/// Result of looking for a tool on the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    /// Installed and reporting a version.
    Present(String),
    /// On `PATH` but `--version` failed — a dead shim, broken venv, or a
    /// partially-removed install. Treated like missing for repair purposes.
    Broken,
    /// Runs, but was installed without extras whetstone depends on (headroom
    /// installed as bare `headroom-ai` instead of `headroom-ai[proxy,code,mcp]`).
    Incomplete {
        version: String,
        missing: Vec<String>,
    },
    /// Not on `PATH` at all.
    Missing,
}

impl Presence {
    /// Short human-readable state, e.g. "0.22.2" or "not installed".
    pub fn describe(&self) -> String {
        match self {
            Self::Present(ver) => ver.clone(),
            Self::Broken => "installed but not runnable".into(),
            Self::Incomplete { version, missing } => format!(
                "{version}, installed without extras: {}",
                missing.join(", "),
            ),
            Self::Missing => "not installed".into(),
        }
    }

    /// Verb for the repair prompt — nothing to re-do when it was never there.
    fn repair_verb(&self) -> &'static str {
        match self {
            Self::Missing => "install",
            _ => "reinstall",
        }
    }
}

/// How aggressively a repair pass may act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairMode {
    /// Report only — never install (used by `doctor` in non-interactive runs
    /// and by the doctor pass at the end of `install-tools`).
    Report,
    /// Ask before installing each missing tool.
    Prompt,
    /// Install without asking.
    Force,
}

impl RepairMode {
    /// Prompting requires a TTY; degrade to reporting when there isn't one so
    /// `whetstone doctor` stays usable in scripts and CI.
    pub fn effective(self) -> Self {
        match self {
            Self::Prompt if !ui::is_interactive() => Self::Report,
            other => other,
        }
    }
}

/// Outcome for one tool after a repair pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOutcome {
    /// Was already there.
    Ok(String),
    /// Was missing/broken and is now installed.
    Repaired { from: Presence, version: String },
    /// Was missing/broken; repair not attempted (report mode or user declined).
    Unrepaired(Presence),
    /// Repair was attempted and failed.
    Failed { presence: Presence, error: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolReport {
    pub tool: Tool,
    pub outcome: ToolOutcome,
}

impl ToolReport {
    pub fn is_healthy(&self) -> bool {
        matches!(
            self.outcome,
            ToolOutcome::Ok(_) | ToolOutcome::Repaired { .. }
        )
    }
}

/// The tools whetstone expects on a machine, given the project's memory
/// provider. `None` means "no project manifest here" — the global trio is
/// still expected, but no provider is assumed.
pub fn expected_tools(provider: Option<MemoryProvider>) -> Vec<Tool> {
    let mut tools = vec![Tool::Headroom, Tool::Rtk, Tool::ClaudeCode];
    if provider == Some(MemoryProvider::Icm) {
        tools.push(Tool::Icm);
    }
    tools
}

/// Memory provider recorded in this directory's manifest, if any.
pub fn project_provider() -> Option<MemoryProvider> {
    let project_dir = std::env::current_dir().ok()?;
    let manifest_path = WhetstoneManifest::path_for(&project_dir);
    let manifest = WhetstoneManifest::load(&manifest_path).ok()??;
    Some(manifest.provider.into())
}

/// Inspect every expected tool and, depending on `mode`, put back the ones
/// that went away. Repaired hook-owning tools get their `init` re-run so
/// `~/.claude/settings.json` is repopulated before the caller inspects it.
pub fn repair(
    provider: Option<MemoryProvider>,
    mode: RepairMode,
    extras: &str,
) -> Vec<ToolReport> {
    let mode = mode.effective();
    expected_tools(provider)
        .into_iter()
        .map(|tool| ToolReport {
            tool,
            outcome: repair_one(tool, mode, extras),
        })
        .collect()
}

fn repair_one(tool: Tool, mode: RepairMode, extras: &str) -> ToolOutcome {
    let presence = match tool.presence(extras) {
        Presence::Present(ver) => return ToolOutcome::Ok(ver),
        gone => gone,
    };

    match mode {
        RepairMode::Report => return ToolOutcome::Unrepaired(presence),
        RepairMode::Prompt => {
            let prompt = format!(
                "{} is {} — {} it now?",
                tool.label(),
                presence.describe(),
                presence.repair_verb(),
            );
            if !ui::confirm(&prompt, true) {
                return ToolOutcome::Unrepaired(presence);
            }
        }
        RepairMode::Force => {
            ui::info(&format!(
                "{} is {} — {}ing",
                tool.label(),
                presence.describe(),
                presence.repair_verb(),
            ));
        }
    }

    if let Err(e) = tool.install(extras, false) {
        return ToolOutcome::Failed {
            presence,
            error: format!("{e:#}"),
        };
    }

    let after = tool.presence(extras);
    let Presence::Present(version) = after else {
        return ToolOutcome::Failed {
            presence,
            error: format!(
                "install completed but `{}` still {}",
                tool.binary(),
                after.describe(),
            ),
        };
    };

    // A reinstalled tool has no hooks in ~/.claude/settings.json until its own
    // init runs again — that is the "fix whatever else broke" half.
    if let Err(e) = tool.reintegrate() {
        ui::warn(&format!(
            "{} reinstalled but re-integration failed: {e:#}",
            tool.label(),
        ));
    }

    ToolOutcome::Repaired {
        from: presence,
        version,
    }
}

/// Print a repair pass in the same shape as the rest of whetstone's output.
pub fn print_reports(reports: &[ToolReport]) {
    for report in reports {
        let label = report.tool.label();
        match &report.outcome {
            ToolOutcome::Ok(ver) => ui::ok(&format!("{label} {ver}")),
            ToolOutcome::Repaired { from, version } => ui::ok(&format!(
                "{label}: reinstalled ({} → {version})",
                from.describe(),
            )),
            ToolOutcome::Unrepaired(presence) => ui::warn(&format!(
                "{label}: {} — run `whetstone install-tools` to reinstall",
                presence.describe(),
            )),
            ToolOutcome::Failed { presence, error } => ui::warn(&format!(
                "{label}: {} and reinstall failed: {error}",
                presence.describe(),
            )),
        }
    }
}

/// Guard the launch path: whetstone is about to `exec` a managed tool, so make
/// sure it is actually there. Deliberately a `which` lookup rather than a
/// `--version` spawn — this runs on every `whetstone claude` and must stay
/// cheap. Exits the process (with a pointer to `install-tools`) when the tool
/// is missing and could not be installed.
pub fn ensure_available(binary: &str) {
    let Some(tool) = Tool::from_binary(binary) else {
        return;
    };
    if which::which(binary).is_ok() {
        return;
    }

    ui::warn(&format!("{} is not installed", tool.label()));

    let may_install = ui::is_interactive()
        && ui::confirm(&format!("Install {} now?", tool.label()), true);

    if may_install {
        match tool.install("all", false) {
            Ok(()) => {
                if let Err(e) = tool.reintegrate() {
                    ui::warn(&format!("re-integration failed: {e:#}"));
                }
                if which::which(binary).is_ok() {
                    return;
                }
            }
            Err(e) => ui::warn(&format!("install failed: {e:#}")),
        }
    }

    ui::fail(&format!(
        "`{binary}` not found — run `whetstone install-tools` to repair this install"
    ));
}

/// Record the versions we ended up with in the project manifest, so
/// `dashboard`/`update` don't keep reporting the pre-repair state. No-op when
/// this directory has no manifest.
pub fn sync_manifest_versions() -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let manifest_path = WhetstoneManifest::path_for(&project_dir);
    let Some(mut manifest) = WhetstoneManifest::load(&manifest_path)? else {
        return Ok(());
    };

    let tools = ToolVersions {
        rtk: rtk::installed_version(),
        icm: icm::installed_version(),
        headroom: headroom::installed_version(),
    };
    if manifest.tool_versions == tools {
        return Ok(());
    }

    manifest.tool_versions = tools;
    manifest
        .touch_and_save(&manifest_path)
        .with_context(|| format!("updating {}", manifest_path.display()))
}

/// A system dependency whetstone needs but does not own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prereq {
    pub name: &'static str,
    pub hint: &'static str,
    /// Whether whetstone knows a safe way to install it unattended.
    pub installable: bool,
}

const PREREQS: &[Prereq] = &[
    Prereq {
        name: "uv",
        hint: "https://docs.astral.sh/uv/ — required to install headroom",
        installable: true,
    },
    Prereq {
        name: "python3",
        hint: "install Python 3.10+ — headroom runs on it",
        installable: false,
    },
    Prereq {
        name: "git",
        hint: "install git — whetstone operates on git projects",
        installable: false,
    },
    Prereq {
        name: "curl",
        hint: "install curl — used by the rtk and icm installers",
        installable: false,
    },
];

/// System dependencies that are currently absent.
pub fn missing_prereqs() -> Vec<&'static Prereq> {
    PREREQS
        .iter()
        .filter(|p| which::which(p.name).is_err())
        .collect()
}

/// Install the system dependencies whetstone knows how to install (today:
/// `uv`) and return the ones still missing. Callers report — this only prints
/// progress for work it actually does.
pub fn repair_prereqs(mode: RepairMode) -> Vec<&'static Prereq> {
    let mode = mode.effective();
    let mut still_missing = Vec::new();

    for prereq in missing_prereqs() {
        let attempt = prereq.installable
            && match mode {
                RepairMode::Report => false,
                RepairMode::Force => true,
                RepairMode::Prompt => ui::confirm(
                    &format!("{} is missing — install it now?", prereq.name),
                    true,
                ),
            };

        if !attempt {
            still_missing.push(prereq);
            continue;
        }

        match install_prereq(prereq) {
            Ok(()) => ui::ok(&format!("{} installed", prereq.name)),
            Err(e) => {
                ui::warn(&format!("{} install failed: {e:#}", prereq.name));
                still_missing.push(prereq);
            }
        }
    }

    still_missing
}

fn install_prereq(prereq: &Prereq) -> Result<()> {
    match prereq.name {
        "uv" => install_uv(),
        other => bail!("no unattended installer for {other}"),
    }
}

fn install_uv() -> Result<()> {
    ui::info("installing uv");
    let status = Command::new("sh")
        .arg("-c")
        .arg("curl -LsSf https://astral.sh/uv/install.sh | sh")
        .status()
        .context("failed to run the uv install script")?;

    if !status.success() {
        bail!("uv install script failed");
    }
    if which::which("uv").is_err() {
        bail!("uv not on PATH after install — restart your shell and retry");
    }
    Ok(())
}

/// `whetstone install-tools` — install or repair every managed dependency.
pub fn run_install_tools(force: bool, extras: &str) -> Result<()> {
    ui::section("whetstone install-tools");

    let mode = if force {
        RepairMode::Force
    } else if ui::is_interactive() {
        RepairMode::Prompt
    } else {
        RepairMode::Force
    };

    let blocked = repair_prereqs(mode);
    for prereq in &blocked {
        ui::warn(&format!("{} missing — {}", prereq.name, prereq.hint));
    }
    if blocked.iter().any(|p| p.name == "uv") {
        ui::warn("headroom cannot be installed without uv");
    }

    let provider = project_provider();
    let mut reports = repair(provider, mode, extras);

    // `--force` means "reinstall everything", not just what disappeared.
    if force {
        for report in &mut reports {
            if let ToolOutcome::Ok(ver) = &report.outcome {
                let tool = report.tool;
                ui::info(&format!("forcing reinstall of {}", tool.label()));
                match tool.install(extras, true).and_then(|()| {
                    tool.reintegrate().map(|()| tool.presence(extras))
                }) {
                    Ok(Presence::Present(new_ver)) if &new_ver != ver => {
                        report.outcome = ToolOutcome::Repaired {
                            from: Presence::Present(ver.clone()),
                            version: new_ver,
                        };
                    }
                    Ok(_) => {}
                    Err(e) => {
                        report.outcome = ToolOutcome::Failed {
                            presence: Presence::Present(ver.clone()),
                            error: format!("{e:#}"),
                        };
                    }
                }
            }
        }
    }

    print_reports(&reports);

    // Keep whetstone itself reachable — a wiped ~/.local/bin takes the
    // symlink with it.
    if let Err(e) = crate::setup::self_install() {
        ui::warn(&format!("whetstone self-install failed: {e:#}"));
    }

    if let Err(e) = sync_manifest_versions() {
        ui::warn(&format!("could not update project manifest: {e:#}"));
    }

    // Report-only doctor: repairs already happened above, and this confirms
    // the hooks landed.
    let doctor_report = crate::doctor::run_with(RepairMode::Report, extras)?;

    let unhealthy: Vec<&ToolReport> =
        reports.iter().filter(|r| !r.is_healthy()).collect();
    if unhealthy.is_empty() && doctor_report.green() {
        ui::summary_ok("All whetstone dependencies are installed");
    } else {
        ui::summary_info("Some dependencies still need attention — see above");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icm_expected_only_for_icm_projects() {
        assert_eq!(
            expected_tools(Some(MemoryProvider::Icm)),
            vec![Tool::Headroom, Tool::Rtk, Tool::ClaudeCode, Tool::Icm],
        );
        assert_eq!(
            expected_tools(Some(MemoryProvider::Skip)),
            vec![Tool::Headroom, Tool::Rtk, Tool::ClaudeCode],
        );
        assert_eq!(
            expected_tools(None),
            vec![Tool::Headroom, Tool::Rtk, Tool::ClaudeCode],
        );
    }

    #[test]
    fn presence_describes_each_state() {
        assert_eq!(Presence::Present("1.2.3".into()).describe(), "1.2.3");
        assert_eq!(Presence::Missing.describe(), "not installed");
        assert_eq!(Presence::Broken.describe(), "installed but not runnable");
        assert_eq!(
            Presence::Incomplete {
                version: "0.36.1".into(),
                missing: vec!["mcp".into()],
            }
            .describe(),
            "0.36.1, installed without extras: mcp",
        );
    }

    #[test]
    fn repair_verb_matches_what_actually_happens() {
        assert_eq!(Presence::Missing.repair_verb(), "install");
        assert_eq!(Presence::Broken.repair_verb(), "reinstall");
        assert_eq!(
            Presence::Incomplete {
                version: "0.36.1".into(),
                missing: vec!["proxy".into()],
            }
            .repair_verb(),
            "reinstall",
        );
    }

    #[test]
    fn prompt_mode_degrades_to_report_without_a_tty() {
        // Tests run without a TTY, so `Prompt` must never try to ask — an
        // unattended `whetstone doctor` has to stay non-blocking.
        assert_eq!(RepairMode::Prompt.effective(), RepairMode::Report);
        assert_eq!(RepairMode::Force.effective(), RepairMode::Force);
        assert_eq!(RepairMode::Report.effective(), RepairMode::Report);
    }

    #[test]
    fn missing_tool_is_only_reported_in_report_mode() {
        // `claude`/`rtk` may or may not exist on the machine running tests, so
        // drive the pure branch with a tool binary that cannot exist.
        let outcome = repair_one(Tool::Icm, RepairMode::Report, "all");
        match Tool::Icm.presence("all") {
            Presence::Present(ver) => {
                assert_eq!(outcome, ToolOutcome::Ok(ver));
            }
            other => {
                assert_eq!(outcome, ToolOutcome::Unrepaired(other));
            }
        }
    }

    #[test]
    fn from_binary_round_trips_every_tool() {
        for tool in [Tool::Headroom, Tool::Rtk, Tool::ClaudeCode, Tool::Icm] {
            assert_eq!(Tool::from_binary(tool.binary()), Some(tool));
        }
        assert_eq!(Tool::from_binary("git"), None);
    }

    #[test]
    fn tool_binaries_are_the_names_whetstone_shells_out_to() {
        assert_eq!(Tool::Headroom.binary(), "headroom");
        assert_eq!(Tool::Rtk.binary(), "rtk");
        assert_eq!(Tool::ClaudeCode.binary(), "claude");
        assert_eq!(Tool::Icm.binary(), "icm");
    }

    #[test]
    fn healthy_covers_ok_and_repaired_only() {
        let ok = ToolReport {
            tool: Tool::Rtk,
            outcome: ToolOutcome::Ok("0.42.3".into()),
        };
        let repaired = ToolReport {
            tool: Tool::Rtk,
            outcome: ToolOutcome::Repaired {
                from: Presence::Missing,
                version: "0.42.3".into(),
            },
        };
        let unrepaired = ToolReport {
            tool: Tool::Rtk,
            outcome: ToolOutcome::Unrepaired(Presence::Missing),
        };
        let failed = ToolReport {
            tool: Tool::Rtk,
            outcome: ToolOutcome::Failed {
                presence: Presence::Missing,
                error: "boom".into(),
            },
        };
        assert!(ok.is_healthy());
        assert!(repaired.is_healthy());
        assert!(!unrepaired.is_healthy());
        assert!(!failed.is_healthy());
    }

    #[test]
    fn prereq_list_covers_the_preflight_dependencies() {
        // `install-tools` must not silently know about fewer dependencies
        // than `whetstone setup`'s preflight checks.
        let names: Vec<&str> = PREREQS.iter().map(|p| p.name).collect();
        for expected in ["uv", "python3", "git", "curl"] {
            assert!(
                names.contains(&expected),
                "prereq list is missing {expected}",
            );
        }
    }

    #[test]
    fn only_uv_claims_an_unattended_installer() {
        for prereq in PREREQS {
            if prereq.installable {
                assert_eq!(prereq.name, "uv");
            } else {
                assert!(install_prereq(prereq).is_err());
            }
        }
    }
}
