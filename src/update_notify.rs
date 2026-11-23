//! Update prompts for whetstone and its managed tools.
//!
//! Two surfaces, both fed by the 12h version cache in `update.rs`:
//! - a full-screen modal before `headroom wrap claude` is exec'd, offering to
//!   install now, skip this launch, or skip these exact versions;
//! - a detached watcher (`whetstone update-watch <pid>`) that lives as long as
//!   the launching session (exec keeps the pid), refreshes the cache hourly,
//!   and sends a desktop notification the first time each new version appears.
//!   It never installs: replacing headroom/claude under a live session — or a
//!   proxy other sessions share — is not safe, so it only says "restart".

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use serde::{Deserialize, Serialize};

use crate::update::{self, OutdatedComponent};

const REFRESH_SECS: u64 = 60 * 60;
const POLL: Duration = Duration::from_secs(60);
/// Set on the re-exec after an in-modal update so a partial failure can't
/// loop straight back into the same modal.
const UPDATED_ENV: &str = "WHETSTONE_JUST_UPDATED";

/// Versions the user skipped and versions already announced, keyed
/// `name@version` so a newer release is offered again.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
struct UpdateState {
    #[serde(default)]
    dismissed: Vec<String>,
    #[serde(default)]
    notified: Vec<String>,
}

fn key(c: &OutdatedComponent) -> String {
    format!("{}@{}", c.name, c.latest)
}

fn state_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".cache/whetstone/update-state.json"))
}

fn load_state() -> UpdateState {
    state_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(state: &UpdateState) {
    let Some(path) = state_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(state) {
        let _ = std::fs::write(path, json);
    }
}

/// Outdated components the user hasn't skipped.
pub fn pending(outdated: Vec<OutdatedComponent>) -> Vec<OutdatedComponent> {
    let state = load_state();
    filter_unseen(outdated, &state.dismissed)
}

fn filter_unseen(
    outdated: Vec<OutdatedComponent>,
    seen: &[String],
) -> Vec<OutdatedComponent> {
    outdated
        .into_iter()
        .filter(|c| !seen.contains(&key(c)))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Choice {
    Update,
    Later,
    Skip,
}

/// Launch-time modal. Returns only if the launch should continue in this
/// process; after a whetstone self-update it re-execs the new binary.
pub fn prompt_at_launch(components: &[OutdatedComponent]) {
    if components.is_empty()
        || !crate::ui::is_interactive()
        || std::env::var_os(UPDATED_ENV).is_some()
    {
        return;
    }
    let mut terminal = ratatui::init();
    let choice = modal_loop(&mut terminal, components).unwrap_or(Choice::Later);
    ratatui::restore();

    match choice {
        Choice::Later => {}
        Choice::Skip => {
            let mut state = load_state();
            for c in components {
                let k = key(c);
                if !state.dismissed.contains(&k) {
                    state.dismissed.push(k);
                }
            }
            save_state(&state);
        }
        Choice::Update => {
            // Resolve before updating: self-update renames the running binary.
            let exe = std::env::current_exe().ok();
            if let Err(e) = update::run(false) {
                crate::ui::warn(&format!("update failed: {e:#}"));
            }
            if components.iter().any(|c| c.name == "whetstone") {
                if let Some(exe) = exe {
                    reexec(&exe);
                }
            }
        }
    }
}

#[cfg(unix)]
fn reexec(exe: &std::path::Path) {
    use std::os::unix::process::CommandExt;
    let err = Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env(UPDATED_ENV, "1")
        .exec();
    crate::ui::warn(&format!("could not restart whetstone: {err}"));
}

#[cfg(not(unix))]
fn reexec(_exe: &std::path::Path) {}

fn modal_loop(
    terminal: &mut DefaultTerminal,
    components: &[OutdatedComponent],
) -> Result<Choice> {
    loop {
        terminal.draw(|frame| draw(frame, components))?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(k) = event::read()? else {
            continue;
        };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        match k.code {
            KeyCode::Enter | KeyCode::Char('u') => return Ok(Choice::Update),
            KeyCode::Char('s') => return Ok(Choice::Skip),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l') => {
                return Ok(Choice::Later)
            }
            _ => {}
        }
    }
}

fn draw(frame: &mut Frame, components: &[OutdatedComponent]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            " Install now before Claude starts, or keep going on what you have.",
            Style::default().add_modifier(Modifier::DIM),
        )),
    ])
    .block(
        Block::default().borders(Borders::ALL).title(Span::styled(
            " ⬆  Updates available ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
    );
    frame.render_widget(header, chunks[0]);

    let lines: Vec<Line<'_>> = components
        .iter()
        .map(|c| {
            Line::from(Span::styled(
                format!("  {}: {} → {}", c.name, c.current, c.latest),
                Style::default().add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Outdated ")),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Enter/u update now   Esc/l later   s skip these versions",
            Style::default().fg(Color::DarkGray),
        )))
        .block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );
}

/// Start the background watcher for the session running as `pid`. Its own
/// process group keeps it out of the terminal's Ctrl-C.
pub fn spawn_watcher(pid: u32) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = Command::new(exe);
    cmd.args(["update-watch", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let _ = cmd.spawn();
}

fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Body of `whetstone update-watch <pid>`: exits when the session does.
pub fn run_watcher(pid: u32) {
    while alive(pid) {
        // Concurrent sessions share the cache, so its age dedupes fetches.
        if update::cache_age_secs().is_none_or(|age| age >= REFRESH_SECS) {
            update::refresh_cache();
        }
        let mut state = load_state();
        let mut seen = state.dismissed.clone();
        seen.extend(state.notified.iter().cloned());
        let fresh = filter_unseen(update::check_cached_upgrade(), &seen);
        if !fresh.is_empty() {
            notify(&fresh);
            state.notified.extend(fresh.iter().map(key));
            save_state(&state);
        }
        std::thread::sleep(POLL);
    }
}

fn notify(components: &[OutdatedComponent]) {
    let title = "whetstone: updates available";
    let body = format!(
        "{}\nRestart whetstone to install.",
        components
            .iter()
            .map(|c| format!("{} {} → {}", c.name, c.current, c.latest))
            .collect::<Vec<_>>()
            .join("\n")
    );
    // No notifier installed is fine: the launch modal still covers it.
    let _ = if cfg!(target_os = "macos") {
        let script = format!(
            "display notification \"{}\" with title \"{title}\"",
            body.replace('"', "'")
        );
        Command::new("osascript").args(["-e", &script]).status()
    } else {
        Command::new("notify-send")
            .args(["-a", "whetstone", title, &body])
            .status()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comp(name: &'static str, latest: &str) -> OutdatedComponent {
        OutdatedComponent {
            name,
            current: "1.0.0".into(),
            latest: latest.into(),
        }
    }

    #[test]
    fn skipped_version_is_hidden_but_newer_one_is_offered() {
        let seen = vec!["headroom@0.41.0".to_string()];
        let out = filter_unseen(
            vec![comp("headroom", "0.41.0"), comp("rtk", "2.0.0")],
            &seen,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "rtk");

        let out = filter_unseen(vec![comp("headroom", "0.42.0")], &seen);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn state_parses_partial_json() {
        let s: UpdateState =
            serde_json::from_str(r#"{"dismissed":["a@1"]}"#).unwrap();
        assert_eq!(s.dismissed, vec!["a@1"]);
        assert!(s.notified.is_empty());
    }

    #[test]
    fn current_pid_is_alive() {
        assert!(alive(std::process::id()));
    }
}
