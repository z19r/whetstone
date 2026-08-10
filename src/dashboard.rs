use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    DefaultTerminal, Frame,
};
use std::time::{Duration, Instant};

use crate::{headroom, icm, rtk, update, version};

const POLL_MS: u64 = 250;
const REFRESH_SECS: u64 = 30;

struct ComponentInfo {
    name: &'static str,
    installed: Option<String>,
    latest: Option<String>,
}

impl ComponentInfo {
    fn status_line(&self) -> Line<'_> {
        match (&self.installed, &self.latest) {
            (Some(cur), Some(lat)) if version::is_older(cur, lat) => {
                Line::from(vec![
                    Span::styled("  ● ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        self.name,
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!("  {cur} → {lat}")),
                ])
            }
            (Some(cur), _) => Line::from(vec![
                Span::styled("  ● ", Style::default().fg(Color::Green)),
                Span::styled(
                    self.name,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  {cur}")),
            ]),
            (None, _) => Line::from(vec![
                Span::styled(
                    "  ○ ",
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(
                    self.name,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  not installed",
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ]),
        }
    }
}

struct DashboardState {
    components: Vec<ComponentInfo>,
    tab: usize,
    last_refresh: Instant,
    status_msg: String,
}

impl DashboardState {
    fn new() -> Self {
        let mut state = Self {
            components: Vec::new(),
            tab: 0,
            last_refresh: Instant::now()
                - Duration::from_secs(REFRESH_SECS + 1),
            status_msg: "Loading...".into(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        self.status_msg = "Refreshing...".into();
        let outdated = update::check_cached_upgrade();
        let is_outdated = |name: &str| outdated.iter().find(|c| c.name == name);

        let whetstone_latest = is_outdated("whetstone")
            .map(|c| c.latest.clone())
            .or_else(|| Some(version::current().to_string()));
        let rtk_latest = is_outdated("rtk")
            .map(|c| c.latest.clone())
            .or_else(rtk::installed_version);
        let headroom_latest = is_outdated("headroom")
            .map(|c| c.latest.clone())
            .or_else(headroom::installed_version);
        let icm_latest = is_outdated("memory (ICM)")
            .map(|c| c.latest.clone())
            .or_else(icm::installed_version);

        self.components = vec![
            ComponentInfo {
                name: "whetstone",
                installed: Some(version::current().to_string()),
                latest: whetstone_latest,
            },
            ComponentInfo {
                name: "headroom",
                installed: headroom::installed_version(),
                latest: headroom_latest,
            },
            ComponentInfo {
                name: "rtk",
                installed: rtk::installed_version(),
                latest: rtk_latest,
            },
            ComponentInfo {
                name: "memory (ICM)",
                installed: icm::installed_version(),
                latest: icm_latest,
            },
        ];
        self.last_refresh = Instant::now();
        self.status_msg = "Ready".into();
    }

    fn should_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= Duration::from_secs(REFRESH_SECS)
    }
}

fn draw(frame: &mut Frame, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, chunks[0], state);
    draw_body(frame, chunks[1], state);
    draw_footer(frame, chunks[2], state);
}

fn draw_header(frame: &mut Frame, area: Rect, state: &DashboardState) {
    let titles = vec!["Components", "Actions"];
    let tabs = Tabs::new(titles)
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                format!(" whetstone v{} ", version::current()),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .select(state.tab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, area);
}

fn draw_body(frame: &mut Frame, area: Rect, state: &DashboardState) {
    match state.tab {
        0 => draw_components(frame, area, state),
        1 => draw_actions(frame, area),
        _ => {}
    }
}

fn draw_components(frame: &mut Frame, area: Rect, state: &DashboardState) {
    let mut lines: Vec<Line<'_>> = vec![Line::from("")];
    for c in &state.components {
        lines.push(c.status_line());
    }
    lines.push(Line::from(""));

    let elapsed = state.last_refresh.elapsed().as_secs();
    lines.push(Line::from(Span::styled(
        format!("  Last checked {elapsed}s ago"),
        Style::default().add_modifier(Modifier::DIM),
    )));

    let block = Block::default().borders(Borders::ALL).title(" Components ");
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_actions(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  u",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  update all components"),
        ]),
        Line::from(vec![
            Span::styled(
                "  v",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  refresh version check"),
        ]),
        Line::from(vec![
            Span::styled(
                "  r",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  refresh dashboard"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press the key to execute an action",
            Style::default().add_modifier(Modifier::DIM),
        )),
    ];

    let block = Block::default().borders(Borders::ALL).title(" Actions ");
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, state: &DashboardState) {
    let help = Line::from(vec![
        Span::styled(
            " q",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":quit  "),
        Span::styled(
            "←→",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":tab  "),
        Span::styled(
            "r",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":refresh  "),
        Span::raw(format!("  {}", state.status_msg)),
    ]);

    let block = Block::default().borders(Borders::ALL);
    let paragraph = Paragraph::new(help).block(block);
    frame.render_widget(paragraph, area);
}

pub fn run() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal);
    ratatui::restore();
    result
}

fn run_loop(terminal: &mut DefaultTerminal) -> Result<()> {
    let mut state = DashboardState::new();

    loop {
        terminal.draw(|frame| draw(frame, &state))?;

        if event::poll(Duration::from_millis(POLL_MS))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Left | KeyCode::Char('h') => {
                        state.tab = state.tab.saturating_sub(1);
                    }
                    KeyCode::Right | KeyCode::Char('l') if state.tab < 1 => {
                        state.tab += 1;
                    }
                    KeyCode::Char('r') | KeyCode::Char('v') => {
                        state.refresh();
                    }
                    KeyCode::Char('u') => {
                        ratatui::restore();
                        if let Err(e) = update::run(false) {
                            eprintln!("Update failed: {e:#}");
                        }
                        eprintln!("\nPress Enter to return to dashboard...");
                        let _ = std::io::stdin().read_line(&mut String::new());
                        *terminal = ratatui::init();
                        state.refresh();
                    }
                    _ => {}
                }
            }
        }

        if state.should_refresh() {
            state.refresh();
        }
    }
}
