mod changelog;
mod cli;
mod config;
mod dashboard;
mod db;
mod doctor;
mod headroom;
mod integrations;
mod memory;
mod migrate;
mod preflight;
mod release;
mod rtk;
mod setup;
mod shell;
mod stats;
mod ui;
mod uninstall;
mod update;
mod version;
mod wizard;
mod wrapper;

use clap::Parser;
use cli::{Cli, Command};

fn show_upgrade_banner(cmd: &Option<Command>) {
    let skip = matches!(
        cmd,
        Some(
            Command::Update { .. }
                | Command::Version
                | Command::Dashboard
                | Command::Stats
                | Command::Migrate { .. }
        )
    );
    if skip {
        return;
    }
    let outdated = update::check_cached_upgrade();
    let v2_project = migrate::detect()
        .map(|d| d.needs_migration())
        .unwrap_or(false);
    ui::upgrade_banner(&outdated, v2_project);
}

fn main() {
    let cli = Cli::parse();

    show_upgrade_banner(&cli.command);

    match cli.command {
        None => {
            wrapper::wrap_claude(&[]);
        }
        Some(cmd) => match cmd {
            Command::Setup {
                full,
                headroom_extras,
            } => {
                if let Err(e) = setup::run(full, &headroom_extras) {
                    ui::fail(&format!("{e:#}"));
                }
            }
            Command::Uninstall => {
                if let Err(e) = uninstall::run() {
                    ui::fail(&format!("{e:#}"));
                }
            }
            Command::Claude { args } | Command::Code { args } => {
                wrapper::wrap_claude(&args);
            }
            Command::Proxy { args } => {
                wrapper::wrap_proxy(&args);
            }
            Command::Rtk { args } => {
                wrapper::wrap_rtk(&args);
            }
            Command::Version => {
                let outdated = update::check_cached_upgrade();
                let is_outdated = |name: &str| outdated.iter().any(|c| c.name == name);

                let entries = vec![
                    ui::VersionEntry {
                        name: "whetstone",
                        version: Some(version::current().to_string()),
                        outdated: is_outdated("whetstone"),
                    },
                    ui::VersionEntry {
                        name: "headroom",
                        version: headroom::installed_version(),
                        outdated: is_outdated("headroom"),
                    },
                    ui::VersionEntry {
                        name: "rtk",
                        version: rtk::installed_version(),
                        outdated: is_outdated("rtk"),
                    },
                ];
                ui::version_report(&entries);
            }
            Command::Update { full } => {
                if let Err(e) = update::run(full) {
                    ui::fail(&format!("{e:#}"));
                }
            }
            Command::Dashboard => {
                if let Err(e) = dashboard::run() {
                    ui::fail(&format!("{e:#}"));
                }
            }
            Command::Doctor => {
                if let Err(e) = doctor::run() {
                    ui::fail(&format!("{e:#}"));
                }
            }
            Command::Release { action } => {
                if let Err(e) = release::run(&action) {
                    ui::fail(&format!("{e:#}"));
                }
            }
            Command::ReleasePublish { action } => {
                if let Err(e) = release::run_publish(&action) {
                    ui::fail(&format!("{e:#}"));
                }
            }
            Command::ChangelogSync {
                input,
                output,
                limit,
            } => {
                if let Err(e) = run_changelog_sync(input, output, limit) {
                    ui::fail(&format!("{e:#}"));
                }
            }
            Command::Stats => {
                if let Err(e) = stats::run() {
                    ui::fail(&format!("{e:#}"));
                }
            }
            Command::Db { action } => {
                if let Err(e) = db::dispatch(action) {
                    ui::fail(&e.to_string());
                }
            }
            Command::Migrate {
                dry_run,
                yes,
                rollback,
            } => {
                let result = match rollback {
                    Some(id) => migrate::rollback(&id),
                    None => migrate::run(migrate::MigrateOptions { dry_run, yes }),
                };
                if let Err(e) = result {
                    ui::fail(&format!("{e:#}"));
                }
            }
        },
    }
}

fn run_changelog_sync(
    input: Option<String>,
    output: Option<String>,
    limit: usize,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let input_path = input
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| cwd.join("CHANGELOG.md"));
    let output_path = output
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| cwd.join("site/src/changelog.js"));

    let entries = changelog::parse_file(&input_path, limit)?;
    let js = changelog::render_js(&entries);

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_path, js)?;

    ui::ok(&format!(
        "changelog: {} -> {} ({} releases)",
        input_path.display(),
        output_path.display(),
        entries.len()
    ));
    Ok(())
}
