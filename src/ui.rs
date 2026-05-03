use console::style;
use dialoguer::{Confirm, Select};
use std::io::{self, IsTerminal};

pub fn info(msg: &str) {
    eprintln!("{} {msg}", style("[INFO]").blue().bold());
}

pub fn ok(msg: &str) {
    eprintln!("{} {msg}", style("  [OK]").green().bold());
}

pub fn warn(msg: &str) {
    eprintln!("{} {msg}", style("[WARN]").yellow().bold());
}

pub fn fail(msg: &str) -> ! {
    eprintln!("{} {msg}", style("[FAIL]").red().bold());
    std::process::exit(1);
}

pub fn is_interactive() -> bool {
    io::stdin().is_terminal()
}

pub fn confirm(prompt: &str, default: bool) -> bool {
    if !is_interactive() {
        return default;
    }
    Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()
        .unwrap_or(default)
}

pub fn select<T: std::fmt::Display>(prompt: &str, items: &[T], default: usize) -> usize {
    if !is_interactive() {
        return default;
    }
    Select::new()
        .with_prompt(prompt)
        .items(items)
        .default(default)
        .interact()
        .unwrap_or(default)
}
