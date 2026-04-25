use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ui;

pub fn copy_hook_scripts(assets_hooks: &Path, dest_hooks: &Path) -> Result<()> {
    fs::create_dir_all(dest_hooks)
        .with_context(|| format!("creating {}", dest_hooks.display()))?;

    let scripts = [
        "pre-tool-notify.sh",
        "pre-push.sh",
        "post-commit.sh",
        "session-start.sh",
        "session-end.sh",
    ];

    for script in &scripts {
        let src = assets_hooks.join(script);
        if !src.exists() {
            continue;
        }
        let dst = dest_hooks.join(script);
        fs::copy(&src, &dst)
            .with_context(|| format!("copying {script} to {}", dest_hooks.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
        }
    }

    ui::ok(&format!("copied hook scripts to {}", dest_hooks.display()));
    Ok(())
}

pub fn merge_settings_json(settings_path: &Path, hooks_dir: &Path) -> Result<()> {
    if settings_path.exists() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let backup = settings_path.with_file_name(format!("settings.json.bak.{ts}"));
        fs::copy(settings_path, &backup)
            .with_context(|| format!("backing up {}", settings_path.display()))?;
        ui::ok("backed up existing settings.json");
    }

    let existing: Value = if settings_path.exists() {
        let content = fs::read_to_string(settings_path)
            .with_context(|| format!("reading {}", settings_path.display()))?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    let hd = hooks_dir.display().to_string();
    let merged = build_hooks_value(&existing, &hd);

    let json_str = serde_json::to_string_pretty(&merged)
        .context("serializing settings.json")?;

    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(settings_path, json_str)
        .with_context(|| format!("writing {}", settings_path.display()))?;

    ui::ok("all hooks registered in settings.json");
    Ok(())
}

fn build_hooks_value(existing: &Value, hd: &str) -> Value {
    let mut result = existing.clone();

    result["hooks"] = json!({
        "PreToolUse": [
            {
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": format!("{hd}/rtk-rewrite.sh")
                }]
            },
            {
                "matcher": "Write|Edit|MultiEdit|Bash",
                "hooks": [{
                    "type": "command",
                    "command": format!("{hd}/pre-tool-notify.sh"),
                    "timeout": 10000
                }]
            },
            {
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": format!(
                        "bash -c 'echo \"$CLAUDE_TOOL_INPUT\" | grep -q \"git push\" && {hd}/pre-push.sh || exit 0'"
                    ),
                    "timeout": 60000
                }]
            }
        ],
        "PostToolUse": [
            {
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": format!(
                        "bash -c 'echo \"$CLAUDE_TOOL_INPUT\" | grep -q \"git commit\" && {hd}/post-commit.sh || exit 0'"
                    ),
                    "timeout": 10000
                }]
            }
        ],
        "SessionStart": [{
            "hooks": [{
                "type": "command",
                "command": format!("{hd}/session-start.sh"),
                "timeout": 10000
            }]
        }],
        "Stop": [{
            "hooks": [{
                "type": "command",
                "command": format!("{hd}/session-end.sh"),
                "timeout": 10000
            }]
        }]
    });

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_into_empty_settings() {
        let existing = json!({});
        let result = build_hooks_value(&existing, "/home/user/.claude/hooks");

        let hooks = &result["hooks"];
        assert!(hooks["PreToolUse"].is_array());
        assert_eq!(hooks["PreToolUse"].as_array().unwrap().len(), 3);
        assert_eq!(hooks["PostToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(hooks["SessionStart"].as_array().unwrap().len(), 1);
        assert_eq!(hooks["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn merge_preserves_existing_keys() {
        let existing = json!({
            "apiKey": "sk-test",
            "model": "claude-opus-4-6"
        });
        let result = build_hooks_value(&existing, "/tmp/hooks");

        assert_eq!(result["apiKey"], "sk-test");
        assert_eq!(result["model"], "claude-opus-4-6");
        assert!(result["hooks"].is_object());
    }

    #[test]
    fn hooks_use_absolute_paths() {
        let result = build_hooks_value(&json!({}), "/home/user/.claude/hooks");

        let rtk_cmd = result["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(rtk_cmd.starts_with("/home/user/.claude/hooks/"));
        assert!(rtk_cmd.ends_with("rtk-rewrite.sh"));
    }
}
