use std::process::Command;

const DEFAULT_PROXY: &str = "http://127.0.0.1:8787";
const DEFAULT_MODEL: &str = "claude-opus-4-6";

fn set_proxy_env() {
    if std::env::var("ANTHROPIC_BASE_URL").is_err() {
        std::env::set_var("ANTHROPIC_BASE_URL", DEFAULT_PROXY);
    }
}

fn has_model_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "--model" || a.starts_with("--model="))
}

pub fn wrap_claude(args: &[String]) -> ! {
    set_proxy_env();

    let mut cmd_args = vec!["wrap".to_string(), "claude".to_string()];
    if !has_model_flag(args) {
        cmd_args.push("--model".into());
        cmd_args.push(DEFAULT_MODEL.into());
    }
    cmd_args.extend_from_slice(args);

    exec("headroom", &cmd_args);
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
