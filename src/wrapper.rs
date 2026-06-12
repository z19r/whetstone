use serde_json::Value;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const DEFAULT_PROXY: &str = "http://127.0.0.1:8787";
const PROXY_HEALTH_URL: &str = "http://127.0.0.1:8787/health";
const PROXY_READY_TIMEOUT: Duration = Duration::from_secs(15);
const PROXY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const PROXY_PROBE_TIMEOUT: Duration = Duration::from_millis(800);

fn set_proxy_env() {
    if std::env::var("ANTHROPIC_BASE_URL").is_err() {
        std::env::set_var("ANTHROPIC_BASE_URL", DEFAULT_PROXY);
    }
}

pub fn wrap_claude(args: &[String]) -> ! {
    set_proxy_env();
    ensure_proxy_ready();

    let skip_rtk_setup = should_skip_headroom_rtk_setup();
    let cmd_args = build_claude_args(args, skip_rtk_setup);

    exec("headroom", &cmd_args);
}

/// Phase 6.3: claude's first API call must hit a live proxy. The SessionStart
/// hook auto-starts headroom, but it fires after claude has already launched —
/// so the proxy may still be down by the time claude makes its first request.
/// Probe `/health`; if dead, spawn `headroom proxy` detached and poll until it
/// answers or we hit `PROXY_READY_TIMEOUT`. Soft-fail with a warning rather
/// than refusing to exec — if the user has a custom upstream, headroom wrap's
/// own logic should still be allowed to run.
fn ensure_proxy_ready() {
    if probe_proxy() {
        return;
    }

    let spawned = spawn_proxy_detached().is_ok();

    let deadline = Instant::now() + PROXY_READY_TIMEOUT;
    while Instant::now() < deadline {
        if probe_proxy() {
            return;
        }
        std::thread::sleep(PROXY_POLL_INTERVAL);
    }

    let tail = if spawned {
        "(spawned a background headroom proxy, but it did not respond in time)"
    } else {
        "(could not spawn `headroom proxy` — is headroom installed?)"
    };
    eprintln!("[WARN] whetstone: proxy at {DEFAULT_PROXY} is not responding {tail}");
}

fn probe_proxy() -> bool {
    ureq::get(PROXY_HEALTH_URL)
        .timeout(PROXY_PROBE_TIMEOUT)
        .call()
        .is_ok()
}

fn spawn_proxy_detached() -> std::io::Result<()> {
    use std::process::Stdio;
    let args = build_proxy_args(headroom_proxy_supports_savings_profile());
    Command::new("headroom")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

fn build_proxy_args(savings_profile: bool) -> Vec<&'static str> {
    let mut args = vec!["proxy", "--port", "8787"];
    if savings_profile {
        args.push("--savings-profile");
    }
    args
}

fn headroom_proxy_supports_savings_profile() -> bool {
    let output = Command::new("headroom")
        .args(["proxy", "--help"])
        .output()
        .ok();

    let Some(output) = output else {
        return false;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    proxy_help_mentions_flag(&stdout, "--savings-profile")
        || proxy_help_mentions_flag(&stderr, "--savings-profile")
}

fn proxy_help_mentions_flag(help_text: &str, flag: &str) -> bool {
    help_text.contains(flag)
}

// Default model — whetstone pins claude-opus-4-6 until it is fully deprecated.
// If the user (or a wrapping CLI layer) already passes `--model`, we leave it
// alone.
const DEFAULT_MODEL: &str = "claude-opus-4-6";

fn build_claude_args(args: &[String], skip_rtk_setup: bool) -> Vec<String> {
    let mut cmd_args = vec!["wrap".to_string(), "claude".to_string()];

    if skip_rtk_setup {
        cmd_args.push("--no-rtk".into());
    }

    let user_set_model = args
        .iter()
        .any(|a| a == "--model" || a.starts_with("--model="));
    if !user_set_model {
        cmd_args.push("--model".into());
        cmd_args.push(DEFAULT_MODEL.into());
    }

    cmd_args.extend_from_slice(args);
    cmd_args
}

fn should_skip_headroom_rtk_setup() -> bool {
    global_headroom_rtk_hook_exists()
}

fn rtk_exists_on_path(path_var: Option<&OsStr>) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };

    env::split_paths(path_var).any(path_has_rtk_binary)
}

fn path_has_rtk_binary(dir: PathBuf) -> bool {
    let binary = dir.join(rtk_binary_name());
    binary.is_file() && path_is_executable(&binary)
}

fn rtk_binary_name() -> &'static str {
    if cfg!(windows) {
        "rtk.exe"
    } else {
        "rtk"
    }
}

fn global_headroom_rtk_hook_exists() -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let settings_path = home.join(".claude/settings.json");
    let Ok(contents) = fs::read_to_string(settings_path) else {
        return false;
    };
    let Ok(settings) = serde_json::from_str::<Value>(&contents) else {
        return false;
    };

    settings_has_headroom_rtk_hook(&settings)
}

fn settings_has_headroom_rtk_hook(settings: &Value) -> bool {
    settings_has_headroom_rtk_hook_for(settings, env::var_os("PATH").as_deref())
}

fn settings_has_headroom_rtk_hook_for(settings: &Value, path_var: Option<&OsStr>) -> bool {
    settings
        .get("hooks")
        .and_then(|hooks| hooks.get("PreToolUse"))
        .and_then(Value::as_array)
        .is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry_has_headroom_rtk_hook(entry, path_var))
        })
}

fn entry_has_headroom_rtk_hook(entry: &Value, path_var: Option<&OsStr>) -> bool {
    matcher_allows_bash(entry)
        && entry
            .get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|hooks| {
                hooks
                    .iter()
                    .any(|hook| command_is_headroom_rtk_hook(hook, path_var))
            })
}

fn matcher_allows_bash(entry: &Value) -> bool {
    entry
        .get("matcher")
        .and_then(Value::as_str)
        .is_some_and(|matcher| matcher.contains("Bash"))
}

fn command_is_headroom_rtk_hook(hook: &Value, path_var: Option<&OsStr>) -> bool {
    hook.get("type").and_then(Value::as_str) == Some("command")
        && hook
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| rtk_hook_command_is_usable(command, path_var))
}

fn rtk_hook_command_is_usable(command: &str, path_var: Option<&OsStr>) -> bool {
    let Some(program) = rtk_hook_program(command) else {
        return false;
    };

    if !program_ends_with_rtk(program) {
        return false;
    }

    if is_bare_rtk_program(program) {
        return rtk_exists_on_path(path_var);
    }

    let path = Path::new(program);
    path.is_file() && path_is_executable(path)
}

fn rtk_hook_program(command: &str) -> Option<&str> {
    let program = command.strip_suffix(" hook claude")?.trim();
    Some(program.trim_matches('"'))
}

fn program_ends_with_rtk(program: &str) -> bool {
    let path = Path::new(program);
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == rtk_binary_name())
}

fn is_bare_rtk_program(program: &str) -> bool {
    program == rtk_binary_name()
}

#[cfg(unix)]
fn path_is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn path_is_executable(path: &Path) -> bool {
    path.is_file()
}

pub fn wrap_proxy(args: &[String]) -> ! {
    set_proxy_env();
    exec("headroom", &[&["proxy".to_string()], args].concat());
}

pub fn wrap_rtk(args: &[String]) -> ! {
    set_proxy_env();
    exec("rtk", args);
}

#[cfg(unix)]
fn exec(program: &str, args: &[String]) -> ! {
    use std::os::unix::process::CommandExt;
    let err = Command::new(program).args(args).exec();
    eprintln!("[FAIL] failed to exec {program}: {err}");
    std::process::exit(127);
}

#[cfg(not(unix))]
fn exec(program: &str, args: &[String]) -> ! {
    let status = Command::new(program)
        .args(args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("[FAIL] failed to run {program}: {e}");
            std::process::exit(127);
        });
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn create_fake_rtk_binary() -> (PathBuf, PathBuf) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("whetstone-rtk-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let binary = dir.join(rtk_binary_name());
        fs::write(
            &binary,
            "#!/bin/sh
exit 0
",
        )
        .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = fs::metadata(&binary).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary, perms).unwrap();
        }

        (dir, binary)
    }

    fn cleanup_fake_rtk(dir: &Path, binary: &Path) {
        let _ = fs::remove_file(binary);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn build_claude_args_injects_default_model_when_absent() {
        let args = build_claude_args(&[], true);

        assert_eq!(
            args,
            strings(&["wrap", "claude", "--no-rtk", "--model", DEFAULT_MODEL])
        );
    }

    #[test]
    fn build_claude_args_preserves_explicit_model() {
        let args = build_claude_args(&["--model".into(), "claude-sonnet".into()], false);

        // Explicit user --model wins; we do NOT add our default on top.
        assert_eq!(
            args,
            strings(&["wrap", "claude", "--model", "claude-sonnet"])
        );
        assert_eq!(args.iter().filter(|a| a.as_str() == "--model").count(), 1);
    }

    #[test]
    fn build_claude_args_preserves_explicit_model_equals_form() {
        let args = build_claude_args(&["--model=claude-sonnet".into()], false);

        assert_eq!(args, strings(&["wrap", "claude", "--model=claude-sonnet"]));
    }

    #[test]
    fn build_claude_args_passes_through_arbitrary_args_unchanged() {
        let args = build_claude_args(
            &["--dangerously-skip-permissions".into(), "--print".into()],
            false,
        );

        // Default --model goes ahead of pass-through args.
        assert_eq!(
            args,
            strings(&[
                "wrap",
                "claude",
                "--model",
                DEFAULT_MODEL,
                "--dangerously-skip-permissions",
                "--print",
            ])
        );
    }

    #[test]
    fn settings_hook_detection_matches_headroom_hook() {
        let (dir, binary) = create_fake_rtk_binary();
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "rtk hook claude"
                    }]
                }]
            }
        });

        assert!(settings_has_headroom_rtk_hook_for(
            &settings,
            Some(dir.as_os_str()),
        ));
        cleanup_fake_rtk(&dir, &binary);
    }

    #[test]
    fn settings_hook_detection_matches_absolute_rtk_hook_path() {
        let (dir, binary) = create_fake_rtk_binary();
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": format!("{} hook claude", binary.display())
                    }]
                }]
            }
        });

        assert!(settings_has_headroom_rtk_hook(&settings));
        cleanup_fake_rtk(&dir, &binary);
    }

    #[test]
    fn settings_hook_detection_rejects_missing_absolute_rtk_hook() {
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/tmp/missing/rtk hook claude"
                    }]
                }]
            }
        });

        assert!(!settings_has_headroom_rtk_hook(&settings));
    }

    #[test]
    fn settings_hook_detection_rejects_non_rtk_hook() {
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/tmp/rtk-rewrite.sh"
                    }]
                }]
            }
        });

        assert!(!settings_has_headroom_rtk_hook(&settings));
    }

    #[test]
    fn path_detection_finds_executable_rtk() {
        let (dir, binary) = create_fake_rtk_binary();

        assert!(path_has_rtk_binary(dir.clone()));
        cleanup_fake_rtk(&dir, &binary);
    }

    #[test]
    fn build_proxy_args_without_savings_profile() {
        let args = build_proxy_args(false);
        assert_eq!(args, vec!["proxy", "--port", "8787"]);
    }

    #[test]
    fn build_proxy_args_with_savings_profile() {
        let args = build_proxy_args(true);
        assert_eq!(args, vec!["proxy", "--port", "8787", "--savings-profile"]);
    }

    #[test]
    fn proxy_help_mentions_flag_finds_present_flag() {
        let help = "  --port INTEGER  Port to listen on\n  --savings-profile  Enable savings\n";
        assert!(proxy_help_mentions_flag(help, "--savings-profile"));
    }

    #[test]
    fn proxy_help_mentions_flag_rejects_absent_flag() {
        let help = "  --port INTEGER  Port to listen on\n  --verbose  Verbose output\n";
        assert!(!proxy_help_mentions_flag(help, "--savings-profile"));
    }

    #[test]
    fn proxy_help_mentions_flag_empty_text() {
        assert!(!proxy_help_mentions_flag("", "--savings-profile"));
    }

    #[test]
    fn rtk_hook_program_extracts_bare_name() {
        assert_eq!(rtk_hook_program("rtk hook claude"), Some("rtk"));
    }

    #[test]
    fn rtk_hook_program_extracts_absolute_path() {
        assert_eq!(
            rtk_hook_program("/usr/local/bin/rtk hook claude"),
            Some("/usr/local/bin/rtk")
        );
    }

    #[test]
    fn rtk_hook_program_strips_quotes() {
        assert_eq!(rtk_hook_program("\"rtk\" hook claude"), Some("rtk"));
    }

    #[test]
    fn rtk_hook_program_rejects_non_hook_command() {
        assert_eq!(rtk_hook_program("rtk gain"), None);
        assert_eq!(rtk_hook_program("echo hello"), None);
    }

    #[test]
    fn program_ends_with_rtk_matches_bare_and_pathed() {
        assert!(program_ends_with_rtk("rtk"));
        assert!(program_ends_with_rtk("/usr/local/bin/rtk"));
        assert!(program_ends_with_rtk("/home/user/.local/bin/rtk"));
    }

    #[test]
    fn program_ends_with_rtk_rejects_non_rtk() {
        assert!(!program_ends_with_rtk("not-rtk"));
        assert!(!program_ends_with_rtk("/usr/bin/rtkx"));
        assert!(!program_ends_with_rtk("rtk-rewrite.sh"));
    }

    #[test]
    fn is_bare_rtk_program_only_matches_bare() {
        assert!(is_bare_rtk_program("rtk"));
        assert!(!is_bare_rtk_program("/usr/bin/rtk"));
        assert!(!is_bare_rtk_program("./rtk"));
    }

    #[test]
    fn build_claude_args_skips_no_rtk_when_not_requested() {
        let args = build_claude_args(&[], false);
        assert!(!args.contains(&"--no-rtk".to_string()));
        assert!(args.contains(&"--model".to_string()));
    }

    #[test]
    fn rtk_exists_on_path_returns_false_for_none() {
        assert!(!rtk_exists_on_path(None));
    }

    #[test]
    fn rtk_exists_on_path_returns_false_for_empty_path() {
        assert!(!rtk_exists_on_path(Some(std::ffi::OsStr::new(""))));
    }

    #[test]
    fn rtk_exists_on_path_finds_in_path_var() {
        let (dir, binary) = create_fake_rtk_binary();
        let path_str = dir.to_str().unwrap();
        assert!(rtk_exists_on_path(Some(std::ffi::OsStr::new(path_str))));
        cleanup_fake_rtk(&dir, &binary);
    }

    #[test]
    fn settings_hook_detection_handles_empty_settings() {
        let settings = json!({});
        assert!(!settings_has_headroom_rtk_hook(&settings));
    }

    #[test]
    fn settings_hook_detection_handles_empty_hooks() {
        let settings = json!({ "hooks": {} });
        assert!(!settings_has_headroom_rtk_hook(&settings));
    }

    #[test]
    fn settings_hook_detection_handles_empty_pre_tool_use() {
        let settings = json!({ "hooks": { "PreToolUse": [] } });
        assert!(!settings_has_headroom_rtk_hook(&settings));
    }
}
