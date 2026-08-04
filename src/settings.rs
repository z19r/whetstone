use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    DefaultTerminal, Frame,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{GlobalSettings, ProjectSettings, WhetstoneManifest};
use crate::memory::MemoryProvider;
use crate::ui;

const FALLBACK_MODELS: &[&str] = &[
    "claude-opus-4-8",
    "claude-opus-4-6",
    "claude-sonnet-5",
    "claude-sonnet-4-6",
    "claude-haiku-4-5-20251001",
    "claude-fable-5",
];

const MODELS_CACHE_TTL_SECS: u64 = 12 * 60 * 60;
const MODELS_API_URL: &str = "https://api.anthropic.com/v1/models";

#[derive(Deserialize)]
struct ModelsApiResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

#[derive(Serialize, Deserialize)]
struct ModelsCache {
    models: Vec<String>,
    timestamp: u64,
}

fn models_cache_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let dir = home.join(".cache").join("whetstone");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("models-cache.json"))
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_models_cache() -> Option<Vec<String>> {
    let path = models_cache_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let cache: ModelsCache = serde_json::from_str(&content).ok()?;
    if now_epoch().saturating_sub(cache.timestamp) > MODELS_CACHE_TTL_SECS {
        return None;
    }
    if cache.models.is_empty() {
        return None;
    }
    Some(cache.models)
}

fn write_models_cache(models: &[String]) {
    if let Some(path) = models_cache_path() {
        let cache = ModelsCache {
            models: models.to_vec(),
            timestamp: now_epoch(),
        };
        if let Ok(json) = serde_json::to_string(&cache) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn is_current_gen(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("claude-") else {
        return false;
    };
    rest.starts_with("opus-4-")
        || rest.starts_with("sonnet-4-")
        || rest.starts_with("haiku-4-")
        || rest.starts_with("fable-")
}

fn strip_date_suffix(id: &str) -> Option<&str> {
    if id.len() > 9 {
        let (base, suffix) = id.split_at(id.len() - 9);
        if suffix.starts_with('-')
            && suffix[1..].bytes().all(|b| b.is_ascii_digit())
        {
            return Some(base);
        }
    }
    None
}

pub(crate) fn family_order(id: &str) -> u8 {
    if id.contains("-opus-") {
        0
    } else if id.contains("-sonnet-") {
        1
    } else if id.contains("-haiku-") {
        2
    } else if id.contains("-fable-") {
        3
    } else {
        4
    }
}

fn fetch_models_from_api() -> Option<Vec<String>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").ok()?;

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();

    let body_str = agent
        .get(MODELS_API_URL)
        .set("x-api-key", &api_key)
        .set("anthropic-version", "2023-06-01")
        .call()
        .ok()?
        .into_string()
        .ok()?;

    let body: ModelsApiResponse = serde_json::from_str(&body_str).ok()?;

    let mut models: Vec<String> = body
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| is_current_gen(id))
        .collect();

    let all_ids: HashSet<String> = models.iter().cloned().collect();
    models.retain(|id| match strip_date_suffix(id) {
        Some(base) => !all_ids.contains(base),
        None => true,
    });

    models.sort_by(|a, b| family_order(a).cmp(&family_order(b)).then(b.cmp(a)));

    if models.is_empty() {
        return None;
    }

    Some(models)
}

/// Available models from the 12h cache or the live API, or `None` when neither
/// is reachable. Distinct from [`load_available_models`], which substitutes the
/// static `FALLBACK_MODELS` — callers that need to know whether availability
/// was *actually* determined (vs. guessed) use this instead.
pub(crate) fn live_available_models() -> Option<Vec<String>> {
    if let Some(cached) = read_models_cache() {
        return Some(cached);
    }

    if let Some(fetched) = fetch_models_from_api() {
        write_models_cache(&fetched);
        return Some(fetched);
    }

    None
}

fn load_available_models() -> Vec<String> {
    live_available_models().unwrap_or_else(|| {
        FALLBACK_MODELS.iter().map(|s| s.to_string()).collect()
    })
}

/// Newest Sonnet in `models`, by lexical id (matching the descending sort the
/// picker uses: `claude-sonnet-5` > `claude-sonnet-4-6`). `None` when the list
/// has no Sonnet. Independent of input ordering so it's safe to unit-test.
pub(crate) fn newest_sonnet(models: &[String]) -> Option<String> {
    models
        .iter()
        .filter(|id| id.contains("-sonnet-"))
        .max_by(|a, b| a.as_str().cmp(b.as_str()))
        .cloned()
}

/// The model whetstone uses when the user hasn't pinned one: the newest
/// available Sonnet per the live/cached models list. Returns `None` when
/// availability can't be determined (offline / no `ANTHROPIC_API_KEY`) so the
/// caller falls back to its own pinned default rather than guessing.
pub fn preferred_default_model() -> Option<String> {
    newest_sonnet(&live_available_models()?)
}

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
    HeadroomMemory,
    ApiModel,
    AnthropicApiUrl,
}

const SETTINGS: &[SettingId] = &[
    SettingId::HeadroomTelemetry,
    SettingId::HeadroomMemory,
    SettingId::ApiModel,
    SettingId::AnthropicApiUrl,
];

struct SettingsState {
    global: GlobalSettings,
    project: ProjectSettings,
    original_global: GlobalSettings,
    original_project: ProjectSettings,
    selected: usize,
    models: Vec<String>,
    /// `Some(buffer)` while the user is typing a free-text value (the Anthropic
    /// API URL); `None` in normal navigation mode.
    editing: Option<String>,
}

impl SettingsState {
    fn scope(&self, id: SettingId) -> Scope {
        match id {
            SettingId::HeadroomTelemetry => {
                match self.project.headroom_telemetry {
                    Some(true) => Scope::Project,
                    Some(false) => Scope::Off,
                    None if self.global.headroom_telemetry => Scope::Global,
                    None => Scope::Off,
                }
            }
            SettingId::HeadroomMemory => match self.project.headroom_memory {
                Some(true) => Scope::Project,
                Some(false) => Scope::Off,
                None if self.global.headroom_memory => Scope::Global,
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
            SettingId::AnthropicApiUrl => {
                if self.project.anthropic_api_url.is_some() {
                    Scope::Project
                } else if self.global.anthropic_api_url.is_some() {
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
            SettingId::HeadroomMemory => match next {
                Scope::Off => {
                    self.global.headroom_memory = false;
                    self.project.headroom_memory = None;
                }
                Scope::Global => {
                    self.global.headroom_memory = true;
                    self.project.headroom_memory = None;
                }
                Scope::Project => {
                    self.project.headroom_memory = Some(true);
                }
            },
            SettingId::ApiModel => match next {
                Scope::Off => {
                    self.global.api_model = None;
                    self.project.api_model = None;
                }
                Scope::Global => {
                    if self.global.api_model.is_none() {
                        self.global.api_model = Some(self.models[0].clone());
                    }
                    self.project.api_model = None;
                }
                Scope::Project => {
                    let base = self
                        .global
                        .api_model
                        .clone()
                        .unwrap_or_else(|| self.models[0].clone());
                    self.project.api_model = Some(base);
                }
            },
            SettingId::AnthropicApiUrl => match next {
                Scope::Off => {
                    self.global.anthropic_api_url = None;
                    self.project.anthropic_api_url = None;
                }
                Scope::Global => {
                    if self.global.anthropic_api_url.is_none() {
                        self.global.anthropic_api_url = Some(String::new());
                    }
                    self.project.anthropic_api_url = None;
                }
                Scope::Project => {
                    let base = self
                        .global
                        .anthropic_api_url
                        .clone()
                        .unwrap_or_default();
                    self.project.anthropic_api_url = Some(base);
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
        let current = target.as_deref().unwrap_or(&self.models[0]);
        let idx = self
            .models
            .iter()
            .position(|m| m == current)
            .map(|i| (i + 1) % self.models.len())
            .unwrap_or(0);
        *target = Some(self.models[idx].clone());
    }

    /// The stored value shown next to a value-bearing setting (model id or API
    /// URL), or `None` for toggle settings and `Scope::Off`.
    fn current_value_display(&self, id: SettingId) -> Option<&str> {
        match id {
            SettingId::ApiModel => match self.scope(id) {
                Scope::Off => None,
                Scope::Global => self.global.api_model.as_deref(),
                Scope::Project => self.project.api_model.as_deref(),
            },
            SettingId::AnthropicApiUrl => match self.scope(id) {
                Scope::Off => None,
                Scope::Global => self.global.anthropic_api_url.as_deref(),
                Scope::Project => self.project.anthropic_api_url.as_deref(),
            },
            _ => None,
        }
    }

    /// Enter text-editing mode for the currently selected URL setting, seeding
    /// the buffer with the active value. No-op unless the selection is the
    /// Anthropic API URL and its scope is active.
    fn begin_edit(&mut self) {
        let id = SETTINGS[self.selected];
        if id != SettingId::AnthropicApiUrl || self.scope(id) == Scope::Off {
            return;
        }
        let current = self
            .current_value_display(id)
            .unwrap_or_default()
            .to_string();
        self.editing = Some(current);
    }

    /// Commit the in-progress edit buffer into the active scope, then leave edit
    /// mode. A blank buffer clears the value (which drops the scope to `Off`).
    fn commit_edit(&mut self) {
        let Some(buffer) = self.editing.take() else {
            return;
        };
        let trimmed = buffer.trim();
        let value = (!trimmed.is_empty()).then(|| trimmed.to_string());
        match self.scope(SettingId::AnthropicApiUrl) {
            Scope::Global | Scope::Off => {
                self.global.anthropic_api_url = value;
                self.project.anthropic_api_url = None;
            }
            Scope::Project => {
                self.project.anthropic_api_url = value;
            }
        }
    }

    fn dirty(&self) -> bool {
        self.global != self.original_global
            || self.project != self.original_project
    }
}

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let manifest_path = WhetstoneManifest::path_for(&cwd);

    let (manifest, had_manifest) =
        match WhetstoneManifest::load(&manifest_path)? {
            Some(m) => (Some(m), true),
            None => (None, false),
        };

    let global = GlobalSettings::load()?;
    let project = manifest
        .as_ref()
        .map(|m| m.settings.clone())
        .unwrap_or_default();

    let models = load_available_models();

    let mut terminal = ratatui::init();
    let result =
        run_loop(&mut terminal, global.clone(), project.clone(), models);
    ratatui::restore();

    match result {
        Ok(Some(mut saved)) => {
            normalize_saved(&mut saved);
            let global_changed = saved.global != global;
            let project_changed = saved.project != project;

            if global_changed {
                saved.global.save()?;
            }
            if project_changed {
                save_project_settings(
                    &manifest_path,
                    manifest,
                    had_manifest,
                    saved.project,
                )?;
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
        WhetstoneManifest::new(
            MemoryProvider::Skip,
            crate::config::ToolVersions::default(),
        )
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

/// Drop blank free-text values (a scope was toggled on but never filled in) to
/// `None` so an empty API URL never persists to disk.
fn normalize_saved(saved: &mut SavedState) {
    let blank = |v: &Option<String>| {
        v.as_deref().map(str::trim).is_some_and(str::is_empty)
    };
    if blank(&saved.global.anthropic_api_url) {
        saved.global.anthropic_api_url = None;
    }
    if blank(&saved.project.anthropic_api_url) {
        saved.project.anthropic_api_url = None;
    }
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    global: GlobalSettings,
    project: ProjectSettings,
    models: Vec<String>,
) -> Result<Option<SavedState>> {
    let mut state = SettingsState {
        original_global: global.clone(),
        original_project: project.clone(),
        global,
        project,
        selected: 0,
        models,
        editing: None,
    };

    loop {
        terminal.draw(|frame| draw(frame, &state))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if state.editing.is_some() {
                    handle_edit_key(&mut state, key.code);
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
                        match SETTINGS[state.selected] {
                            SettingId::ApiModel => state.cycle_model_value(),
                            SettingId::AnthropicApiUrl => state.begin_edit(),
                            _ => {}
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.selected = state.selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j')
                        if state.selected + 1 < SETTINGS.len() =>
                    {
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

/// Route a keypress while the URL text buffer is open. Enter commits, Esc
/// cancels, and printable characters / Backspace edit the buffer in place.
fn handle_edit_key(state: &mut SettingsState, code: KeyCode) {
    match code {
        KeyCode::Enter => state.commit_edit(),
        KeyCode::Esc => {
            state.editing = None;
        }
        KeyCode::Backspace => {
            if let Some(buffer) = state.editing.as_mut() {
                buffer.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(buffer) = state.editing.as_mut() {
                buffer.push(c);
            }
        }
        _ => {}
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
            SettingId::HeadroomMemory => (
                "Headroom Memory",
                "Enable Headroom persistent cross-session memory (proxy --memory)",
            ),
            SettingId::ApiModel => ("API Model", "Model used for Claude Code sessions"),
            SettingId::AnthropicApiUrl => (
                "Anthropic API URL",
                "Custom upstream Anthropic API URL for the Headroom proxy",
            ),
        };

        let scope = state.scope(id);
        let is_editing = is_selected
            && id == SettingId::AnthropicApiUrl
            && state.editing.is_some();

        let mut row = vec![Span::styled(
            cursor,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];
        row.extend(scope_spans(scope));
        row.push(Span::raw("  "));
        row.push(Span::styled(label.to_string(), label_style));

        if is_editing {
            let buffer = state.editing.as_deref().unwrap_or_default();
            row.push(Span::styled(
                format!("  {buffer}"),
                Style::default().fg(Color::Yellow),
            ));
            row.push(Span::styled(
                "█",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::DIM),
            ));
        } else if let Some(value) = state.current_value_display(id) {
            let shown = if value.is_empty() { "(unset)" } else { value };
            row.push(Span::styled(
                format!("  {shown}"),
                Style::default().fg(Color::Yellow),
            ));
        }

        lines.push(Line::from(row));

        let desc_line = if id == SettingId::ApiModel && scope != Scope::Off {
            format!("{desc}  (tab to cycle model)")
        } else if id == SettingId::AnthropicApiUrl && scope != Scope::Off {
            if is_editing {
                format!("{desc}  (enter to save, esc to cancel)")
            } else {
                format!("{desc}  (enter to edit)")
            }
        } else {
            desc.to_string()
        };

        lines.push(Line::from(vec![
            Span::raw("                  "),
            Span::styled(
                desc_line,
                Style::default().add_modifier(Modifier::DIM),
            ),
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

    fn test_models() -> Vec<String> {
        FALLBACK_MODELS.iter().map(|s| s.to_string()).collect()
    }

    fn default_state() -> SettingsState {
        SettingsState {
            global: GlobalSettings::default(),
            project: ProjectSettings::default(),
            original_global: GlobalSettings::default(),
            original_project: ProjectSettings::default(),
            selected: 0,
            models: test_models(),
            editing: None,
        }
    }

    fn url_index() -> usize {
        SETTINGS
            .iter()
            .position(|&s| s == SettingId::AnthropicApiUrl)
            .unwrap()
    }

    #[test]
    fn api_url_scope_detection() {
        let mut s = default_state();
        assert_eq!(s.scope(SettingId::AnthropicApiUrl), Scope::Off);

        s.global.anthropic_api_url = Some("https://global.example".into());
        assert_eq!(s.scope(SettingId::AnthropicApiUrl), Scope::Global);

        s.project.anthropic_api_url = Some("https://project.example".into());
        assert_eq!(s.scope(SettingId::AnthropicApiUrl), Scope::Project);
    }

    #[test]
    fn api_url_cycle_through_scopes() {
        let mut s = default_state();

        s.cycle_scope(SettingId::AnthropicApiUrl);
        assert_eq!(s.scope(SettingId::AnthropicApiUrl), Scope::Global);
        assert_eq!(s.global.anthropic_api_url.as_deref(), Some(""));

        s.cycle_scope(SettingId::AnthropicApiUrl);
        assert_eq!(s.scope(SettingId::AnthropicApiUrl), Scope::Project);
        assert_eq!(s.project.anthropic_api_url.as_deref(), Some(""));

        s.cycle_scope(SettingId::AnthropicApiUrl);
        assert_eq!(s.scope(SettingId::AnthropicApiUrl), Scope::Off);
        assert!(s.global.anthropic_api_url.is_none());
        assert!(s.project.anthropic_api_url.is_none());
    }

    #[test]
    fn begin_edit_noop_when_scope_off() {
        let mut s = default_state();
        s.selected = url_index();
        s.begin_edit();
        assert!(s.editing.is_none());
    }

    #[test]
    fn begin_edit_seeds_buffer_with_current_value() {
        let mut s = default_state();
        s.selected = url_index();
        s.global.anthropic_api_url = Some("https://seed.example".into());
        s.begin_edit();
        assert_eq!(s.editing.as_deref(), Some("https://seed.example"));
    }

    #[test]
    fn commit_edit_writes_global_value() {
        let mut s = default_state();
        s.selected = url_index();
        s.global.anthropic_api_url = Some(String::new());
        s.editing = Some("https://typed.example".into());
        s.commit_edit();
        assert!(s.editing.is_none());
        assert_eq!(
            s.global.anthropic_api_url.as_deref(),
            Some("https://typed.example")
        );
    }

    #[test]
    fn commit_edit_writes_project_value() {
        let mut s = default_state();
        s.selected = url_index();
        s.project.anthropic_api_url = Some(String::new());
        s.editing = Some("  https://project.example  ".into());
        s.commit_edit();
        assert_eq!(
            s.project.anthropic_api_url.as_deref(),
            Some("https://project.example")
        );
    }

    #[test]
    fn commit_edit_blank_clears_value() {
        let mut s = default_state();
        s.selected = url_index();
        s.global.anthropic_api_url = Some("https://old.example".into());
        s.editing = Some("   ".into());
        s.commit_edit();
        assert!(s.global.anthropic_api_url.is_none());
        assert_eq!(s.scope(SettingId::AnthropicApiUrl), Scope::Off);
    }

    #[test]
    fn edit_key_typing_and_backspace() {
        let mut s = default_state();
        s.selected = url_index();
        s.editing = Some(String::new());
        handle_edit_key(&mut s, KeyCode::Char('h'));
        handle_edit_key(&mut s, KeyCode::Char('i'));
        handle_edit_key(&mut s, KeyCode::Backspace);
        assert_eq!(s.editing.as_deref(), Some("h"));
    }

    #[test]
    fn normalize_saved_drops_blank_urls() {
        let mut saved = SavedState {
            global: GlobalSettings {
                anthropic_api_url: Some(String::new()),
                ..Default::default()
            },
            project: ProjectSettings {
                anthropic_api_url: Some("https://keep.example".into()),
                ..Default::default()
            },
        };
        normalize_saved(&mut saved);
        assert!(saved.global.anthropic_api_url.is_none());
        assert_eq!(
            saved.project.anthropic_api_url.as_deref(),
            Some("https://keep.example")
        );
    }

    #[test]
    fn edit_key_esc_cancels_without_writing() {
        let mut s = default_state();
        s.selected = url_index();
        s.global.anthropic_api_url = Some("https://keep.example".into());
        s.editing = Some("https://discard.example".into());
        handle_edit_key(&mut s, KeyCode::Esc);
        assert!(s.editing.is_none());
        assert_eq!(
            s.global.anthropic_api_url.as_deref(),
            Some("https://keep.example")
        );
    }

    #[test]
    fn newest_sonnet_picks_highest_version() {
        let models = vec![
            "claude-opus-4-8".to_string(),
            "claude-sonnet-4-6".to_string(),
            "claude-sonnet-5".to_string(),
            "claude-haiku-4-5".to_string(),
        ];
        assert_eq!(newest_sonnet(&models).as_deref(), Some("claude-sonnet-5"));
    }

    #[test]
    fn newest_sonnet_ignores_input_order() {
        let models = vec![
            "claude-sonnet-5".to_string(),
            "claude-sonnet-4-6".to_string(),
        ];
        assert_eq!(newest_sonnet(&models).as_deref(), Some("claude-sonnet-5"));
    }

    #[test]
    fn newest_sonnet_none_when_absent() {
        let models = vec![
            "claude-opus-4-8".to_string(),
            "claude-haiku-4-5".to_string(),
        ];
        assert_eq!(newest_sonnet(&models), None);
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
    fn memory_cycle_through_all_scopes() {
        let mut s = default_state();
        assert_eq!(s.scope(SettingId::HeadroomMemory), Scope::Off);

        s.cycle_scope(SettingId::HeadroomMemory);
        assert_eq!(s.scope(SettingId::HeadroomMemory), Scope::Global);
        assert!(s.global.headroom_memory);

        s.cycle_scope(SettingId::HeadroomMemory);
        assert_eq!(s.scope(SettingId::HeadroomMemory), Scope::Project);
        assert_eq!(s.project.headroom_memory, Some(true));

        s.cycle_scope(SettingId::HeadroomMemory);
        assert_eq!(s.scope(SettingId::HeadroomMemory), Scope::Off);
        assert!(!s.global.headroom_memory);
        assert!(s.project.headroom_memory.is_none());
    }

    #[test]
    fn memory_scope_from_project_override() {
        let mut s = default_state();
        s.project.headroom_memory = Some(false);
        assert_eq!(s.scope(SettingId::HeadroomMemory), Scope::Off);

        s.project.headroom_memory = Some(true);
        assert_eq!(s.scope(SettingId::HeadroomMemory), Scope::Project);
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
        assert_eq!(s.global.api_model.as_deref(), Some(FALLBACK_MODELS[0]));

        s.cycle_scope(SettingId::ApiModel);
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Project);
        assert_eq!(s.project.api_model.as_deref(), Some(FALLBACK_MODELS[0]));

        s.cycle_scope(SettingId::ApiModel);
        assert_eq!(s.scope(SettingId::ApiModel), Scope::Off);
        assert!(s.global.api_model.is_none());
        assert!(s.project.api_model.is_none());
    }

    #[test]
    fn model_value_cycling() {
        let mut s = default_state();
        s.selected = 2;
        s.global.api_model = Some(FALLBACK_MODELS[0].to_string());

        s.cycle_model_value();
        assert_eq!(s.global.api_model.as_deref(), Some(FALLBACK_MODELS[1]));

        s.cycle_model_value();
        assert_eq!(s.global.api_model.as_deref(), Some(FALLBACK_MODELS[2]));
    }

    #[test]
    fn model_value_cycling_wraps() {
        let mut s = default_state();
        s.selected = 2;
        s.global.api_model =
            Some(FALLBACK_MODELS[FALLBACK_MODELS.len() - 1].to_string());

        s.cycle_model_value();
        assert_eq!(s.global.api_model.as_deref(), Some(FALLBACK_MODELS[0]));
    }

    #[test]
    fn model_value_cycle_noop_when_off() {
        let mut s = default_state();
        s.selected = 2;
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

    #[test]
    fn is_current_gen_accepts_latest_families() {
        assert!(is_current_gen("claude-opus-4-8"));
        assert!(is_current_gen("claude-opus-4-6"));
        assert!(is_current_gen("claude-sonnet-4-6"));
        assert!(is_current_gen("claude-haiku-4-5-20251001"));
        assert!(is_current_gen("claude-fable-5"));
    }

    #[test]
    fn is_current_gen_rejects_older_families() {
        assert!(!is_current_gen("claude-3-5-sonnet-20241022"));
        assert!(!is_current_gen("claude-3-opus-20240229"));
        assert!(!is_current_gen("gpt-4o"));
    }

    #[test]
    fn strip_date_suffix_extracts_base() {
        assert_eq!(
            strip_date_suffix("claude-opus-4-8-20260501"),
            Some("claude-opus-4-8")
        );
        assert_eq!(
            strip_date_suffix("claude-sonnet-4-6-20250514"),
            Some("claude-sonnet-4-6")
        );
    }

    #[test]
    fn strip_date_suffix_returns_none_for_short_ids() {
        assert_eq!(strip_date_suffix("claude-opus-4-8"), None);
        assert_eq!(strip_date_suffix("claude-fable-5"), None);
    }

    #[test]
    fn family_order_sorts_correctly() {
        assert!(
            family_order("claude-opus-4-8") < family_order("claude-sonnet-4-6")
        );
        assert!(
            family_order("claude-sonnet-4-6")
                < family_order("claude-haiku-4-5-20251001")
        );
        assert!(
            family_order("claude-haiku-4-5-20251001")
                < family_order("claude-fable-5")
        );
    }
}
