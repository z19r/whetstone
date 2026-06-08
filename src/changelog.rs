//! Parser for the repo-root `CHANGELOG.md`.
//!
//! Produces a flat, JSON-friendly list of recent releases that the marketing
//! site renders. The shape is intentionally minimal: each entry carries a
//! version, date, and a handful of one-line bullets bucketed by Keep-A-Changelog
//! section (Breaking / Added / Changed / Fixed / Removed). The `[Unreleased]`
//! section is skipped.
//!
//! Bullets are aggressively shortened — the site is meant to skim, not to read.
//! Long markdown bullets are truncated at the first sentence boundary and capped
//! at `BULLETS_PER_SECTION` items per section. Markdown emphasis (`**bold**`,
//! `` `code` ``) is stripped to plain text so the JSX layer doesn't need a
//! renderer.

use anyhow::{Context, Result};
use std::path::Path;

const BULLETS_PER_SECTION: usize = 4;
const MAX_BULLET_CHARS: usize = 140;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Section {
    pub name: String,
    pub bullets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Entry {
    pub ver: String,
    pub date: String,
    pub sections: Vec<Section>,
}

/// Parse the CHANGELOG.md at `path` and return up to `limit` released entries
/// in source order (newest first, as they appear).
pub fn parse_file(path: &Path, limit: usize) -> Result<Vec<Entry>> {
    let body = std::fs::read_to_string(path)
        .with_context(|| format!("read changelog: {}", path.display()))?;
    Ok(parse_str(&body, limit))
}

/// Parse a CHANGELOG body. Pure function, easy to unit-test.
pub fn parse_str(body: &str, limit: usize) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut current: Option<Entry> = None;
    let mut current_section: Option<Section> = None;
    // Buffer for the bullet we're actively accumulating across continuation
    // lines (Keep-A-Changelog items often wrap with 2-space indent).
    let mut pending_bullet: Option<String> = None;

    let commit_pending = |pending: &mut Option<String>, section: &mut Option<Section>| {
        if let (Some(text), Some(s)) = (pending.take(), section.as_mut()) {
            if s.bullets.len() < BULLETS_PER_SECTION {
                if let Some(cleaned) = finalize_bullet(&text) {
                    s.bullets.push(cleaned);
                }
            }
        }
    };

    for line in body.lines() {
        let trimmed = line.trim_end();

        if let Some((ver, date)) = parse_release_header(trimmed) {
            commit_pending(&mut pending_bullet, &mut current_section);
            flush_section(&mut current, &mut current_section);
            if let Some(done) = current.take() {
                entries.push(done);
            }
            if entries.len() >= limit {
                return entries;
            }
            current = Some(Entry {
                ver,
                date,
                sections: Vec::new(),
            });
            current_section = None;
            continue;
        }

        if let Some(name) = parse_section_header(trimmed) {
            commit_pending(&mut pending_bullet, &mut current_section);
            flush_section(&mut current, &mut current_section);
            current_section = Some(Section {
                name,
                bullets: Vec::new(),
            });
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("- ") {
            commit_pending(&mut pending_bullet, &mut current_section);
            pending_bullet = Some(rest.to_string());
            continue;
        }

        // Continuation line: 2+ leading spaces and a non-empty rest.
        if let Some(buf) = pending_bullet.as_mut() {
            if line.starts_with("  ") && !trimmed.is_empty() {
                buf.push(' ');
                buf.push_str(trimmed.trim_start());
                continue;
            }
            // Blank or de-dented line ends the bullet.
            commit_pending(&mut pending_bullet, &mut current_section);
        }
    }

    commit_pending(&mut pending_bullet, &mut current_section);
    flush_section(&mut current, &mut current_section);
    if let Some(done) = current.take() {
        entries.push(done);
    }

    entries.truncate(limit);
    entries
}

fn flush_section(entry: &mut Option<Entry>, section: &mut Option<Section>) {
    if let (Some(e), Some(s)) = (entry.as_mut(), section.take()) {
        if !s.bullets.is_empty() {
            e.sections.push(s);
        }
    }
}

/// Match `## [VER] - DATE`. Skips `[Unreleased]`. Returns (ver, date) on match.
fn parse_release_header(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("## ")?;
    let rest = rest.strip_prefix('[')?;
    let close = rest.find(']')?;
    let ver = &rest[..close];
    if ver.eq_ignore_ascii_case("unreleased") {
        return None;
    }
    let after = rest[close + 1..].trim_start();
    let date = after.strip_prefix('-')?.trim().to_string();
    if date.is_empty() {
        return None;
    }
    Some((ver.to_string(), date))
}

/// Match `### Section Name`. Returns a lowercased single-word label
/// (`added`, `changed`, `fixed`, `removed`, `breaking`). Returns `None` for
/// unknown sections so they're ignored.
fn parse_section_header(line: &str) -> Option<String> {
    let rest = line.strip_prefix("### ")?.trim();
    let first = rest.split_whitespace().next()?.to_ascii_lowercase();
    let normalized: &str = match first.as_str() {
        "added" => "added",
        "changed" => "changed",
        "fixed" => "fixed",
        "removed" => "removed",
        "breaking" => "breaking",
        "deprecated" => "deprecated",
        "security" => "security",
        _ => return None,
    };
    Some(normalized.to_string())
}

/// Clean a buffered bullet (already stripped of its `- ` lead) and apply the
/// site-side display rules: drop markdown, take the first sentence, truncate.
fn finalize_bullet(raw: &str) -> Option<String> {
    let cleaned = strip_markdown(raw);
    let one_line = first_sentence(&cleaned);
    let truncated = truncate(&one_line, MAX_BULLET_CHARS);
    let final_text = truncated.trim().to_string();
    if final_text.is_empty() {
        None
    } else {
        Some(final_text)
    }
}

/// Strip the small subset of markdown we use in bullets: `**bold**`, `*em*`,
/// `` `code` `` and `[text](url)` → `text`. Not a real markdown renderer —
/// just enough so the site doesn't show literal asterisks. Char-aware so
/// multi-byte glyphs like em-dashes survive intact.
fn strip_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '*' || c == '`' {
            continue;
        }
        if c == '[' {
            // [text](url) → text. Look ahead in the source slice.
            let rest = &input[i + c.len_utf8()..];
            if let Some(close_rel) = rest.find(']') {
                let after_close = &rest[close_rel + 1..];
                if let Some(stripped) = after_close.strip_prefix('(') {
                    if let Some(paren_close_rel) = stripped.find(')') {
                        out.push_str(&rest[..close_rel]);
                        // Advance the iterator past the consumed slice.
                        let consumed_bytes = c.len_utf8()
                            + close_rel
                            + 1 // ']'
                            + 1 // '('
                            + paren_close_rel
                            + 1; // ')'
                        let resume_at = i + consumed_bytes;
                        while let Some(&(idx, _)) = chars.peek() {
                            if idx < resume_at {
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        continue;
                    }
                }
            }
        }
        out.push(c);
    }
    out
}

/// Trim trailing whitespace and take the first sentence (split at `. `).
/// If no sentence boundary, return the whole input.
fn first_sentence(input: &str) -> String {
    let s = input.trim();
    if let Some(pos) = s.find(". ") {
        let mut sentence = s[..pos].to_string();
        sentence.push('.');
        sentence
    } else {
        s.to_string()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// Render entries as a JavaScript file assigning `window.WHETSTONE_CHANGELOG`.
pub fn render_js(entries: &[Entry]) -> String {
    let json = serde_json::to_string_pretty(entries).expect("serialize changelog");
    format!(
        "// Auto-generated by `whetstone changelog-sync` — do not edit.\n\
         // Source of truth: repo-root CHANGELOG.md.\n\
         window.WHETSTONE_CHANGELOG = {json};\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Changelog

## [Unreleased]

## [3.0.0] - 2026-06-08

### BREAKING — read this before upgrading from v2

- **AutoMem provider removed.** `MemoryProvider` is now `{ Icm, Skip }`. Side note here.
- **Whetstone no longer bundles skills, rules, or hook scripts.**
- **Hooks are tool-managed.**
- **Migration is required.**
- **`config.local.json` removed.**

### Added

- **`whetstone migrate`** — staged, reversible v2 → v3 migration.
- **`whetstone doctor`** — inspects installed tool versions.

### Fixed

- **RTK `MIN_VERSION`**: was `0.39.0`, which never shipped.

## [2.3.2] - 2025-05-26

### Fixed

- Release workflow: give verify-release job explicit repo context so things work.
";

    #[test]
    fn parses_release_headers_and_dates() {
        let entries = parse_str(SAMPLE, 10);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].ver, "3.0.0");
        assert_eq!(entries[0].date, "2026-06-08");
        assert_eq!(entries[1].ver, "2.3.2");
        assert_eq!(entries[1].date, "2025-05-26");
    }

    #[test]
    fn skips_unreleased_section() {
        let entries = parse_str(SAMPLE, 10);
        assert!(entries.iter().all(|e| e.ver.to_lowercase() != "unreleased"));
    }

    #[test]
    fn caps_bullets_per_section() {
        let entries = parse_str(SAMPLE, 10);
        let breaking = entries[0]
            .sections
            .iter()
            .find(|s| s.name == "breaking")
            .unwrap();
        assert_eq!(breaking.bullets.len(), BULLETS_PER_SECTION);
    }

    #[test]
    fn respects_limit() {
        let entries = parse_str(SAMPLE, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].ver, "3.0.0");
    }

    #[test]
    fn strips_markdown_emphasis() {
        let entries = parse_str(SAMPLE, 10);
        let added = entries[0]
            .sections
            .iter()
            .find(|s| s.name == "added")
            .unwrap();
        let migrate = &added.bullets[0];
        assert!(!migrate.contains('*'), "bold markers should be stripped");
        assert!(!migrate.contains('`'), "code ticks should be stripped");
        assert!(migrate.starts_with("whetstone migrate"));
    }

    #[test]
    fn takes_first_sentence_only() {
        let entries = parse_str(SAMPLE, 10);
        let breaking = entries[0]
            .sections
            .iter()
            .find(|s| s.name == "breaking")
            .unwrap();
        let first = &breaking.bullets[0];
        assert!(first.ends_with('.'));
        assert!(
            !first.contains("Side note"),
            "trailing sentences should be dropped: {first}"
        );
    }

    #[test]
    fn ignores_unknown_section_headers() {
        let body = "\
## [1.0.0] - 2026-01-01

### Bogus

- Should not appear

### Added

- Real entry.
";
        let entries = parse_str(body, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sections.len(), 1);
        assert_eq!(entries[0].sections[0].name, "added");
    }

    #[test]
    fn preserves_multibyte_glyphs() {
        let body = "\
## [1.0.0] - 2026-01-01

### Added

- **whetstone migrate** — staged, reversible v2 → v3 migration.
";
        let entries = parse_str(body, 10);
        let bullet = &entries[0].sections[0].bullets[0];
        assert!(bullet.contains('—'), "em-dash should survive: {bullet}");
        assert!(bullet.contains('→'), "arrow should survive: {bullet}");
        assert!(!bullet.contains('Ã'), "no UTF-8 corruption: {bullet}");
    }

    #[test]
    fn strips_link_keeps_text_only() {
        let body = "\
## [1.0.0] - 2026-01-01

### Added

- See the [Migration Guide](docs/migration.md) for details.
";
        let entries = parse_str(body, 10);
        let bullet = &entries[0].sections[0].bullets[0];
        assert!(bullet.contains("Migration Guide"));
        assert!(!bullet.contains('['));
        assert!(!bullet.contains("docs/migration"));
    }

    #[test]
    fn renders_js_with_window_assignment() {
        let entries = parse_str(SAMPLE, 10);
        let js = render_js(&entries);
        assert!(js.contains("window.WHETSTONE_CHANGELOG"));
        assert!(js.contains("\"ver\": \"3.0.0\""));
        assert!(js.contains("\"name\": \"breaking\""));
    }
}
