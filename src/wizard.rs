use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    DefaultTerminal, Frame,
};
use std::io;
use std::time::Duration;

use crate::memory::MemoryProvider;
use crate::{headroom, preflight, rtk, setup, shell, ui, version};

const TOTAL_STEPS: usize = 7;

pub fn run(full: bool, headroom_extras: &str) -> Result<()> {
    welcome_screen()?;

    let assets = setup::resolve_assets_dir()?;
    ui::ok(&format!("assets at {}", assets.display()));

    step_progress(1, "Dependencies");
    preflight::check_all()?;

    step_progress(2, "Headroom");
    headroom::install(headroom_extras, full)?;

    step_progress(3, "RTK");
    rtk::install(full)?;

    step_progress(4, "Shell profile");
    shell::set_anthropic_base_url(setup::DEFAULT_PROXY)?;
    shell::ensure_path_contains_local_bin()?;

    step_progress(5, "Binary install");
    setup::self_install()?;

    step_progress(6, "Memory provider");
    let provider = setup::prompt_memory_provider(full)?;

    if provider != MemoryProvider::Skip {
        step_progress(7, "Integrations & manifest");
        setup::complete_setup(provider, &assets, full)?;
    } else {
        ui::info("skipped memory provider, skills, integrations, manifest");
    }

    completion_screen(provider)?;
    Ok(())
}

fn step_progress(step: usize, name: &str) {
    ui::section(&format!("Step {step}/{TOTAL_STEPS} — {name}"));
    render_gauge(step);
}

fn render_gauge(step: usize) {
    let ratio = (step.saturating_sub(1) as f64 / TOTAL_STEPS as f64).min(1.0);
    let label = format!(" {}/{TOTAL_STEPS} ", step.saturating_sub(1));

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .ratio(ratio)
        .label(Span::styled(
            label,
            Style::default().add_modifier(Modifier::BOLD),
        ));

    let backend = ratatui::backend::CrosstermBackend::new(io::stderr());
    if let Ok(mut terminal) = ratatui::Terminal::with_options(
        backend,
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(1),
        },
    ) {
        let _ = terminal.draw(|frame| {
            frame.render_widget(gauge, frame.area());
        });
    }
}

fn welcome_screen() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = welcome_loop(&mut terminal);
    ratatui::restore();
    result
}

fn welcome_loop(terminal: &mut DefaultTerminal) -> Result<()> {
    loop {
        terminal.draw(draw_welcome)?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') => return Ok(()),
                    KeyCode::Char('q') | KeyCode::Esc => {
                        anyhow::bail!("setup cancelled");
                    }
                    _ => {}
                }
            }
        }
    }
}

fn draw_welcome(frame: &mut Frame) {
    let area = centered_rect(52, 14, frame.area());

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Whetstone Setup",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  This wizard will install and configure:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("    1. ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled("Headroom", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" — context compression proxy"),
        ]),
        Line::from(vec![
            Span::styled("    2. ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled("RTK", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" — CLI output compression"),
        ]),
        Line::from(vec![
            Span::styled("    3. ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled("Memory", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" — persistent session context"),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "  Press Enter to begin, q to cancel",
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(""),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn completion_screen(provider: MemoryProvider) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = completion_loop(&mut terminal, provider);
    ratatui::restore();
    result
}

fn completion_loop(terminal: &mut DefaultTerminal, provider: MemoryProvider) -> Result<()> {
    loop {
        terminal.draw(|frame| draw_completion(frame, provider))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                return Ok(());
            }
        }
    }
}

fn draw_completion(frame: &mut Frame, provider: MemoryProvider) {
    let area = centered_rect(52, 16, frame.area());

    let headroom_ver = headroom::installed_version().unwrap_or_else(|| "—".into());
    let rtk_ver = rtk::installed_version().unwrap_or_else(|| "—".into());

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  \u{2713} ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Setup Complete",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Installed:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("    \u{25cf} ", Style::default().fg(Color::Green)),
            Span::raw(format!("Headroom {headroom_ver}")),
        ]),
        Line::from(vec![
            Span::styled("    \u{25cf} ", Style::default().fg(Color::Green)),
            Span::raw(format!("RTK {rtk_ver}")),
        ]),
    ];

    match provider {
        MemoryProvider::Icm => lines.push(Line::from(vec![
            Span::styled("    \u{25cf} ", Style::default().fg(Color::Green)),
            Span::raw("ICM (embedded SQLite)"),
        ])),
        MemoryProvider::Skip => lines.push(Line::from(vec![
            Span::styled(
                "    \u{25cb} ",
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::styled(
                "Memory provider skipped",
                Style::default().add_modifier(Modifier::DIM),
            ),
        ])),
    }

    lines.extend([
        Line::from(""),
        Line::from(Span::styled(
            format!("  whetstone v{}", version::current()),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Run: whetstone",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to exit",
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(""),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " whetstone ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(height),
            Constraint::Fill(1),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(width),
            Constraint::Fill(1),
        ])
        .split(vertical[1])[1]
}
