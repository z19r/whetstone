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

use crate::config::{
    GlobalSettings, ProjectSettings, WhetstoneManifest, PERMISSION_MODES,
    SAVINGS_PROFILES,
};
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

/// Where a setting's value is stored. This is a *separate axis* from the
/// value itself: a setting is `Off` (unset / default behaviour), or active at
/// `Global` scope (`~/.whetstone/settings.json`, all projects) or `Project`
/// scope (`.claude/whetstone.json`, this project only, wins over global).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Off,
    Global,
    Project,
}

/// Direction a value control moves when the user presses ←/→.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Prev,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingId {
    HeadroomTelemetry,
    HeadroomBeacon,
    HeadroomMemory,
    HeadroomTargetRatio,
    HeadroomBudget,
    HeadroomLogMessages,
    HeadroomRolloutChannel,
    HeadroomOutputShaper,
    ApiModel,
    PermissionMode,
    HeadroomSavingsProfile,
    EnableToolSearch,
    AnthropicApiUrl,
}

const SETTINGS: &[SettingId] = &[
    SettingId::HeadroomTelemetry,
    SettingId::HeadroomBeacon,
    SettingId::HeadroomMemory,
    SettingId::HeadroomTargetRatio,
    SettingId::HeadroomBudget,
    SettingId::HeadroomLogMessages,
    SettingId::HeadroomRolloutChannel,
    SettingId::HeadroomOutputShaper,
    SettingId::ApiModel,
    SettingId::PermissionMode,
    SettingId::HeadroomSavingsProfile,
    SettingId::EnableToolSearch,
    SettingId::AnthropicApiUrl,
];

/// The shape of a setting's *value* control, independent of its scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    /// A plain On/Off switch (←/→ toggles).
    Bool,
    /// A pick from a fixed list (←/→ cycles through Off + each choice).
    Choice,
    /// A free-text value the user types (enter to edit).
    Text,
}

fn control_kind(id: SettingId) -> Control {
    match id {
        SettingId::HeadroomTelemetry
        | SettingId::HeadroomBeacon
        | SettingId::HeadroomMemory
        | SettingId::HeadroomLogMessages
        | SettingId::EnableToolSearch => Control::Bool,
        SettingId::ApiModel
        | SettingId::PermissionMode
        | SettingId::HeadroomSavingsProfile => Control::Choice,
        SettingId::HeadroomTargetRatio
        | SettingId::HeadroomBudget
        | SettingId::HeadroomRolloutChannel
        | SettingId::HeadroomOutputShaper
        | SettingId::AnthropicApiUrl => Control::Text,
    }
}

/// Display name shown next to the value control.
fn label(id: SettingId) -> &'static str {
    match id {
        SettingId::HeadroomTelemetry => "Headroom Telemetry",
        SettingId::HeadroomBeacon => "Headroom Beacon",
        SettingId::HeadroomMemory => "Headroom Memory",
        SettingId::HeadroomTargetRatio => "Headroom Target Ratio",
        SettingId::HeadroomBudget => "Headroom Budget",
        SettingId::HeadroomLogMessages => "Headroom Log Messages",
        SettingId::HeadroomRolloutChannel => "Headroom Rollout Channel",
        SettingId::HeadroomOutputShaper => "Headroom Output Shaper",
        SettingId::ApiModel => "API Model",
        SettingId::PermissionMode => "Edit Mode",
        SettingId::HeadroomSavingsProfile => "Headroom Savings Profile",
        SettingId::EnableToolSearch => "Tool Search",
        SettingId::AnthropicApiUrl => "Anthropic API URL",
    }
}

/// How a `headroom_env` map-backed setting behaves: a free-text `Value` the
/// user types, or a `Toggle` that writes a fixed value when active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvKind {
    Value,
    Toggle(&'static str),
}

/// The `HEADROOM_*` map key and behaviour for the settings backed by the
/// `headroom_env` passthrough map. `None` for settings that live in their own
/// dedicated fields (telemetry, memory, model, edit mode, API URL).
fn env_knob(id: SettingId) -> Option<(&'static str, EnvKind)> {
    match id {
        SettingId::HeadroomTargetRatio => {
            Some(("HEADROOM_TARGET_RATIO", EnvKind::Value))
        }
        SettingId::HeadroomBudget => Some(("HEADROOM_BUDGET", EnvKind::Value)),
        SettingId::HeadroomRolloutChannel => {
            Some(("HEADROOM_ROLLOUT_CHANNEL", EnvKind::Value))
        }
        SettingId::HeadroomOutputShaper => {
            Some(("HEADROOM_OUTPUT_SHAPER", EnvKind::Value))
        }
        SettingId::HeadroomLogMessages => {
            Some(("HEADROOM_LOG_MESSAGES", EnvKind::Toggle("1")))
        }
        SettingId::HeadroomBeacon => {
            Some(("HEADROOM_BEACON", EnvKind::Toggle("off")))
        }
        _ => None,
    }
}

/// Whether a setting accepts a typed free-text value (enter to edit).
fn is_value_editable(id: SettingId) -> bool {
    control_kind(id) == Control::Text
}

struct SettingsState {
    global: GlobalSettings,
    project: ProjectSettings,
    original_global: GlobalSettings,
    original_project: ProjectSettings,
    selected: usize,
    models: Vec<String>,
    /// `Some(buffer)` while the user is typing a free-text value; `None` in
    /// normal navigation mode.
    editing: Option<String>,
}

impl SettingsState {
    /// Where the given setting is currently stored (or `Off` when unset).
    fn scope(&self, id: SettingId) -> Scope {
        if let Some((key, _)) = env_knob(id) {
            return self.env_scope(key);
        }
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
            SettingId::EnableToolSearch => {
                match self.project.enable_tool_search {
                    Some(true) => Scope::Project,
                    Some(false) => Scope::Off,
                    None if self.global.enable_tool_search => Scope::Global,
                    None => Scope::Off,
                }
            }
            SettingId::ApiModel => {
                if self.project.api_model.is_some() {
                    Scope::Project
                } else if self.global.api_model.is_some() {
                    Scope::Global
                } else {
                    Scope::Off
                }
            }
            SettingId::PermissionMode => {
                if self.project.permission_mode.is_some() {
                    Scope::Project
                } else if self.global.permission_mode.is_some() {
                    Scope::Global
                } else {
                    Scope::Off
                }
            }
            SettingId::HeadroomSavingsProfile => {
                if self.project.savings_profile.is_some() {
                    Scope::Project
                } else if self.global.savings_profile.is_some() {
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
            // env-backed settings handled above
            SettingId::HeadroomTargetRatio
            | SettingId::HeadroomBudget
            | SettingId::HeadroomLogMessages
            | SettingId::HeadroomRolloutChannel
            | SettingId::HeadroomOutputShaper
            | SettingId::HeadroomBeacon => unreachable!(),
        }
    }

    /// Move a setting to a specific scope, preserving its value where possible.
    /// This is the single writer that both the value controls and the scope
    /// toggle drive.
    fn set_scope(&mut self, id: SettingId, target: Scope) {
        if let Some((key, kind)) = env_knob(id) {
            let default_val = match kind {
                EnvKind::Toggle(v) => v,
                EnvKind::Value => "",
            };
            self.set_env_scope(key, default_val, target);
            return;
        }
        match id {
            SettingId::HeadroomTelemetry => match target {
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
            SettingId::HeadroomMemory => match target {
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
            SettingId::EnableToolSearch => match target {
                Scope::Off => {
                    self.global.enable_tool_search = false;
                    self.project.enable_tool_search = None;
                }
                Scope::Global => {
                    self.global.enable_tool_search = true;
                    self.project.enable_tool_search = None;
                }
                Scope::Project => {
                    self.project.enable_tool_search = Some(true);
                }
            },
            SettingId::ApiModel => match target {
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
            SettingId::PermissionMode => match target {
                Scope::Off => {
                    self.global.permission_mode = None;
                    self.project.permission_mode = None;
                }
                Scope::Global => {
                    if self.global.permission_mode.is_none() {
                        self.global.permission_mode =
                            Some(PERMISSION_MODES[0].to_string());
                    }
                    self.project.permission_mode = None;
                }
                Scope::Project => {
                    let base = self
                        .global
                        .permission_mode
                        .clone()
                        .unwrap_or_else(|| PERMISSION_MODES[0].to_string());
                    self.project.permission_mode = Some(base);
                }
            },
            SettingId::HeadroomSavingsProfile => match target {
                Scope::Off => {
                    self.global.savings_profile = None;
                    self.project.savings_profile = None;
                }
                Scope::Global => {
                    if self.global.savings_profile.is_none() {
                        self.global.savings_profile =
                            Some(SAVINGS_PROFILES[0].to_string());
                    }
                    self.project.savings_profile = None;
                }
                Scope::Project => {
                    let base = self
                        .global
                        .savings_profile
                        .clone()
                        .unwrap_or_else(|| SAVINGS_PROFILES[0].to_string());
                    self.project.savings_profile = Some(base);
                }
            },
            SettingId::AnthropicApiUrl => match target {
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
            SettingId::HeadroomTargetRatio
            | SettingId::HeadroomBudget
            | SettingId::HeadroomLogMessages
            | SettingId::HeadroomRolloutChannel
            | SettingId::HeadroomOutputShaper
            | SettingId::HeadroomBeacon => unreachable!(),
        }
    }

    /// Scope of a `headroom_env` map key: `Project` if the project map holds
    /// it, else `Global` if the global map does, else `Off`.
    fn env_scope(&self, key: &str) -> Scope {
        if self.project.headroom_env.contains_key(key) {
            Scope::Project
        } else if self.global.headroom_env.contains_key(key) {
            Scope::Global
        } else {
            Scope::Off
        }
    }

    /// The active value for a `headroom_env` key, or `None` when `Off`.
    fn env_value(&self, key: &str) -> Option<&str> {
        match self.env_scope(key) {
            Scope::Off => None,
            Scope::Global => {
                self.global.headroom_env.get(key).map(String::as_str)
            }
            Scope::Project => {
                self.project.headroom_env.get(key).map(String::as_str)
            }
        }
    }

    /// Move a `headroom_env` key to `target`, keeping any value the user has
    /// already typed and falling back to `default_val` when there is none.
    fn set_env_scope(&mut self, key: &str, default_val: &str, target: Scope) {
        let cur = self.env_value(key).map(str::to_string);
        match target {
            Scope::Off => {
                self.global.headroom_env.remove(key);
                self.project.headroom_env.remove(key);
            }
            Scope::Global => {
                let v = cur.unwrap_or_else(|| default_val.to_string());
                self.global.headroom_env.insert(key.to_string(), v);
                self.project.headroom_env.remove(key);
            }
            Scope::Project => {
                let v = cur.unwrap_or_else(|| default_val.to_string());
                self.project.headroom_env.insert(key.to_string(), v);
            }
        }
    }

    /// Whether the setting reads as "On" to the user. For most booleans that is
    /// simply "stored somewhere". Beacon is inverted (opt-out): being *unset*
    /// leaves the beacon on, and storing `HEADROOM_BEACON=off` turns it off.
    fn value_on(&self, id: SettingId) -> bool {
        let active = self.scope(id) != Scope::Off;
        if id == SettingId::HeadroomBeacon {
            !active
        } else {
            active
        }
    }

    /// Toggle a boolean setting on/off. Turning on lands at `Global` scope; the
    /// user moves it to `Project` afterwards with the scope key.
    fn toggle_bool(&mut self, id: SettingId) {
        if self.scope(id) == Scope::Off {
            self.set_scope(id, Scope::Global);
        } else {
            self.set_scope(id, Scope::Off);
        }
    }

    /// Advance the value control in the given direction. Booleans flip; choice
    /// lists cycle through `Off` + each option; text opens the editor.
    fn cycle_value(&mut self, id: SettingId, dir: Dir) {
        match control_kind(id) {
            Control::Bool => self.toggle_bool(id),
            Control::Choice => self.cycle_choice(id, dir),
            Control::Text => self.begin_edit(),
        }
    }

    /// The choices for a `Control::Choice` setting (excluding the leading Off).
    fn choices(&self, id: SettingId) -> Vec<String> {
        match id {
            SettingId::ApiModel => self.models.clone(),
            SettingId::PermissionMode => {
                PERMISSION_MODES.iter().map(|s| s.to_string()).collect()
            }
            SettingId::HeadroomSavingsProfile => {
                SAVINGS_PROFILES.iter().map(|s| s.to_string()).collect()
            }
            _ => Vec::new(),
        }
    }

    /// Cycle a choice setting through `[Off, choice_0, choice_1, ...]`, writing
    /// the selected value into the active scope (defaulting to `Global` when it
    /// was previously `Off`).
    fn cycle_choice(&mut self, id: SettingId, dir: Dir) {
        let list = self.choices(id);
        if list.is_empty() {
            return;
        }
        let ring = list.len() + 1; // slot 0 == Off
        let cur = if self.scope(id) == Scope::Off {
            0
        } else {
            let value = self.current_value_display(id).unwrap_or_default();
            list.iter().position(|m| m == value).map_or(0, |i| i + 1)
        };
        let next = match dir {
            Dir::Next => (cur + 1) % ring,
            Dir::Prev => (cur + ring - 1) % ring,
        };
        if next == 0 {
            self.set_scope(id, Scope::Off);
        } else {
            if self.scope(id) == Scope::Off {
                self.set_scope(id, Scope::Global);
            }
            self.set_choice_value(id, list[next - 1].clone());
        }
    }

    /// Write a choice value into whichever scope is currently active.
    fn set_choice_value(&mut self, id: SettingId, value: String) {
        match (id, self.scope(id)) {
            (SettingId::ApiModel, Scope::Global) => {
                self.global.api_model = Some(value)
            }
            (SettingId::ApiModel, Scope::Project) => {
                self.project.api_model = Some(value)
            }
            (SettingId::PermissionMode, Scope::Global) => {
                self.global.permission_mode = Some(value)
            }
            (SettingId::PermissionMode, Scope::Project) => {
                self.project.permission_mode = Some(value)
            }
            (SettingId::HeadroomSavingsProfile, Scope::Global) => {
                self.global.savings_profile = Some(value)
            }
            (SettingId::HeadroomSavingsProfile, Scope::Project) => {
                self.project.savings_profile = Some(value)
            }
            _ => {}
        }
    }

    /// Flip an active setting between `Global` and `Project`. No-op when the
    /// setting is `Off` (scope is meaningless with no stored value).
    fn toggle_scope(&mut self, id: SettingId) {
        let other = match self.scope(id) {
            Scope::Global => Scope::Project,
            Scope::Project => Scope::Global,
            Scope::Off => return,
        };
        self.set_scope(id, other);
    }

    /// The stored value shown next to a value-bearing setting (model id, edit
    /// mode, or text), or `None` for `Scope::Off`.
    fn current_value_display(&self, id: SettingId) -> Option<&str> {
        if let Some((key, EnvKind::Value)) = env_knob(id) {
            return self.env_value(key);
        }
        match id {
            SettingId::ApiModel => match self.scope(id) {
                Scope::Off => None,
                Scope::Global => self.global.api_model.as_deref(),
                Scope::Project => self.project.api_model.as_deref(),
            },
            SettingId::PermissionMode => match self.scope(id) {
                Scope::Off => None,
                Scope::Global => self.global.permission_mode.as_deref(),
                Scope::Project => self.project.permission_mode.as_deref(),
            },
            SettingId::HeadroomSavingsProfile => match self.scope(id) {
                Scope::Off => None,
                Scope::Global => self.global.savings_profile.as_deref(),
                Scope::Project => self.project.savings_profile.as_deref(),
            },
            SettingId::AnthropicApiUrl => match self.scope(id) {
                Scope::Off => None,
                Scope::Global => self.global.anthropic_api_url.as_deref(),
                Scope::Project => self.project.anthropic_api_url.as_deref(),
            },
            _ => None,
        }
    }

    /// The short label shown in the value chip: `On`/`Off`, a chosen value, or
    /// the typed text.
    fn value_label(&self, id: SettingId) -> String {
        match control_kind(id) {
            Control::Bool => {
                if self.value_on(id) { "On" } else { "Off" }.to_string()
            }
            Control::Choice | Control::Text => {
                match self.current_value_display(id) {
                    Some(v) if !v.trim().is_empty() => v.to_string(),
                    _ => "Off".to_string(),
                }
            }
        }
    }

    /// Helper text describing what the setting does *in its current state*. This
    /// updates as the user cycles the value, so the meaning of the selected
    /// option is always spelled out.
    fn helper(&self, id: SettingId) -> String {
        match id {
            SettingId::HeadroomTelemetry => if self.value_on(id) {
                "On — Headroom sends anonymous usage data back to Headroom for service improvement."
            } else {
                "Off — no usage data is sent to Headroom."
            }
            .to_string(),
            SettingId::HeadroomBeacon => if self.value_on(id) {
                "On — Headroom uploads its anonymous telemetry beacon (Headroom default)."
            } else {
                "Off — opts out of the telemetry upload beacon (writes HEADROOM_BEACON=off)."
            }
            .to_string(),
            SettingId::HeadroomMemory => if self.value_on(id) {
                "On — Headroom keeps persistent cross-session memory (proxy --memory)."
            } else {
                "Off — each session starts fresh; no cross-session memory."
            }
            .to_string(),
            SettingId::HeadroomLogMessages => if self.value_on(id) {
                "On — Headroom logs full message content for debugging (HEADROOM_LOG_MESSAGES=1)."
            } else {
                "Off — message content is not logged."
            }
            .to_string(),
            SettingId::EnableToolSearch => if self.value_on(id) {
                "On — Claude Code turns on deferred-tool search (ENABLE_TOOL_SEARCH=1)."
            } else {
                "Off — Claude Code uses its own default; any external ENABLE_TOOL_SEARCH passes through."
            }
            .to_string(),
            SettingId::ApiModel => match self.current_value_display(id) {
                Some(m) => format!("Claude Code sessions use {m}."),
                None => {
                    "Off — Claude Code uses its own default model.".to_string()
                }
            },
            SettingId::PermissionMode => match self.current_value_display(id) {
                Some(m) => {
                    format!("Claude Code runs with --permission-mode {m}.")
                }
                None => "Off — Claude Code uses its own default permission mode."
                    .to_string(),
            },
            SettingId::HeadroomSavingsProfile => {
                match self.current_value_display(id) {
                    Some(p) => format!(
                        "Headroom compresses with the {p} savings profile (HEADROOM_SAVINGS_PROFILE)."
                    ),
                    None => "Off — Headroom uses its default savings profile (agent-90)."
                        .to_string(),
                }
            }
            SettingId::AnthropicApiUrl => match self.current_value_display(id) {
                Some(u) if !u.trim().is_empty() => {
                    format!("Headroom proxies to {u} instead of the default Anthropic API.")
                }
                _ => "Off — Headroom uses the default Anthropic API endpoint."
                    .to_string(),
            },
            SettingId::HeadroomTargetRatio => {
                match self.current_value_display(id) {
                    Some(v) if !v.trim().is_empty() => format!(
                        "Headroom keeps ~{v} of the context window (HEADROOM_TARGET_RATIO)."
                    ),
                    _ => "Off — Headroom uses its default context keep-ratio."
                        .to_string(),
                }
            }
            SettingId::HeadroomBudget => match self.current_value_display(id) {
                Some(v) if !v.trim().is_empty() => {
                    format!("Headroom stops after ${v} of proxy spend (HEADROOM_BUDGET).")
                }
                _ => "Off — no spend cap on the Headroom proxy.".to_string(),
            },
            SettingId::HeadroomRolloutChannel => {
                match self.current_value_display(id) {
                    Some(v) if !v.trim().is_empty() => format!(
                        "Headroom uses the {v} rollout channel (HEADROOM_ROLLOUT_CHANNEL)."
                    ),
                    _ => "Off — Headroom uses its default rollout channel."
                        .to_string(),
                }
            }
            SettingId::HeadroomOutputShaper => {
                match self.current_value_display(id) {
                    Some(v) if !v.trim().is_empty() => format!(
                        "Headroom shapes output with {v} (HEADROOM_OUTPUT_SHAPER)."
                    ),
                    _ => "Off — Headroom uses its default output shaper."
                        .to_string(),
                }
            }
        }
    }

    /// Enter text-editing mode for the selected text setting, seeding the buffer
    /// with the active value. No-op unless the selection is a text control.
    fn begin_edit(&mut self) {
        let id = SETTINGS[self.selected];
        if !is_value_editable(id) {
            return;
        }
        let current = self
            .current_value_display(id)
            .unwrap_or_default()
            .to_string();
        self.editing = Some(current);
    }

    /// Commit the in-progress edit buffer into the active scope (defaulting to
    /// `Global` when the setting was `Off`), then leave edit mode. A blank
    /// buffer clears the value (dropping the scope to `Off`).
    fn commit_edit(&mut self) {
        let Some(buffer) = self.editing.take() else {
            return;
        };
        let trimmed = buffer.trim();
        let value = (!trimmed.is_empty()).then(|| trimmed.to_string());
        let id = SETTINGS[self.selected];

        if let Some((key, EnvKind::Value)) = env_knob(id) {
            let target = match self.env_scope(key) {
                Scope::Project => &mut self.project.headroom_env,
                Scope::Global | Scope::Off => {
                    self.project.headroom_env.remove(key);
                    &mut self.global.headroom_env
                }
            };
            match value {
                Some(v) => {
                    target.insert(key.to_string(), v);
                }
                None => {
                    target.remove(key);
                }
            }
            return;
        }

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

/// Drop blank free-text values (a scope was toggled on but never filled in) so
/// an empty API URL or `HEADROOM_*` value never persists to disk.
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
    saved
        .global
        .headroom_env
        .retain(|_, v| !v.trim().is_empty());
    saved
        .project
        .headroom_env
        .retain(|_, v| !v.trim().is_empty());
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
                let id = SETTINGS[state.selected];
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
                    KeyCode::Left | KeyCode::Char('h') => {
                        state.cycle_value(id, Dir::Prev);
                    }
                    KeyCode::Right
                    | KeyCode::Char('l')
                    | KeyCode::Char(' ') => {
                        state.cycle_value(id, Dir::Next);
                    }
                    KeyCode::Enter => {
                        if control_kind(id) == Control::Text {
                            state.begin_edit();
                        } else {
                            state.cycle_value(id, Dir::Next);
                        }
                    }
                    KeyCode::Char('g') | KeyCode::Char('G') => {
                        state.toggle_scope(id);
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

/// Route a keypress while a text buffer is open. Enter commits, Esc cancels, and
/// printable characters / Backspace edit the buffer in place.
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

/// The `[ … ]` value chip. Green for On, dim for Off, yellow for a set value.
fn value_chip(state: &SettingsState, id: SettingId) -> Span<'static> {
    let text = state.value_label(id);
    let is_off = text == "Off";
    let style = if is_off {
        Style::default().fg(Color::DarkGray)
    } else if control_kind(id) == Control::Bool {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    };
    Span::styled(format!("[ {text} ]"), style)
}

/// The `· Global` / `· Project` badge shown only when the value is stored
/// somewhere (i.e. not Off).
fn scope_badge(scope: Scope) -> Option<Span<'static>> {
    let style = Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    match scope {
        Scope::Off => None,
        Scope::Global => Some(Span::styled("· Global", style)),
        Scope::Project => Some(Span::styled("· Project", style)),
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
        let is_editing =
            is_selected && is_value_editable(id) && state.editing.is_some();

        let mut row = vec![
            Span::styled(
                cursor,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{:<22}", label(id)), label_style),
        ];

        if is_editing {
            let buffer = state.editing.as_deref().unwrap_or_default();
            row.push(Span::styled(
                format!("[ {buffer}"),
                Style::default().fg(Color::Yellow),
            ));
            row.push(Span::styled(
                "█",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::DIM),
            ));
            row.push(Span::styled(" ]", Style::default().fg(Color::Yellow)));
        } else {
            row.push(value_chip(state, id));
        }

        if let Some(badge) = scope_badge(state.scope(id)) {
            row.push(Span::raw("  "));
            row.push(badge);
        }

        lines.push(Line::from(row));

        // Helper text reflects the *current* value and updates as it changes.
        let mut helper = state.helper(id);
        if is_editing {
            helper = format!("{helper}  (enter to save, esc to cancel)");
        } else if is_selected && control_kind(id) == Control::Text {
            helper = format!("{helper}  (enter to edit)");
        }

        lines.push(Line::from(vec![
            Span::raw("     "),
            Span::styled(helper, Style::default().add_modifier(Modifier::DIM)),
        ]));

        lines.push(Line::from(""));
    }

    let block = Block::default().borders(Borders::ALL);
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let key = |s: &'static str| {
        Span::styled(
            s,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    };
    let help = Line::from(vec![
        Span::raw(" "),
        key("←/→"),
        Span::raw(":change "),
        key("g"),
        Span::raw(":global/project "),
        key("enter"),
        Span::raw(":edit "),
        key("s"),
        Span::raw(":save "),
        key("q"),
        Span::raw(":quit "),
        key("↑↓"),
        Span::raw(":move"),
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

    fn index_of(id: SettingId) -> usize {
        SETTINGS.iter().position(|&s| s == id).unwrap()
    }

    fn url_index() -> usize {
        index_of(SettingId::AnthropicApiUrl)
    }

    // ---- boolean value + scope ------------------------------------------

    #[test]
    fn telemetry_toggle_on_off() {
        let mut s = default_state();
        let id = SettingId::HeadroomTelemetry;
        assert!(!s.value_on(id));
        assert_eq!(s.scope(id), Scope::Off);

        s.toggle_bool(id);
        assert!(s.value_on(id));
        assert_eq!(s.scope(id), Scope::Global);
        assert!(s.global.headroom_telemetry);

        s.toggle_bool(id);
        assert!(!s.value_on(id));
        assert_eq!(s.scope(id), Scope::Off);
    }

    #[test]
    fn telemetry_scope_toggle_moves_global_project() {
        let mut s = default_state();
        let id = SettingId::HeadroomTelemetry;
        s.toggle_bool(id); // On @ Global
        s.toggle_scope(id);
        assert_eq!(s.scope(id), Scope::Project);
        assert_eq!(s.project.headroom_telemetry, Some(true));

        s.toggle_scope(id);
        assert_eq!(s.scope(id), Scope::Global);
        assert!(s.global.headroom_telemetry);
    }

    #[test]
    fn scope_toggle_noop_when_off() {
        let mut s = default_state();
        let id = SettingId::HeadroomTelemetry;
        s.toggle_scope(id);
        assert_eq!(s.scope(id), Scope::Off);
    }

    #[test]
    fn memory_toggle_on_off() {
        let mut s = default_state();
        let id = SettingId::HeadroomMemory;
        s.toggle_bool(id);
        assert!(s.value_on(id));
        assert!(s.global.headroom_memory);
        s.toggle_bool(id);
        assert!(!s.value_on(id));
        assert!(!s.global.headroom_memory);
    }

    #[test]
    fn tool_search_toggle_on_off() {
        let mut s = default_state();
        let id = SettingId::EnableToolSearch;
        assert!(!s.value_on(id));
        s.toggle_bool(id);
        assert!(s.value_on(id));
        assert!(s.global.enable_tool_search);
        assert_eq!(s.scope(id), Scope::Global);
        s.toggle_bool(id);
        assert!(!s.value_on(id));
        assert!(!s.global.enable_tool_search);
    }

    #[test]
    fn tool_search_scope_toggle_moves_global_project() {
        let mut s = default_state();
        let id = SettingId::EnableToolSearch;
        s.toggle_bool(id); // On @ Global
        s.toggle_scope(id);
        assert_eq!(s.scope(id), Scope::Project);
        assert_eq!(s.project.enable_tool_search, Some(true));

        s.toggle_scope(id);
        assert_eq!(s.scope(id), Scope::Global);
        assert!(s.global.enable_tool_search);
    }

    #[test]
    fn beacon_is_inverted_opt_out() {
        let mut s = default_state();
        let id = SettingId::HeadroomBeacon;
        // Unset => beacon On (Headroom default).
        assert!(s.value_on(id));
        assert_eq!(s.scope(id), Scope::Off);

        // Toggling Off writes HEADROOM_BEACON=off at global scope.
        s.toggle_bool(id);
        assert!(!s.value_on(id));
        assert_eq!(s.scope(id), Scope::Global);
        assert_eq!(
            s.global
                .headroom_env
                .get("HEADROOM_BEACON")
                .map(String::as_str),
            Some("off")
        );

        // Move the opt-out to project scope.
        s.toggle_scope(id);
        assert_eq!(s.scope(id), Scope::Project);
        assert_eq!(
            s.project
                .headroom_env
                .get("HEADROOM_BEACON")
                .map(String::as_str),
            Some("off")
        );

        // Toggling back On removes the key entirely.
        s.toggle_bool(id);
        assert!(s.value_on(id));
        assert!(s.global.headroom_env.is_empty());
        assert!(s.project.headroom_env.is_empty());
    }

    #[test]
    fn log_messages_toggle_writes_fixed_value() {
        let mut s = default_state();
        let id = SettingId::HeadroomLogMessages;
        s.toggle_bool(id);
        assert_eq!(
            s.global
                .headroom_env
                .get("HEADROOM_LOG_MESSAGES")
                .map(String::as_str),
            Some("1")
        );
        s.toggle_scope(id);
        assert_eq!(
            s.project
                .headroom_env
                .get("HEADROOM_LOG_MESSAGES")
                .map(String::as_str),
            Some("1")
        );
        s.toggle_bool(id);
        assert!(s.global.headroom_env.is_empty());
        assert!(s.project.headroom_env.is_empty());
    }

    // ---- choice value + scope -------------------------------------------

    #[test]
    fn model_cycles_off_through_list_and_back() {
        let mut s = default_state();
        let id = SettingId::ApiModel;
        assert_eq!(s.scope(id), Scope::Off);

        s.cycle_choice(id, Dir::Next);
        assert_eq!(s.scope(id), Scope::Global);
        assert_eq!(s.global.api_model.as_deref(), Some(FALLBACK_MODELS[0]));

        s.cycle_choice(id, Dir::Next);
        assert_eq!(s.global.api_model.as_deref(), Some(FALLBACK_MODELS[1]));

        // Advance to the last model, then one more lands on Off.
        for _ in 2..FALLBACK_MODELS.len() {
            s.cycle_choice(id, Dir::Next);
        }
        assert_eq!(
            s.global.api_model.as_deref(),
            Some(FALLBACK_MODELS[FALLBACK_MODELS.len() - 1])
        );
        s.cycle_choice(id, Dir::Next);
        assert_eq!(s.scope(id), Scope::Off);
        assert!(s.global.api_model.is_none());
    }

    #[test]
    fn model_cycle_prev_from_off_selects_last() {
        let mut s = default_state();
        let id = SettingId::ApiModel;
        s.cycle_choice(id, Dir::Prev);
        assert_eq!(
            s.global.api_model.as_deref(),
            Some(FALLBACK_MODELS[FALLBACK_MODELS.len() - 1])
        );
    }

    #[test]
    fn model_scope_toggle_preserves_value() {
        let mut s = default_state();
        let id = SettingId::ApiModel;
        s.cycle_choice(id, Dir::Next); // Global, models[0]
        s.set_choice_value(id, "claude-sonnet-5".to_string());
        s.toggle_scope(id);
        assert_eq!(s.scope(id), Scope::Project);
        assert_eq!(s.project.api_model.as_deref(), Some("claude-sonnet-5"));
    }

    #[test]
    fn edit_mode_cycles_off_through_modes() {
        let mut s = default_state();
        let id = SettingId::PermissionMode;
        assert_eq!(s.scope(id), Scope::Off);

        s.cycle_choice(id, Dir::Next);
        assert_eq!(
            s.global.permission_mode.as_deref(),
            Some(PERMISSION_MODES[0])
        );

        for _ in 1..PERMISSION_MODES.len() {
            s.cycle_choice(id, Dir::Next);
        }
        assert_eq!(
            s.global.permission_mode.as_deref(),
            Some(PERMISSION_MODES[PERMISSION_MODES.len() - 1])
        );
        s.cycle_choice(id, Dir::Next);
        assert_eq!(s.scope(id), Scope::Off);
    }

    #[test]
    fn savings_profile_cycles_off_through_profiles() {
        let mut s = default_state();
        let id = SettingId::HeadroomSavingsProfile;
        assert_eq!(s.scope(id), Scope::Off);

        s.cycle_choice(id, Dir::Next);
        assert_eq!(s.scope(id), Scope::Global);
        assert_eq!(
            s.global.savings_profile.as_deref(),
            Some(SAVINGS_PROFILES[0])
        );

        for _ in 1..SAVINGS_PROFILES.len() {
            s.cycle_choice(id, Dir::Next);
        }
        assert_eq!(
            s.global.savings_profile.as_deref(),
            Some(SAVINGS_PROFILES[SAVINGS_PROFILES.len() - 1])
        );
        s.cycle_choice(id, Dir::Next);
        assert_eq!(s.scope(id), Scope::Off);
        assert!(s.global.savings_profile.is_none());
    }

    #[test]
    fn savings_profile_scope_toggle_preserves_value() {
        let mut s = default_state();
        let id = SettingId::HeadroomSavingsProfile;
        s.cycle_choice(id, Dir::Next); // Global, SAVINGS_PROFILES[0]
        s.set_choice_value(id, "balanced".to_string());
        s.toggle_scope(id);
        assert_eq!(s.scope(id), Scope::Project);
        assert_eq!(s.project.savings_profile.as_deref(), Some("balanced"));
    }

    // ---- text value + scope ---------------------------------------------

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
    fn cycle_value_on_text_opens_editor() {
        let mut s = default_state();
        s.selected = url_index();
        s.global.anthropic_api_url = Some("https://seed.example".into());
        s.cycle_value(SettingId::AnthropicApiUrl, Dir::Next);
        assert_eq!(s.editing.as_deref(), Some("https://seed.example"));
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
    fn commit_edit_writes_env_value_knob() {
        let mut s = default_state();
        s.selected = index_of(SettingId::HeadroomTargetRatio);
        s.editing = Some("0.6".into());
        s.commit_edit();
        assert_eq!(
            s.global
                .headroom_env
                .get("HEADROOM_TARGET_RATIO")
                .map(String::as_str),
            Some("0.6")
        );
    }

    #[test]
    fn rollout_channel_and_output_shaper_write_env_value_knobs() {
        let mut s = default_state();

        s.selected = index_of(SettingId::HeadroomRolloutChannel);
        s.editing = Some("beta".into());
        s.commit_edit();
        assert_eq!(
            s.global
                .headroom_env
                .get("HEADROOM_ROLLOUT_CHANNEL")
                .map(String::as_str),
            Some("beta")
        );

        s.selected = index_of(SettingId::HeadroomOutputShaper);
        s.editing = Some("markdown".into());
        s.commit_edit();
        assert_eq!(
            s.global
                .headroom_env
                .get("HEADROOM_OUTPUT_SHAPER")
                .map(String::as_str),
            Some("markdown")
        );
    }

    #[test]
    fn ratio_scope_toggle_preserves_typed_value() {
        let mut s = default_state();
        let id = SettingId::HeadroomTargetRatio;
        s.global
            .headroom_env
            .insert("HEADROOM_TARGET_RATIO".into(), "0.6".into());
        s.toggle_scope(id);
        assert_eq!(s.scope(id), Scope::Project);
        assert_eq!(
            s.project
                .headroom_env
                .get("HEADROOM_TARGET_RATIO")
                .map(String::as_str),
            Some("0.6")
        );
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

    // ---- display helpers -------------------------------------------------

    #[test]
    fn value_label_reflects_state() {
        let mut s = default_state();
        assert_eq!(s.value_label(SettingId::HeadroomTelemetry), "Off");
        s.toggle_bool(SettingId::HeadroomTelemetry);
        assert_eq!(s.value_label(SettingId::HeadroomTelemetry), "On");

        // Beacon reads On while unset (opt-out default).
        assert_eq!(s.value_label(SettingId::HeadroomBeacon), "On");

        s.global.api_model = Some("claude-sonnet-5".into());
        assert_eq!(s.value_label(SettingId::ApiModel), "claude-sonnet-5");
    }

    #[test]
    fn helper_text_changes_with_value() {
        let mut s = default_state();
        let id = SettingId::HeadroomTelemetry;
        let off = s.helper(id);
        s.toggle_bool(id);
        let on = s.helper(id);
        assert_ne!(off, on);
        assert!(on.starts_with("On"));
        assert!(off.starts_with("Off"));
    }

    #[test]
    fn is_value_editable_only_text_controls() {
        assert!(is_value_editable(SettingId::AnthropicApiUrl));
        assert!(is_value_editable(SettingId::HeadroomTargetRatio));
        assert!(is_value_editable(SettingId::HeadroomBudget));
        assert!(!is_value_editable(SettingId::HeadroomLogMessages));
        assert!(!is_value_editable(SettingId::HeadroomBeacon));
        assert!(!is_value_editable(SettingId::HeadroomTelemetry));
        assert!(!is_value_editable(SettingId::ApiModel));
    }

    // ---- persistence -----------------------------------------------------

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
    fn normalize_saved_strips_blank_env_values() {
        let mut saved = SavedState {
            global: GlobalSettings::default(),
            project: ProjectSettings::default(),
        };
        saved
            .global
            .headroom_env
            .insert("HEADROOM_TARGET_RATIO".into(), String::new());
        saved
            .project
            .headroom_env
            .insert("HEADROOM_BUDGET".into(), "10".into());
        normalize_saved(&mut saved);
        assert!(saved.global.headroom_env.is_empty());
        assert_eq!(
            saved
                .project
                .headroom_env
                .get("HEADROOM_BUDGET")
                .map(String::as_str),
            Some("10")
        );
    }

    #[test]
    fn dirty_detection() {
        let mut s = default_state();
        assert!(!s.dirty());
        s.toggle_bool(SettingId::HeadroomTelemetry);
        assert!(s.dirty());
    }

    // ---- model list helpers ---------------------------------------------

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
    fn newest_sonnet_none_when_absent() {
        let models = vec![
            "claude-opus-4-8".to_string(),
            "claude-haiku-4-5".to_string(),
        ];
        assert_eq!(newest_sonnet(&models), None);
    }

    #[test]
    fn is_current_gen_accepts_latest_families() {
        assert!(is_current_gen("claude-opus-4-8"));
        assert!(is_current_gen("claude-sonnet-4-6"));
        assert!(is_current_gen("claude-haiku-4-5-20251001"));
        assert!(is_current_gen("claude-fable-5"));
    }

    #[test]
    fn is_current_gen_rejects_older_families() {
        assert!(!is_current_gen("claude-3-5-sonnet-20241022"));
        assert!(!is_current_gen("gpt-4o"));
    }

    #[test]
    fn strip_date_suffix_extracts_base() {
        assert_eq!(
            strip_date_suffix("claude-opus-4-8-20260501"),
            Some("claude-opus-4-8")
        );
    }

    #[test]
    fn strip_date_suffix_returns_none_for_short_ids() {
        assert_eq!(strip_date_suffix("claude-opus-4-8"), None);
    }

    #[test]
    fn family_order_sorts_correctly() {
        assert!(
            family_order("claude-opus-4-8") < family_order("claude-sonnet-4-6")
        );
        assert!(
            family_order("claude-sonnet-4-6")
                < family_order("claude-haiku-4-5")
        );
        assert!(
            family_order("claude-haiku-4-5") < family_order("claude-fable-5")
        );
    }
}
