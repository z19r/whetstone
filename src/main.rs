mod cli;
mod config;
mod db;
mod headroom;
mod hooks;
mod preflight;
mod release;
mod rtk;
mod setup;
mod shell;
mod ui;
mod uninstall;
mod update;
mod version;
mod wrapper;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

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
                println!("whetstone {}", version::current());
            }
            Command::Update { full } => {
                if let Err(e) = update::run(full) {
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
            Command::Db { action } => {
                if let Err(e) = db::dispatch(action) {
                    ui::fail(&e.to_string());
                }
            }
        },
    }
}
