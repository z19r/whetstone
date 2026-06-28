//! Versioned per-project manifest, written to `.claude/whetstone.json`.
//!
//! Replaces v2's `config.local.json` (which mixed project-list metadata,
//! per-author state, and headroom config). v3's manifest is intentionally
//! minimal: just enough state for `whetstone doctor`, `whetstone update`, and
//! Phase 3's migration layer to know what's installed and when it last ran.
//!
//! Phase 1 task 1.5.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::memory::MemoryProvider;

/// Bump this whenever the manifest schema gains/changes a required field.
/// `whetstone update` uses it to decide when to rewrite the manifest in place.
pub const SCHEMA_VERSION: u32 = 2;

/// Bump this whenever whetstone's *integration contract* changes — e.g. the
/// expected hook layout shifts because we now delegate to a tool's init that
/// emits a different shape. Phase 3 migration consults this to decide whether
/// a fresh `whetstone setup` is needed.
pub const INTEGRATION_VERSION: u32 = 1;

const MANIFEST_FILENAME: &str = "whetstone.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTag {
    Icm,
    Skip,
}

impl From<MemoryProvider> for ProviderTag {
    fn from(p: MemoryProvider) -> Self {
        match p {
            MemoryProvider::Icm => Self::Icm,
            MemoryProvider::Skip => Self::Skip,
        }
    }
}

impl From<ProviderTag> for MemoryProvider {
    fn from(t: ProviderTag) -> Self {
        match t {
            ProviderTag::Icm => Self::Icm,
            ProviderTag::Skip => Self::Skip,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolVersions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headroom: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headroom_telemetry: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headroom_memory: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_model: Option<String>,
}

const GLOBAL_DIR: &str = ".whetstone";
const GLOBAL_SETTINGS_FILENAME: &str = "settings.json";

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalSettings {
    #[serde(default)]
    pub headroom_telemetry: bool,
    #[serde(default)]
    pub headroom_memory: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_model: Option<String>,
}

impl GlobalSettings {
    pub fn path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(GLOBAL_DIR).join(GLOBAL_SETTINGS_FILENAME))
    }

    pub fn load() -> Result<Self> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("could not determine home directory")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let pretty = serde_json::to_string_pretty(self).context("serializing global settings")?;
        fs::write(&path, pretty).with_context(|| format!("writing {}", path.display()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSettings {
    pub headroom_telemetry: bool,
    pub headroom_memory: bool,
    pub api_model: Option<String>,
}

impl ResolvedSettings {
    pub fn resolve(global: &GlobalSettings, project: &ProjectSettings) -> Self {
        Self {
            headroom_telemetry: project
                .headroom_telemetry
                .unwrap_or(global.headroom_telemetry),
            headroom_memory: project.headroom_memory.unwrap_or(global.headroom_memory),
            api_model: project
                .api_model
                .clone()
                .or_else(|| global.api_model.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhetstoneManifest {
    pub schema: u32,
    pub whetstone_version: String,
    pub integration_version: u32,
    pub provider: ProviderTag,
    #[serde(default)]
    pub tool_versions: ToolVersions,
    #[serde(default)]
    pub settings: ProjectSettings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WhetstoneManifest {
    /// Fresh manifest stamped for a brand-new `whetstone setup` run.
    pub fn new(provider: MemoryProvider, tool_versions: ToolVersions) -> Self {
        let now = Utc::now();
        Self {
            schema: SCHEMA_VERSION,
            whetstone_version: crate::version::current().to_string(),
            integration_version: INTEGRATION_VERSION,
            provider: provider.into(),
            tool_versions,
            settings: ProjectSettings::default(),
            migration_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Conventional path: `<project>/.claude/whetstone.json`.
    pub fn path_for(project_dir: &Path) -> PathBuf {
        project_dir.join(".claude").join(MANIFEST_FILENAME)
    }

    /// Load the manifest if it exists. Returns `Ok(None)` for "no manifest
    /// yet" (fresh project), and a real error for a malformed file.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let raw =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let parsed: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parsing manifest at {}", path.display()))?;
        Ok(Some(parsed))
    }

    /// Atomic write: serialize, ensure parent dir exists, write.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let pretty = serde_json::to_string_pretty(self).context("serializing whetstone.json")?;
        fs::write(path, pretty).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Refresh `updated_at` and persist the change.
    pub fn touch_and_save(&mut self, path: &Path) -> Result<()> {
        self.updated_at = Utc::now();
        self.save(path)
    }

    /// Migration id stamped by `whetstone migrate` (Phase 3.6).
    pub fn migration_id(&self) -> Option<&str> {
        self.migration_id.as_deref()
    }

    /// Set the migration id (called by `whetstone migrate` after re-init).
    pub fn set_migration_id(&mut self, id: &str) {
        self.migration_id = Some(id.to_string());
    }

    /// Clear the migration id (called by `--rollback`).
    pub fn clear_migration_id(&mut self) {
        self.migration_id = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn provider_round_trip() {
        let icm: ProviderTag = MemoryProvider::Icm.into();
        assert_eq!(MemoryProvider::from(icm), MemoryProvider::Icm);
        let skip: ProviderTag = MemoryProvider::Skip.into();
        assert_eq!(MemoryProvider::from(skip), MemoryProvider::Skip);
    }

    #[test]
    fn provider_serializes_snake_case() {
        let tag = ProviderTag::Icm;
        let s = serde_json::to_string(&tag).unwrap();
        assert_eq!(s, "\"icm\"");
    }

    #[test]
    fn new_manifest_has_current_schema_and_integration_version() {
        let m = WhetstoneManifest::new(MemoryProvider::Icm, ToolVersions::default());
        assert_eq!(m.schema, SCHEMA_VERSION);
        assert_eq!(m.integration_version, INTEGRATION_VERSION);
        assert_eq!(m.provider, ProviderTag::Icm);
        assert!(m.migration_id.is_none());
    }

    #[test]
    fn save_and_load_round_trip() {
        let m = WhetstoneManifest::new(
            MemoryProvider::Icm,
            ToolVersions {
                rtk: Some("0.42.3".into()),
                icm: Some("0.10.43".into()),
                headroom: Some("0.23.0".into()),
            },
        );
        let f = NamedTempFile::new().unwrap();
        m.save(f.path()).unwrap();
        let loaded = WhetstoneManifest::load(f.path()).unwrap().unwrap();
        assert_eq!(loaded.provider, m.provider);
        assert_eq!(loaded.tool_versions.rtk.as_deref(), Some("0.42.3"));
    }

    #[test]
    fn load_returns_none_when_file_missing() {
        let path = Path::new("/nonexistent-whetstone-test/whetstone.json");
        assert!(WhetstoneManifest::load(path).unwrap().is_none());
    }

    #[test]
    fn load_errors_on_malformed_json() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"not json").unwrap();
        assert!(WhetstoneManifest::load(f.path()).is_err());
    }

    #[test]
    fn path_for_uses_dot_claude() {
        let p = WhetstoneManifest::path_for(Path::new("/tmp/proj"));
        assert!(p.ends_with(".claude/whetstone.json"));
    }

    #[test]
    fn touch_and_save_bumps_updated_at() {
        let mut m = WhetstoneManifest::new(MemoryProvider::Skip, ToolVersions::default());
        let original = m.updated_at;
        let f = NamedTempFile::new().unwrap();
        // Sleep enough to guarantee a chrono-detectable bump on fast machines.
        std::thread::sleep(std::time::Duration::from_millis(10));
        m.touch_and_save(f.path()).unwrap();
        assert!(m.updated_at > original);
    }
}
