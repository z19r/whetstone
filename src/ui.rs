use crossterm::style::Stylize;
use crossterm::{cursor, event, terminal};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Gauge, Widget};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal, TerminalOptions, Viewport,
};
use std::io::{self, IsTerminal, Stderr, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

// Shared inline viewport for the setup wizard. Only one is active at a time.
// While active, every status helper (`info/ok/warn/fail/section/...`) routes
// through `Terminal::insert_before` so the line is pushed into scrollback ABOVE
// the inline viewport. That keeps the gauge stable at the bottom of the screen
// and prevents the line/gauge overdraw that produced mangled output before.
type WizardTerm = Terminal<ratatui::backend::CrosstermBackend<Stderr>>;

#[derive(Clone)]
struct GaugeState {
    step: usize,
    total: usize,
    name: String,
}

struct Wizard {
    term: Option<WizardTerm>,
    last: Option<GaugeState>,
}

impl Wizard {
    const fn empty() -> Self {
        Self {
            term: None,
            last: None,
        }
    }
}

fn wizard_slot() -> &'static Mutex<Wizard> {
    static SLOT: OnceLock<Mutex<Wizard>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(Wizard::empty()))
}

fn build_wizard_terminal() -> Option<WizardTerm> {
    let backend = ratatui::backend::CrosstermBackend::new(io::stderr());
    let mut term = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(2),
        },
    )
    .ok()?;
    // Pre-draw an empty gauge area so the viewport is reserved on screen.
    let _ = term.draw(|frame| {
        frame.render_widget(Paragraph::new(""), frame.area());
    });
    Some(term)
}

fn draw_gauge(term: &mut WizardTerm, state: &GaugeState) {
    let ratio = (state.step as f64 / state.total as f64).clamp(0.0, 1.0);
    let label = format!(
        "{step}/{total}  {name}",
        step = state.step,
        total = state.total,
        name = state.name
    );
    let title_line = Line::from(vec![
        Span::styled(
            format!(
                "Step {step}/{total}",
                step = state.step,
                total = state.total
            ),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" — {}", state.name)),
    ]);
    let ratio_copy = ratio;
    let label_copy = label;
    let _ = term.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(frame.area());
        let title = Paragraph::new(title_line.clone());
        frame.render_widget(title, chunks[0]);
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
            .ratio(ratio_copy)
            .label(Span::styled(
                format!(" {label_copy} "),
                Style::default().add_modifier(Modifier::BOLD),
            ));
        frame.render_widget(gauge, chunks[1]);
    });
}

/// Reserve the bottom 2 rows of the terminal for the wizard gauge. After this,
/// every `info/ok/warn/...` line is inserted ABOVE the gauge via ratatui's
/// `insert_before`, and the gauge is updated with `wizard_step`.
pub fn enter_wizard() {
    if !is_interactive() {
        return;
    }
    if let Some(term) = build_wizard_terminal() {
        if let Ok(mut slot) = wizard_slot().lock() {
            slot.term = Some(term);
        }
    }
}

/// Tear down the wizard viewport. Subsequent status helpers fall back to
/// plain `eprintln!`.
pub fn exit_wizard() {
    if let Ok(mut slot) = wizard_slot().lock() {
        if let Some(mut term) = slot.term.take() {
            // Final empty draw so the viewport leaves a clean trailing row.
            let _ = term.draw(|frame| {
                frame.render_widget(Paragraph::new(""), frame.area());
            });
            drop(term);
        }
        slot.last = None;
    }
}

/// Redraw the inline gauge for the given step. `total` is the denominator
/// shown to the user (e.g. 7). Cheap to call repeatedly.
pub fn wizard_step(step: usize, total: usize, name: &str) {
    let state = GaugeState {
        step,
        total,
        name: name.to_string(),
    };
    if let Ok(mut slot) = wizard_slot().lock() {
        slot.last = Some(state.clone());
        if let Some(term) = slot.term.as_mut() {
            draw_gauge(term, &state);
        }
    }
}

/// Pause the wizard viewport so an interactive prompt can write raw stderr
/// without colliding with the inline region. Returns a guard that re-installs
/// the viewport and redraws the last gauge state when dropped.
#[must_use = "WizardPause restores the wizard on drop; bind to a variable"]
pub struct WizardPause {
    was_active: bool,
    last: Option<GaugeState>,
}

impl Drop for WizardPause {
    fn drop(&mut self) {
        if !self.was_active {
            return;
        }
        if let Some(term) = build_wizard_terminal() {
            if let Ok(mut slot) = wizard_slot().lock() {
                slot.term = Some(term);
                if let Some(state) = self.last.clone() {
                    slot.last = Some(state.clone());
                    if let Some(term) = slot.term.as_mut() {
                        draw_gauge(term, &state);
                    }
                }
            }
        }
    }
}

pub fn pause_wizard() -> WizardPause {
    let mut was_active = false;
    let mut last = None;
    if let Ok(mut slot) = wizard_slot().lock() {
        if let Some(mut term) = slot.term.take() {
            was_active = true;
            // Empty draw + drop so the viewport releases its rows. After this,
            // the cursor lands on a fresh line below the area the viewport had
            // reserved, which is exactly where an interactive prompt wants to
            // start writing.
            let _ = term.draw(|frame| {
                frame.render_widget(Paragraph::new(""), frame.area());
            });
            drop(term);
        }
        last = slot.last.clone();
    }
    WizardPause { was_active, last }
}

/// Emit a styled multi-line message. If the wizard viewport is active, the
/// lines are inserted above the gauge via `insert_before`; otherwise they are
/// printed straight to stderr (with the same colors via crossterm Stylize).
fn emit_lines(lines: Vec<Line<'static>>, plain: &str) {
    if let Ok(mut slot) = wizard_slot().lock() {
        if let Some(term) = slot.term.as_mut() {
            let height = lines.len() as u16;
            let _ = term.insert_before(height, |buf| {
                let area = buf.area;
                Paragraph::new(lines).render(area, buf);
            });
            return;
        }
    }
    eprintln!("{plain}");
}

fn status_line(tag: &'static str, color: Color, leading: &'static str, msg: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw(leading),
        Span::styled(tag, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::raw(format!(" {msg}")),
    ])
}

pub fn info(msg: &str) {
    let plain = format!("{} {msg}", "[INFO]".blue().bold());
    emit_lines(vec![status_line("[INFO]", Color::Blue, "", msg)], &plain);
}

pub fn ok(msg: &str) {
    let plain = format!("  {} {msg}", "[OK]".green().bold());
    emit_lines(vec![status_line("[OK]", Color::Green, "  ", msg)], &plain);
}

pub fn warn(msg: &str) {
    let plain = format!("{} {msg}", "[WARN]".yellow().bold());
    emit_lines(vec![status_line("[WARN]", Color::Yellow, "", msg)], &plain);
}

pub fn fail(msg: &str) -> ! {
    let plain = format!("{} {msg}", "[FAIL]".red().bold());
    emit_lines(vec![status_line("[FAIL]", Color::Red, "", msg)], &plain);
    // Make sure the wizard tears down cleanly before exit, otherwise the
    // inline viewport leaves the terminal in raw-ish state.
    exit_wizard();
    std::process::exit(1);
}

pub fn is_interactive() -> bool {
    io::stdin().is_terminal()
}

pub fn confirm(prompt: &str, default: bool) -> bool {
    if !is_interactive() {
        return default;
    }

    let _pause = pause_wizard();

    let hint = if default { "[Y/n]" } else { "[y/N]" };
    eprint!("{} {} ", prompt, hint.dim());
    let _ = io::stderr().flush();

    terminal::enable_raw_mode().ok();
    let result = loop {
        if let Ok(event::Event::Key(key)) = event::read() {
            match key.code {
                event::KeyCode::Char('y' | 'Y') => break true,
                event::KeyCode::Char('n' | 'N') => break false,
                event::KeyCode::Enter => break default,
                event::KeyCode::Esc => break false,
                _ => {}
            }
        }
    };
    terminal::disable_raw_mode().ok();
    eprintln!("{}", if result { "yes" } else { "no" });
    result
}

pub fn select<T: std::fmt::Display>(prompt: &str, items: &[T], default: usize) -> usize {
    if !is_interactive() || items.is_empty() {
        return default;
    }

    let _pause = pause_wizard();

    let mut selected = default;

    eprintln!("{prompt}");

    terminal::enable_raw_mode().ok();
    loop {
        let mut stderr = io::stderr();
        for (i, item) in items.iter().enumerate() {
            if i == selected {
                let _ = write!(
                    stderr,
                    "\r  {} {}\r\n",
                    "›".cyan().bold(),
                    item.to_string().bold()
                );
            } else {
                let _ = write!(stderr, "\r    {}\r\n", item);
            }
        }
        let _ = stderr.flush();

        if let Ok(event::Event::Key(key)) = event::read() {
            match key.code {
                event::KeyCode::Up | event::KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                }
                event::KeyCode::Down | event::KeyCode::Char('j') if selected + 1 < items.len() => {
                    selected += 1;
                }
                event::KeyCode::Enter => break,
                event::KeyCode::Esc => {
                    selected = default;
                    break;
                }
                _ => {}
            }
        }

        // Move cursor back up to redraw
        let _ = crossterm::execute!(stderr, cursor::MoveUp(items.len() as u16));
    }
    terminal::disable_raw_mode().ok();

    // Clear the selection display
    let mut stderr = io::stderr();
    for _ in items {
        let _ = write!(stderr, "\r{}\r\n", " ".repeat(60));
    }
    let _ = crossterm::execute!(stderr, cursor::MoveUp(items.len() as u16));
    eprintln!("{}: {}", prompt, items[selected].to_string().cyan().bold());

    selected
}

pub fn upgrade_banner(components: &[crate::update::OutdatedComponent], v2_project: bool) {
    if components.is_empty() && !v2_project {
        return;
    }

    let title = "⬆  UPDATES AVAILABLE";

    let mut content_lines: Vec<Line<'_>> = Vec::new();
    for c in components {
        content_lines.push(Line::from(vec![Span::styled(
            format!("{}: {} → {}", c.name, c.current, c.latest),
            Style::default().add_modifier(Modifier::BOLD),
        )]));
    }
    if v2_project {
        if !components.is_empty() {
            content_lines.push(Line::from(""));
        }
        content_lines.push(Line::from(Span::styled(
            "⚠ v2 project — run `whetstone migrate` for ICM + updated skills",
            Style::default().fg(Color::Yellow),
        )));
    }
    content_lines.push(Line::from(""));
    let action = if v2_project && components.is_empty() {
        "Run: whetstone migrate"
    } else if v2_project {
        "Run: whetstone update  (or  whetstone migrate)"
    } else {
        "Run: whetstone update"
    };
    content_lines.push(Line::from(Span::styled(
        action,
        Style::default().add_modifier(Modifier::DIM),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(content_lines).block(block);

    let mut line_count = components.len();
    if v2_project {
        line_count += 2;
    }
    let height = (line_count + 5) as u16;
    eprintln!();
    let backend = ratatui::backend::CrosstermBackend::new(io::stderr());
    if let Ok(mut terminal) = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(height),
        },
    ) {
        let _ = terminal.draw(|frame| {
            frame.render_widget(paragraph, frame.area());
        });
    }
    eprintln!();
}

pub fn section(title: &str) {
    let rule = "─".repeat(40);
    let plain = format!("\n  {title} {rule}\n");
    let title_owned = title.to_string();
    emit_lines(
        vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(title_owned, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(rule, Style::default().add_modifier(Modifier::DIM)),
            ]),
            Line::from(""),
        ],
        &plain,
    );
}

#[derive(Debug)]
pub enum ComponentStatus {
    UpToDate(String),
    Updated(String, String),
    NotInstalled,
    Failed(String),
}

pub fn component_line(name: &str, status: &ComponentStatus) {
    let label = format!("{:.<16}", format!("{name} "));
    let label_owned = label.clone();

    let line = match status {
        ComponentStatus::UpToDate(ver) => Line::from(vec![
            Span::raw("  "),
            Span::styled("●", Style::default().fg(Color::Green)),
            Span::raw(" "),
            Span::styled(label_owned, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(
                format!("{ver} (up to date)"),
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]),
        ComponentStatus::Updated(from, to) => Line::from(vec![
            Span::raw("  "),
            Span::styled("●", Style::default().fg(Color::Green)),
            Span::raw(" "),
            Span::styled(label_owned, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(from.clone(), Style::default().add_modifier(Modifier::DIM)),
            Span::raw(" → "),
            Span::styled(
                to.clone(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        ComponentStatus::NotInstalled => Line::from(vec![
            Span::raw("  "),
            Span::styled("○", Style::default().add_modifier(Modifier::DIM)),
            Span::raw(" "),
            Span::styled(label_owned, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(
                "not installed",
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]),
        ComponentStatus::Failed(reason) => Line::from(vec![
            Span::raw("  "),
            Span::styled("✗", Style::default().fg(Color::Red)),
            Span::raw(" "),
            Span::styled(label_owned, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(reason.clone(), Style::default().fg(Color::Red)),
        ]),
    };

    let plain = match status {
        ComponentStatus::UpToDate(ver) => format!("  ● {label} {ver} (up to date)"),
        ComponentStatus::Updated(from, to) => format!("  ● {label} {from} → {to}"),
        ComponentStatus::NotInstalled => format!("  ○ {label} not installed"),
        ComponentStatus::Failed(reason) => format!("  ✗ {label} {reason}"),
    };

    emit_lines(vec![line], &plain);
}

pub struct VersionEntry {
    pub name: &'static str,
    pub version: Option<String>,
    pub outdated: bool,
}

pub fn version_report(entries: &[VersionEntry]) {
    let parts: Vec<String> = entries
        .iter()
        .map(|e| {
            let indicator = if e.outdated { " ⬆" } else { "" };
            match &e.version {
                Some(v) => format!(
                    "{} {}{}",
                    e.name.bold(),
                    v.clone().cyan(),
                    indicator.yellow().bold()
                ),
                None => format!("{} {}", e.name.bold(), "—".dim()),
            }
        })
        .collect();
    println!("{}", parts.join("  "));
}

pub fn summary_ok(msg: &str) {
    let msg_owned = msg.to_string();
    let plain = format!("\n  ✓ {msg}\n");
    emit_lines(
        vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "✓",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(msg_owned, Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
        ],
        &plain,
    );
}

pub fn summary_info(msg: &str) {
    let msg_owned = msg.to_string();
    let plain = format!("\n  ℹ {msg}\n");
    emit_lines(
        vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "ℹ",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(msg_owned, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
        ],
        &plain,
    );
}

const BRAILLE: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct Spinner {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    pub fn finish_and_clear(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let mut stderr = io::stderr();
        let _ = write!(stderr, "\r{}\r", " ".repeat(80));
        let _ = stderr.flush();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if !self.stop.load(Ordering::Relaxed) {
            self.finish_and_clear();
        }
    }
}

pub fn spinner(msg: &str) -> Spinner {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let msg = msg.to_string();

    if !is_interactive() {
        eprintln!("  {msg}");
        return Spinner { stop, handle: None };
    }

    let handle = std::thread::spawn(move || {
        let mut i = 0;
        let mut stderr = io::stderr();
        while !stop_clone.load(Ordering::Relaxed) {
            let frame = BRAILLE[i % BRAILLE.len()];
            let _ = write!(stderr, "\r  {} {}", frame.cyan(), msg);
            let _ = stderr.flush();
            i += 1;
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
    });

    Spinner {
        stop,
        handle: Some(handle),
    }
}
