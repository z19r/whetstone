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
const PROXY_KILL_TIMEOUT: Duration = Duration::from_secs(10);
const PROXY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const PROXY_PROBE_TIMEOUT: Duration = Duration::from_millis(800);

fn set_proxy_env() {
    if std::env::var("ANTHROPIC_BASE_URL").is_err() {
        std::env::set_var("ANTHROPIC_BASE_URL", DEFAULT_PROXY);
    }
}

pub fn wrap_claude(args: &[String], memory_flag: bool) -> ! {
    set_proxy_env();

    let resolved = resolved_settings();
    let want_memory = memory_flag || resolved.headroom_memory;
    let decision = resolve_proxy(want_memory);

    let skip_rtk_setup = should_skip_headroom_rtk_setup();
    let model = resolve_model(resolved.api_model.clone());
    let cmd_args = build_claude_args(
        args,
        skip_rtk_setup,
        decision.proxy_ready,
        decision.wrap_memory,
        &model,
    );

    exec("headroom", &cmd_args);
}

/// How whetstone should hand the proxy off to `headroom wrap claude`.
struct ProxyDecision {
    /// A live proxy already serves :8787 — pass `--no-proxy` so wrap reuses it.
    proxy_ready: bool,
    /// Let `headroom wrap` own a session-bound proxy started with `--memory`.
    wrap_memory: bool,
}

/// Reconcile the running proxy (if any) with whether this session wants
/// persistent memory. When a proxy is already up *without* memory but memory
/// is wanted, prompt the user before replacing it.
fn resolve_proxy(want_memory: bool) -> ProxyDecision {
    match probe_proxy_health() {
        Some(health) => {
            if want_memory && !health.memory {
                resolve_memory_conflict(want_memory, health.pid)
            } else {
                // A live proxy already satisfies this session.
                ProxyDecision {
                    proxy_ready: true,
                    wrap_memory: false,
                }
            }
        }
        None => start_detached_decision(want_memory),
    }
}

/// Spawn a detached proxy (optionally with memory) and wait for it to answer.
/// If it never comes up, fall back to letting `headroom wrap` start its own
/// session-bound proxy — carrying the memory preference across.
fn start_detached_decision(want_memory: bool) -> ProxyDecision {
    let ready = start_proxy_detached_ready(want_memory);
    ProxyDecision {
        proxy_ready: ready,
        wrap_memory: if ready { false } else { want_memory },
    }
}

/// The running proxy lacks memory but this session wants it. Ask what to do.
fn resolve_memory_conflict(want_memory: bool, pid: Option<u32>) -> ProxyDecision {
    // Non-interactive: never kill a proxy other sessions may be sharing.
    if !crate::ui::is_interactive() {
        eprintln!(
            "[WARN] whetstone: a proxy is already running without --memory; \
             continuing without memory (run interactively to replace it)"
        );
        return ProxyDecision {
            proxy_ready: true,
            wrap_memory: false,
        };
    }

    let choices = [
        "Restart the proxy with memory (replaces it for all sessions)",
        "Start a memory proxy for this session only",
        "Cancel and abort launch",
    ];
    let prompt = "A Headroom proxy is already running without --memory. What now?";
    match crate::ui::select(prompt, &choices, 0) {
        0 => {
            kill_proxy(pid);
            start_detached_decision(want_memory)
        }
        1 => {
            kill_proxy(pid);
            // `headroom wrap claude --memory` brings up its own session proxy.
            ProxyDecision {
                proxy_ready: false,
                wrap_memory: true,
            }
        }
        _ => {
            crate::ui::info("aborted; proxy left untouched");
            std::process::exit(0);
        }
    }
}

/// SIGTERM the running proxy by PID and wait for the port to free up, so a
/// replacement can bind :8787 without an [Errno 98] address-in-use race.
fn kill_proxy(pid: Option<u32>) {
    match pid {
        Some(pid) => {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
        None => {
            eprintln!("[WARN] whetstone: running proxy did not report a pid; cannot kill it");
            return;
        }
    }

    let deadline = Instant::now() + PROXY_KILL_TIMEOUT;
    while Instant::now() < deadline {
        if !probe_proxy() {
            return;
        }
        std::thread::sleep(PROXY_POLL_INTERVAL);
    }
    eprintln!("[WARN] whetstone: proxy at {DEFAULT_PROXY} did not shut down in time");
}

fn resolved_settings() -> crate::config::ResolvedSettings {
    let global = crate::config::GlobalSettings::load().unwrap_or_default();
    let cwd = env::current_dir().ok();
    let project = cwd
        .map(|d| crate::config::WhetstoneManifest::path_for(&d))
        .and_then(|p| crate::config::WhetstoneManifest::load(&p).ok().flatten())
        .map(|m| m.settings)
        .unwrap_or_default();
    crate::config::ResolvedSettings::resolve(&global, &project)
}

/// Phase 6.3: claude's first API call must hit a live proxy. The SessionStart
/// hook auto-starts headroom, but it fires after claude has already launched —
/// so the proxy may still be down by the time claude makes its first request.
/// Spawn `headroom proxy` (optionally with `--memory`) detached and poll until
/// it answers or we hit `PROXY_READY_TIMEOUT`. Returns whether a proxy is up,
/// so the caller can pass `--no-proxy` to `headroom wrap` — whetstone owns the
/// proxy lifecycle, and letting wrap manage it risks a hot-restart crash. On
/// failure we soft-warn and return false so wrap may still try its own proxy
/// startup as a last-resort fallback (e.g. a custom upstream).
fn start_proxy_detached_ready(memory: bool) -> bool {
    let spawned = spawn_proxy_detached(memory).is_ok();

    let deadline = Instant::now() + PROXY_READY_TIMEOUT;
    while Instant::now() < deadline {
        if probe_proxy() {
            return true;
        }
        std::thread::sleep(PROXY_POLL_INTERVAL);
    }

    let tail = if spawned {
        "(spawned a background headroom proxy, but it did not respond in time)"
    } else {
        "(could not spawn `headroom proxy` — is headroom installed?)"
    };
    eprintln!("[WARN] whetstone: proxy at {DEFAULT_PROXY} is not responding {tail}");
    false
}

fn probe_proxy() -> bool {
    ureq::get(PROXY_HEALTH_URL)
        .timeout(PROXY_PROBE_TIMEOUT)
        .call()
        .is_ok()
}

/// A live proxy's relevant state, read from `/health` `config`.
struct ProxyHealth {
    /// Whether the running proxy was started with persistent memory enabled.
    memory: bool,
    /// The proxy's process id, used to replace it cleanly. `None` if absent.
    pid: Option<u32>,
}

/// Probe `/health` and parse memory state + pid. `None` when no proxy answers.
fn probe_proxy_health() -> Option<ProxyHealth> {
    let body = ureq::get(PROXY_HEALTH_URL)
        .timeout(PROXY_PROBE_TIMEOUT)
        .call()
        .ok()?
        .into_string()
        .ok()?;
    parse_proxy_health(&body)
}

fn parse_proxy_health(body: &str) -> Option<ProxyHealth> {
    let json: Value = serde_json::from_str(body).ok()?;
    let config = json.get("config");
    let memory = config
        .and_then(|c| c.get("memory"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let pid = config
        .and_then(|c| c.get("pid"))
        .and_then(Value::as_u64)
        .map(|p| p as u32);
    Some(ProxyHealth { memory, pid })
}

fn spawn_proxy_detached(memory: bool) -> std::io::Result<()> {
    use std::process::Stdio;
    let telemetry_disabled = !headroom_telemetry_enabled();
    let args = build_proxy_args(
        headroom_proxy_supports_savings_profile(),
        telemetry_disabled,
        memory,
    );
    let mut cmd = Command::new("headroom");
    cmd.args(&args)
        .env("HEADROOM_SAVINGS_PROFILE", required_savings_profile())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if telemetry_disabled {
        cmd.env("HEADROOM_TELEMETRY", "off");
    }
    cmd.spawn().map(|_| ())
}

fn build_proxy_args(savings_profile: bool, no_telemetry: bool, memory: bool) -> Vec<&'static str> {
    let mut args = vec!["proxy", "--port", "8787"];
    if savings_profile {
        args.push("--savings-profile");
    }
    if no_telemetry {
        args.push("--no-telemetry");
    }
    if memory {
        args.push("--memory");
    }
    args
}

fn headroom_telemetry_enabled() -> bool {
    resolved_settings().headroom_telemetry
}

// The savings profile `headroom wrap claude` requires (headroom
// `cli/wrap.py` `_DEFAULT_AGENT_SAVINGS_PROFILE`). The proxy must report this
// profile via `/health`, or `headroom wrap claude` decides it is "missing:
// --savings-profile", hot-restarts it, and crashes binding the port before
// the old proxy releases it ([Errno 98]). The proxy reads the profile from
// the `HEADROOM_SAVINGS_PROFILE` env var, not a CLI flag, in current headroom.
const DEFAULT_SAVINGS_PROFILE: &str = "agent-90";

fn required_savings_profile() -> String {
    resolve_savings_profile(env::var("HEADROOM_SAVINGS_PROFILE").ok())
}

// Honor an existing override (matching what `headroom wrap claude` will read),
// falling back to headroom's default so a whetstone-spawned proxy always
// matches the profile wrap demands.
fn resolve_savings_profile(override_value: Option<String>) -> String {
    override_value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_SAVINGS_PROFILE.to_string())
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

// Pinned fallback model — used only when the user hasn't selected one AND we
// can't reach the models API to detect a newer Sonnet (offline / no
// ANTHROPIC_API_KEY). If the user (or a wrapping CLI layer) already passes
// `--model`, we leave it alone.
const DEFAULT_MODEL: &str = "claude-opus-4-6";

// Resolve the model to launch with, in priority order:
//   1. an explicit selection stored in whetstone settings (`api_model`)
//   2. the newest available Sonnet, per the 12h-cached models API
//   3. the pinned `DEFAULT_MODEL` fallback
fn resolve_model(explicit: Option<String>) -> String {
    explicit
        .or_else(crate::settings::preferred_default_model)
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

fn build_claude_args(
    args: &[String],
    skip_rtk_setup: bool,
    proxy_ready: bool,
    wrap_memory: bool,
    model: &str,
) -> Vec<String> {
    let mut cmd_args = vec!["wrap".to_string(), "claude".to_string()];

    if proxy_ready {
        cmd_args.push("--no-proxy".into());
    }

    if skip_rtk_setup {
        cmd_args.push("--no-rtk".into());
    }

    cmd_args.push("--no-serena".into());

    // Only when whetstone is *not* managing the proxy itself — wrap brings up
    // its own session-bound proxy and needs `--memory` to enable it there.
    if wrap_memory {
        cmd_args.push("--memory".into());
    }

    let user_set_model = args
        .iter()
        .any(|a| a == "--model" || a.starts_with("--model="));
    if !user_set_model {
        cmd_args.push("--model".into());
        cmd_args.push(model.into());
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

pub fn wrap_proxy(args: &[String], memory: bool) -> ! {
    set_proxy_env();
    let mut proxy_args = vec!["proxy".to_string()];
    if memory && !args.iter().any(|a| a == "--memory") {
        proxy_args.push("--memory".into());
    }
    proxy_args.extend_from_slice(args);
    exec("headroom", &proxy_args);
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

    #[test]
    fn resolve_model_prefers_explicit_selection() {
        assert_eq!(
            resolve_model(Some("claude-sonnet-5".to_string())),
            "claude-sonnet-5"
        );
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
        let args = build_claude_args(&[], true, false, false, DEFAULT_MODEL);

        assert_eq!(
            args,
            strings(&[
                "wrap",
                "claude",
                "--no-rtk",
                "--no-serena",
                "--model",
                DEFAULT_MODEL,
            ])
        );
    }

    #[test]
    fn build_claude_args_preserves_explicit_model() {
        let args = build_claude_args(
            &["--model".into(), "claude-sonnet".into()],
            false,
            false,
            false,
            DEFAULT_MODEL,
        );

        assert_eq!(
            args,
            strings(&["wrap", "claude", "--no-serena", "--model", "claude-sonnet"])
        );
        assert_eq!(args.iter().filter(|a| a.as_str() == "--model").count(), 1);
    }

    #[test]
    fn build_claude_args_preserves_explicit_model_equals_form() {
        let args = build_claude_args(
            &["--model=claude-sonnet".into()],
            false,
            false,
            false,
            DEFAULT_MODEL,
        );

        assert_eq!(
            args,
            strings(&["wrap", "claude", "--no-serena", "--model=claude-sonnet"])
        );
    }

    #[test]
    fn build_claude_args_passes_through_arbitrary_args_unchanged() {
        let args = build_claude_args(
            &["--dangerously-skip-permissions".into(), "--print".into()],
            false,
            false,
            false,
            DEFAULT_MODEL,
        );

        assert_eq!(
            args,
            strings(&[
                "wrap",
                "claude",
                "--no-serena",
                "--model",
                DEFAULT_MODEL,
                "--dangerously-skip-permissions",
                "--print",
            ])
        );
    }

    #[test]
    fn build_claude_args_uses_configured_model() {
        let args = build_claude_args(&[], false, false, false, "claude-sonnet-4-6");

        assert_eq!(
            args,
            strings(&[
                "wrap",
                "claude",
                "--no-serena",
                "--model",
                "claude-sonnet-4-6"
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
        let args = build_proxy_args(false, false, false);
        assert_eq!(args, vec!["proxy", "--port", "8787"]);
    }

    #[test]
    fn build_proxy_args_with_savings_profile() {
        let args = build_proxy_args(true, false, false);
        assert_eq!(args, vec!["proxy", "--port", "8787", "--savings-profile"]);
    }

    #[test]
    fn build_proxy_args_with_no_telemetry() {
        let args = build_proxy_args(false, true, false);
        assert_eq!(args, vec!["proxy", "--port", "8787", "--no-telemetry"]);
    }

    #[test]
    fn build_proxy_args_with_memory() {
        let args = build_proxy_args(false, false, true);
        assert_eq!(args, vec!["proxy", "--port", "8787", "--memory"]);
    }

    #[test]
    fn build_proxy_args_with_savings_profile_and_memory() {
        let args = build_proxy_args(true, false, true);
        assert_eq!(
            args,
            vec!["proxy", "--port", "8787", "--savings-profile", "--memory"]
        );
    }

    #[test]
    fn build_claude_args_injects_memory_when_wrap_manages_proxy() {
        let args = build_claude_args(&[], false, false, true, DEFAULT_MODEL);
        assert!(args.contains(&"--memory".to_string()));
        // wrap owns the proxy here, so --no-proxy must NOT be present.
        assert!(!args.contains(&"--no-proxy".to_string()));
    }

    #[test]
    fn build_claude_args_omits_memory_by_default() {
        let args = build_claude_args(&[], false, true, false, DEFAULT_MODEL);
        assert!(!args.contains(&"--memory".to_string()));
    }

    #[test]
    fn parse_proxy_health_reads_memory_and_pid() {
        let body = r#"{"config":{"memory":true,"pid":4242}}"#;
        let health = parse_proxy_health(body).unwrap();
        assert!(health.memory);
        assert_eq!(health.pid, Some(4242));
    }

    #[test]
    fn parse_proxy_health_defaults_memory_false_when_absent() {
        let body = r#"{"config":{"pid":7}}"#;
        let health = parse_proxy_health(body).unwrap();
        assert!(!health.memory);
        assert_eq!(health.pid, Some(7));
    }

    #[test]
    fn parse_proxy_health_handles_missing_config() {
        let body = r#"{"status":"healthy"}"#;
        let health = parse_proxy_health(body).unwrap();
        assert!(!health.memory);
        assert_eq!(health.pid, None);
    }

    #[test]
    fn parse_proxy_health_rejects_garbage() {
        assert!(parse_proxy_health("not json").is_none());
    }

    #[test]
    fn build_proxy_args_with_savings_profile_and_no_telemetry() {
        let args = build_proxy_args(true, true, false);
        assert_eq!(
            args,
            vec![
                "proxy",
                "--port",
                "8787",
                "--savings-profile",
                "--no-telemetry"
            ]
        );
    }

    #[test]
    fn resolve_savings_profile_defaults_when_unset() {
        assert_eq!(resolve_savings_profile(None), DEFAULT_SAVINGS_PROFILE);
    }

    #[test]
    fn resolve_savings_profile_defaults_when_empty() {
        assert_eq!(
            resolve_savings_profile(Some(String::new())),
            DEFAULT_SAVINGS_PROFILE
        );
    }

    #[test]
    fn resolve_savings_profile_honors_override() {
        assert_eq!(
            resolve_savings_profile(Some("agent-50".to_string())),
            "agent-50"
        );
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
        let args = build_claude_args(&[], false, false, false, DEFAULT_MODEL);
        assert!(!args.contains(&"--no-rtk".to_string()));
        assert!(args.contains(&"--no-serena".to_string()));
        assert!(args.contains(&"--model".to_string()));
    }

    #[test]
    fn build_claude_args_adds_no_proxy_when_proxy_ready() {
        let args = build_claude_args(&[], false, true, false, DEFAULT_MODEL);
        assert!(args.contains(&"--no-proxy".to_string()));
        assert_eq!(&args[0..2], &["wrap".to_string(), "claude".to_string()]);
    }

    #[test]
    fn build_claude_args_omits_no_proxy_when_proxy_not_ready() {
        let args = build_claude_args(&[], false, false, false, DEFAULT_MODEL);
        assert!(!args.contains(&"--no-proxy".to_string()));
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
