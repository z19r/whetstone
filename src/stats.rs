use anyhow::{Context, Result};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal, TerminalOptions, Viewport,
};
use serde::Deserialize;
use std::io;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::ui;

const HEADROOM_STATS_URL: &str = "http://127.0.0.1:8787/stats";
const HEADROOM_HEALTH_URL: &str = "http://127.0.0.1:8787/health";
const PROXY_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const PROXY_POLL_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Debug, Deserialize)]
struct HeadroomStats {
    #[serde(default)]
    persistent_savings: PersistentSavings,
    #[serde(default)]
    savings: Savings,
    #[serde(default)]
    tokens: Tokens,
    #[serde(default)]
    cost: Cost,
    #[serde(default)]
    requests: Requests,
}

#[derive(Debug, Default, Deserialize)]
struct PersistentSavings {
    #[serde(default)]
    lifetime: LifetimeSavings,
}

#[derive(Debug, Default, Deserialize)]
struct LifetimeSavings {
    #[serde(default)]
    tokens_saved: u64,
    #[serde(default)]
    compression_savings_usd: f64,
}

#[derive(Debug, Default, Deserialize)]
struct Savings {
    #[serde(default)]
    by_layer: ByLayer,
}

#[derive(Debug, Default, Deserialize)]
struct ByLayer {
    #[serde(default)]
    cli_filtering: Option<CliFiltering>,
    #[serde(default)]
    prefix_cache: Option<PrefixCache>,
}

#[derive(Debug, Deserialize)]
struct CliFiltering {
    #[serde(default)]
    lifetime: Option<RtkLifetime>,
}

#[derive(Debug, Default, Deserialize)]
struct RtkLifetime {
    #[serde(default)]
    commands: u64,
    #[serde(default)]
    tokens_saved: u64,
    #[serde(default)]
    savings_pct: f64,
}

#[derive(Debug, Deserialize)]
struct PrefixCache {
    #[serde(default)]
    discount_usd: f64,
}

#[derive(Debug, Default, Deserialize)]
struct Tokens {
    #[serde(default)]
    saved: u64,
    #[serde(default)]
    savings_percent: f64,
}

#[derive(Debug, Default, Deserialize)]
struct Cost {
    #[serde(default)]
    total_saved_usd: f64,
}

#[derive(Debug, Default, Deserialize)]
struct Requests {
    #[serde(default)]
    total: u64,
    #[serde(default)]
    cached: u64,
}

fn fetch_stats() -> Result<HeadroomStats> {
    let body = ureq::get(HEADROOM_STATS_URL)
        .timeout(std::time::Duration::from_secs(3))
        .call()
        .context("headroom proxy not reachable at localhost:8787")?
        .into_string()
        .context("failed to read headroom stats")?;

    serde_json::from_str(&body).context("failed to parse headroom stats JSON")
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_usd(n: f64) -> String {
    if n >= 1000.0 {
        format!("${:.0}", n)
    } else if n >= 100.0 {
        format!("${:.1}", n)
    } else {
        format!("${:.2}", n)
    }
}

fn stat_line<'a>(label: &'a str, value: String, color: Color) -> Line<'a> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{label:<24}"),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(
            value,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn section_header(title: &str) -> Line<'_> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn proxy_is_running() -> bool {
    ureq::get(HEADROOM_HEALTH_URL)
        .timeout(Duration::from_secs(2))
        .call()
        .is_ok()
}

fn start_temporary_proxy() -> Option<Child> {
    Command::new("headroom")
        .args(["proxy", "--port", "8787"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

fn wait_for_proxy_healthy() -> bool {
    let start = Instant::now();
    while start.elapsed() < PROXY_STARTUP_TIMEOUT {
        if proxy_is_running() {
            return true;
        }
        thread::sleep(PROXY_POLL_INTERVAL);
    }
    false
}

struct TempProxy {
    child: Child,
}

impl TempProxy {
    fn start() -> Option<Self> {
        ui::info("starting headroom proxy temporarily for stats…");
        let child = start_temporary_proxy()?;
        if wait_for_proxy_healthy() {
            Some(Self { child })
        } else {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

impl Drop for TempProxy {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn run() -> Result<()> {
    let _temp_proxy = if proxy_is_running() {
        None
    } else {
        match TempProxy::start() {
            Some(tp) => Some(tp),
            None => {
                ui::warn("headroom proxy not running and failed to start temporarily");
                eprintln!("  start it with: whetstone proxy");
                return Ok(());
            }
        }
    };

    let stats = match fetch_stats() {
        Ok(s) => s,
        Err(e) => {
            ui::warn(&format!("failed to fetch stats: {e:#}"));
            return Ok(());
        }
    };

    let rtk_lifetime = stats
        .savings
        .by_layer
        .cli_filtering
        .as_ref()
        .and_then(|c| c.lifetime.as_ref());

    let cache_usd = stats
        .savings
        .by_layer
        .prefix_cache
        .as_ref()
        .map(|c| c.discount_usd)
        .unwrap_or(0.0);

    let lt = &stats.persistent_savings.lifetime;
    let compression_tokens = lt.tokens_saved;
    let compression_usd = lt.compression_savings_usd;

    let rtk_tokens = rtk_lifetime.map(|r| r.tokens_saved).unwrap_or(0);
    let rtk_commands = rtk_lifetime.map(|r| r.commands).unwrap_or(0);
    let rtk_pct = rtk_lifetime.map(|r| r.savings_pct).unwrap_or(0.0);

    let total_tokens = compression_tokens + rtk_tokens;
    let total_usd = compression_usd + cache_usd;

    let session_tokens = stats.tokens.saved;
    let session_pct = stats.tokens.savings_percent;
    let session_compression_usd = stats.cost.total_saved_usd;

    let cache_hits = stats.requests.cached;
    let cache_rate = if stats.requests.total > 0 {
        (cache_hits as f64 / stats.requests.total as f64) * 100.0
    } else {
        0.0
    };

    if ui::is_interactive() {
        let mut lines: Vec<Line<'_>> = Vec::new();
        lines.push(Line::from(""));

        lines.push(section_header("LIFETIME"));
        lines.push(Line::from(""));
        lines.push(stat_line(
            "Compression",
            format!("{} tokens", format_tokens(compression_tokens)),
            Color::Green,
        ));
        lines.push(stat_line(
            "RTK filtering",
            format!(
                "{} tokens ({:.0}% avg, {} cmds)",
                format_tokens(rtk_tokens),
                rtk_pct,
                rtk_commands
            ),
            Color::Green,
        ));
        lines.push(stat_line(
            "Prefix cache",
            format!("{} saved", format_usd(cache_usd)),
            Color::Green,
        ));
        lines.push(Line::from(""));
        lines.push(stat_line(
            "Total tokens saved",
            format_tokens(total_tokens),
            Color::Yellow,
        ));
        lines.push(stat_line(
            "Total USD saved",
            format_usd(total_usd),
            Color::Yellow,
        ));
        lines.push(Line::from(""));

        lines.push(section_header("THIS SESSION"));
        lines.push(Line::from(""));
        lines.push(stat_line(
            "Tokens saved",
            format!("{} ({:.1}%)", format_tokens(session_tokens), session_pct),
            Color::Blue,
        ));
        lines.push(stat_line(
            "Compression savings",
            format_usd(session_compression_usd),
            Color::Blue,
        ));
        lines.push(stat_line(
            "Cache hit rate",
            format!("{:.0}% ({} hits)", cache_rate, cache_hits),
            Color::Blue,
        ));
        lines.push(stat_line(
            "API requests",
            stats.requests.total.to_string(),
            Color::Blue,
        ));
        lines.push(Line::from(""));

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " whetstone stats ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(Color::DarkGray));

        let paragraph = Paragraph::new(lines.clone()).block(block);
        let height = (lines.len() + 2) as u16;

        let backend = ratatui::backend::CrosstermBackend::new(io::stderr());
        if let Ok(mut terminal) = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        ) {
            let _ = terminal.draw(|frame| {
                frame.render_widget(paragraph, frame.area());
            });
        }
        eprintln!();
    } else {
        eprintln!("whetstone stats");
        eprintln!("{}", "─".repeat(50));
        eprintln!();
        eprintln!("LIFETIME");
        eprintln!(
            "  Compression            {} tokens",
            format_tokens(compression_tokens)
        );
        eprintln!(
            "  RTK filtering          {} tokens ({:.0}% avg, {} cmds)",
            format_tokens(rtk_tokens),
            rtk_pct,
            rtk_commands
        );
        eprintln!("  Prefix cache           {} saved", format_usd(cache_usd));
        eprintln!();
        eprintln!("  Total tokens saved     {}", format_tokens(total_tokens));
        eprintln!("  Total USD saved        {}", format_usd(total_usd));
        eprintln!();
        eprintln!("THIS SESSION");
        eprintln!(
            "  Tokens saved           {} ({:.1}%)",
            format_tokens(session_tokens),
            session_pct
        );
        eprintln!(
            "  Compression savings    {}",
            format_usd(session_compression_usd)
        );
        eprintln!(
            "  Cache hit rate         {:.0}% ({} hits)",
            cache_rate, cache_hits
        );
        eprintln!("  API requests           {}", stats.requests.total);
    }

    Ok(())
}
