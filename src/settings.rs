use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    DefaultTerminal, Frame,
};
use std::time::Duration;

use crate::config::{GlobalSettings, ProjectSettings, WhetstoneManifest};
use crate::memory::MemoryProvider;
use crate::ui;

const KNOWN_MODELS: &[&str] = &[
    "claude-opus-4-6",
    "claude-sonnet-4-6",
    "claude-haiku-4-5-20251001",
    "claude-fable-5",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Off,
    Global,
    Project,
}

impl Scope {
    fn cycle(self) -> Self {
        match self {
            Self::Off => Self::Global,
            Self::Global => Self::Project,
            Self::Project => Self::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingId {
    HeadroomTelemetry,
    ApiModel,
}

const SETTINGS: &[SettingId] = &[SettingId::HeadroomTelemetry, SettingId::ApiModel];

struct SettingsState {
    global: GlobalSettings,
    project: ProjectSettings,
    original_global: GlobalSettings,
    original_project: ProjectSettings,
    selected: usize,
}

impl SettingsState {
    fn scope(&self, id: SettingId) -> Scope {
        match id {
            SettingId::HeadroomTelemetry => match self.project.headroom_telemetry {
                Some(true) => Scope::Project,
                Some(false) => Scope::Off,
                None if self.global.headroom_telemetry => Scope::Global,
                None => Scope::Off,
            },
            SettingId::ApiModel => {
                if self.project.api_model.is_some() {
                    Scope::Project
                } else if self.global.api_model.is_some() {
                    Scope::Global
                } else {
                    Scope::Off
                }
            }
        }
    }

    fn cycle_scope(&mut self, id: SettingId) {
        let next = self.scope(id).cycle();
        match id {
            SettingId::HeadroomTelemetry => match next {
                Scope::Off => {
                    self.global.headroom_telemetry = false;
                    self.project.headroom_telemetry = None;
                }
                Scope::Global => {
                    self.global.headroom_telemetry = true;
                    self.project.headroom_telemetry = None;
                }
                Scope::Project => {
                    self.project.headroom_telemetry = Some(true);
                }
            },
            SettingId::ApiModel => match next {
                Scope::Off => {
                    self.global.api_model = None;
                    self.project.api_model = None;
                }
                Scope::Global => {
                    if self.global.api_model.is_none() {
                        self.global.api_model = Some(KNOWN_MODELS[0].to_string());
                    }
                    self.project.api_model = None;
                }
                Scope::Project => {
                    let base = self
                        .global
                        .api_model
                        .clone()
                        .unwrap_or_else(|| KNOWN_MODELS[0].to_string());
                    self.project.api_model = Some(base);
                }
            },
        }
    }

    fn cycle_model_value(&mut self) {
        let id = SETTINGS[self.selected];
        if id != SettingId::ApiModel {
            return;
        }
        let scope = self.scope(id);
        let target = match scope {
            Scope::Global => &mut self.global.api_model,
            Scope::Project => &mut self.project.api_model,
            Scope::Off => return,
        };
        let current = target.as_deref().unwrap_or(KNOWN_MODELS[0]);
        let idx = KNOWN_MODELS
            .iter()
            .position(|m| *m == current)
            .map(|i| (i + 1) % KNOWN_MODELS.len())
            .unwrap_or(0);
        *target = Some(KNOWN_MODELS[idx].to_string());
    }

    fn current_model_display(&self, id: SettingId) -> Option<&str> {
        if id != SettingId::ApiModel {
            return None;
        }
        match self.scope(id) {
            Scope::Off => None,
            Scope::Global => self.global.api_model.as_deref(),
            Scope::Project => self.project.api_model.as_deref(),
        }
    }

    fn dirty(&self) -> bool {
        self.global != self.original_global || self.project != self.original_project
    }
}

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let manifest_path = WhetstoneManifest::path_for(&cwd);

    let (manifest, had_manifest) = match WhetstoneManifest::load(&manifest_path)? {
        Some(m) => (Some(m), true),
        None => (None, false),
    };

    let global = GlobalSettings::load()?;
    let project = manifest
        .as_ref()
        .map(|m| m.settings.clone())
        .unwrap_or_default();

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, global.clone(), project.clone());
    ratatui::restore();

    match result {
        Ok(Some(saved)) => {
            let global_changed = saved.global != global;
            let project_changed = saved.project != project;

            if global_changed {
                saved.global.save()?;
            }
            if project_changed {
                save_project_settings(&manifest_path, manifest, had_manifest, saved.project)?;
            }

            if global_changed || project_changed {
                ui::ok("settings saved");
            } else {
                ui::info("no changes");
            }
        }
        Ok(None) => {
            ui::info("no changes");
        }
        Err(e) => return Err(e),
    }

    Ok(())
}

fn save_project_settings(
    manifest_path: &std::path::Path,
    existing: Option<WhetstoneManifest>,
    had_manifest: bool,
    project: ProjectSettings,
) -> Result<()> {
    let mut manifest = if had_manifest {
        existing.unwrap()
    } else {
        WhetstoneManifest::new(MemoryProvider::Skip, crate::config::ToolVersions::default())
    };
    manifest.settings = project;
    manifest.updated_at = chrono::Utc::now();

    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    manifest.save(manifest_path)
}

struct SavedState {
    global: GlobalSettings,
    project: ProjectSettings,
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    global: GlobalSettings,
    project: ProjectSettings,
) -> Result<Option<SavedState>> {
    let mut state = SettingsState {
        original_global: global.clone(),
        original_project: project.clone(),
        global,
        project,
        selected: 0,
    };

    loop {
        terminal.draw(|frame| draw(frame, &state))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if state.dirty() {
                            return Ok(Some(SavedState {
                                global: state.global,
                                project: state.project,
                            }));
                        }
                        return Ok(None);
                    }
                    KeyCode::Char(' ') => {
                        state.cycle_scope(SETTINGS[state.selected]);
                    }
                    KeyCode::Tab | KeyCode::Enter => {
                        state.cycle_model_value();
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.selected = state.selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') if state.selected + 1 < SETTINGS.len() => {
                        state.selected += 1;
                    }
                    KeyCode::Char('s') => {
                        return Ok(Some(SavedState {
                            global: state.global,
                            project: state.project,
                        }));
                    }
                    _ => {}
                }
            }
        }
    }
}

fn draw(frame: &mut Frame, state: &SettingsState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, chunks[0], state.dirty());
    draw_entries(frame, chunks[1], state);
    draw_footer(frame, chunks[2]);
}

fn draw_header(frame: &mut Frame, area: Rect, dirty: bool) {
    let title = if dirty {
        " Settings (modified) "
    } else {
        " Settings "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(Line::from(Span::styled(
        " whetstone configuration",
        Style::default().add_modifier(Modifier::DIM),
    )))
    .block(block);
    frame.render_widget(paragraph, area);
}

fn scope_spans(scope: Scope) -> Vec<Span<'static>> {
    let dim = Style::default().fg(Color::DarkGray);
    let active = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);

    match scope {
        Scope::Off => vec![
            Span::styled("off", active),
            Span::styled(" | ", dim),
            Span::styled("g", dim),
            Span::styled(" | ", dim),
            Span::styled("p", dim),
        ],
        Scope::Global => vec![
            Span::styled("off", dim),
            Span::styled(" | ", dim),
            Span::styled("g", active),
            Span::styled(" | ", dim),
            Span::styled("p", dim),
        ],
        Scope::Project => vec![
            Span::styled("off", dim),
            Span::styled(" | ", dim),
            Span::styled("g", dim),
            Span::styled(" | ", dim),
            Span::styled("p", active),
        ],
    }
}

fn draw_entries(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let mut lines: Vec<Line<'_>> = vec![Line::from("")];

    for (i, &id) in SETTINGS.iter().enumerate() {
        let is_selected = i == state.selected;
        let cursor = if is_selected { " › " } else { "   " };
        let label_style = if is_selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let (label, desc) = match id {
            SettingId::HeadroomTelemetry => (
                "Headroom Telemetry",
                "Send anonymous usage data to Headroom",
            ),
            SettingId::ApiModel => ("API Model", "Model used for Claude Code sessions"),
        };

        let scope = state.scope(id);

        let mut row = vec![Span::styled(
            cursor,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];
        row.extend(scope_spans(scope));
        row.push(Span::raw("  "));
        row.push(Span::styled(label.to_string(), label_style));

        if let Some(model) = state.current_model_display(id) {
            row.push(Span::styled(
                format!("  {model}"),
                Style::default().fg(Color::Yellow),
            ));
        }

        lines.push(Line::from(row));

        let desc_line = if id == SettingId::ApiModel && scope != Scope::Off {
            format!("{desc}  (tab to cycle model)")
        } else {
            desc.to_string()
        };

        lines.push(Line::from(vec![
            Span::raw("                  "),
            Span::styled(desc_line, Style::default().add_modifier(Modifier::DIM)),
        ]));

        lines.push(Line::from(""));
    }

    let block = Block::default().borders(Borders::ALL);
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled(
            " space",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":scope "),
        Span::styled(
            "tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":value "),
        Span::styled(
            "s",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":save "),
        Span::styled(
            "q",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":quit "),
        Span::styled(
            "↑↓",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":navigate"),
    ]);

    let block = Block::default().borders(Borders::ALL);
    let paragraph = Paragraph::new(help).block(block);
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_state() -> SettingsState {
        SettingsState {
            global: GlobalSettings::default(),
            project: ProjectSettings::default(),
            original_global: GlobalSettings::default(),
            original_project: ProjectSettings::default(),
            selected: 0,
        }
    }

    #[test]
    fn scope_cycle_order() {
        assert_eq!(Scope::Off.cycle(), Scope::Global);
        assert_eq!(Scope::Global.cycle(), Scope::Project);
        assert_eq!(Scope::Project.cycle(), Scope::Off);
    }

    #[test]
    fn telemetry_scope_from_project_override() {
        let mut s = default_state();
        s.project.headroom_telemetry = Some(true);
        assert_eq!(s.scope(SettingId::HeadroomTelemetry), Scope::Project);

        s.project.headroom_telemetry = Some(false);
        assert_eq!(s.scope(SettingId::HeadroomTelemetry), Scope::Off);
    }

    #[test]
    fn telemetry_scope_from_global() {
        let mut s = default_state();
        s.global.headroom_telemetry = true;
        assert_eq!(s.scope(SettingId::HeadroomTelemetry), Scope::Global);
    }

    #[test]
    fn telemetry_cycle_through_all_scopes() {
        let mut s = default_state();
        assert_eq!(s.scope(SettingId::HeadroomTelemetry), Scope::Off);

        s.cycle_scope(SettingId::HeadroomTelemetry);
        assert_eq!(s.scope(SettingId::HeadroomTelemetry), Scope::Global);
        assert!(s.global.headroom_telemetry);

        s.cycle_scope(SettingId::HeadroomTelemetry);
        assert_eq!(s.scope(SettingId::HeadroomTelemetry), Scope::Project);
        assert_eq!(s.project.headroom_telemetry, Some(true));

        s.cycle_scope(SettingId::HeadroomTelemetry);
        assert_eq!(s.scope(SettingId::HeadroomTelemetry), Scope::Off);
    }

    #[test]
    fn model_scope_detection() {
        let mut s = default_state();
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Off);

        s.global.api_model = Some("claude-opus-4-6".into());
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Global);

        s.project.api_model = Some("claude-sonnet-4-6".into());
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Project);
    }

    #[test]
    fn model_cycle_through_scopes() {
        let mut s = default_state();

        s.cycle_scope(SettingId::ApiModel);
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Global);
        assert_eq!(s.global.api_model.as_deref(), Some(KNOWN_MODELS[0]));

        s.cycle_scope(SettingId::ApiModel);
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Project);
        assert_eq!(s.project.api_model.as_deref(), Some(KNOWN_MODELS[0]));

        s.cycle_scope(SettingId::ApiModel);
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Off);
        assert!(s.global.api_model.is_none());
        assert!(s.project.api_model.is_none());
    }

    #[test]
    fn model_value_cycling() {
        let mut s = default_state();
        s.selected = 1;
        s.global.api_model = Some(KNOWN_MODELS[0].to_string());

        s.cycle_model_value();
        assert_eq!(s.global.api_model.as_deref(), Some(KNOWN_MODELS[1]));

        s.cycle_model_value();
        assert_eq!(s.global.api_model.as_deref(), Some(KNOWN_MODELS[2]));
    }

    #[test]
    fn model_value_cycling_wraps() {
        let mut s = default_state();
        s.selected = 1;
        s.global.api_model = Some(KNOWN_MODELS[KNOWN_MODELS.len() - 1].to_string());

        s.cycle_model_value();
        assert_eq!(s.global.api_model.as_deref(), Some(KNOWN_MODELS[0]));
    }

    #[test]
    fn model_value_cycle_noop_when_off() {
        let mut s = default_state();
        s.selected = 1;
        s.cycle_model_value();
        assert!(s.global.api_model.is_none());
        assert!(s.project.api_model.is_none());
    }

    #[test]
    fn model_value_cycle_noop_for_non_model_setting() {
        let mut s = default_state();
        s.selected = 0;
        s.cycle_model_value();
        assert!(s.global.api_model.is_none());
    }

    #[test]
    fn dirty_detection() {
        let mut s = default_state();
        assert!(!s.dirty());
        s.cycle_scope(SettingId::HeadroomTelemetry);
        assert!(s.dirty());
    }

    #[test]
    fn project_model_inherits_global_on_promote() {
        let mut s = default_state();

        s.cycle_scope(SettingId::ApiModel);
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Global);

        s.global.api_model = Some("claude-sonnet-4-6".into());

        s.cycle_scope(SettingId::ApiModel);
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Project);
        assert_eq!(s.project.api_model.as_deref(), Some("claude-sonnet-4-6"));
    }
}
