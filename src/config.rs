use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct WhetstoneConfig {
    pub version: String,
    pub author: String,
    pub projects: HashMap<String, ProjectEntry>,
    pub headroom: HeadroomConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub dir: String,
    pub claude_md: String,
    pub deploy_target: String,
    pub repo: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeadroomConfig {
    pub auto_start: bool,
    pub port: u16,
    pub health_url: String,
    pub startup_flags: String,
    pub required_extras: Vec<String>,
}

impl WhetstoneConfig {
    pub fn new_for_project(project_dir: &Path, headroom_extras: &str) -> Self {
        let project_name = project_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string());

        let author = Command::new("git")
            .args(["config", "user.name"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "User".to_string());

        let dir_str = project_dir.display().to_string();
        let claude_md = project_dir.join("CLAUDE.md").display().to_string();

        let resolved = crate::headroom::resolve_extras(headroom_extras);
        let extras: Vec<String> = if resolved.is_empty() {
            vec![]
        } else {
            resolved.split(',').map(|s| format!("[{s}]")).collect()
        };

        let mut projects = HashMap::new();
        projects.insert(
            project_name,
            ProjectEntry {
                dir: dir_str,
                claude_md,
                deploy_target: String::new(),
                repo: String::new(),
            },
        );

        Self {
            version: "3.2.3".to_string(),
            author,
            projects,
            headroom: HeadroomConfig {
                auto_start: true,
                port: 8787,
                health_url: "http://127.0.0.1:8787/health".to_string(),
                startup_flags: String::new(),
                required_extras: extras,
            },
        }
    }

    pub fn write_to(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("serializing config.local.json")?;
        fs::write(path, json)
            .with_context(|| format!("writing {}", path.display()))
    }
}
