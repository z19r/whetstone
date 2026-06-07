mod cli;
mod config;
mod dashboard;
mod db;
mod headroom;
mod hooks;
mod memory;
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
        Some(Command::Update { .. } | Command::Version | Command::Dashboard | Command::Stats)
    );
    if skip {
        return;
    }
    let outdated = update::check_cached_upgrade();
    ui::upgrade_banner(&outdated);
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
        },
    }
}
