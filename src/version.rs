use anyhow::{Context, Result};
use semver::Version;
use std::fs;
use std::path::Path;

pub fn current() -> &'static str {
    env!("WHETSTONE_VERSION")
}

pub fn read_from_file(path: &Path) -> Result<Version> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading VERSION from {}", path.display()))?;
    let trimmed = raw.trim();
    Version::parse(trimmed)
        .with_context(|| format!("parsing semver from '{trimmed}'"))
}

pub fn write_to_file(path: &Path, version: &Version) -> Result<()> {
    fs::write(path, format!("{version}\n"))
        .with_context(|| format!("writing VERSION to {}", path.display()))
}

pub fn bump(current: &Version, kind: BumpKind) -> Version {
    match kind {
        BumpKind::Patch => Version::new(current.major, current.minor, current.patch + 1),
        BumpKind::Minor => Version::new(current.major, current.minor + 1, 0),
        BumpKind::Major => Version::new(current.major + 1, 0, 0),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BumpKind {
    Patch,
    Minor,
    Major,
}

pub fn is_older(installed: &str, minimum: &str) -> bool {
    let Ok(inst) = Version::parse(installed) else {
        return true;
    };
    let Ok(min) = Version::parse(minimum) else {
        return false;
    };
    inst < min
}

pub fn extract_semver(raw: &str) -> Option<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                if chars[i] == '.' {
                    dots += 1;
                }
                i += 1;
            }
            if dots >= 2 {
                let candidate: String = chars[start..i].iter().collect();
                let candidate = candidate.trim_end_matches('.');
                if Version::parse(candidate).is_ok() {
                    return Some(candidate.to_string());
                }
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_patch() {
        let v = Version::new(1, 2, 3);
        assert_eq!(bump(&v, BumpKind::Patch), Version::new(1, 2, 4));
    }

    #[test]
    fn bump_minor() {
        let v = Version::new(1, 2, 3);
        assert_eq!(bump(&v, BumpKind::Minor), Version::new(1, 3, 0));
    }

    #[test]
    fn bump_major() {
        let v = Version::new(1, 2, 3);
        assert_eq!(bump(&v, BumpKind::Major), Version::new(2, 0, 0));
    }

    #[test]
    fn extract_from_version_string() {
        assert_eq!(extract_semver("headroom 0.14.2"), Some("0.14.2".into()));
        assert_eq!(extract_semver("no version here"), None);
    }

    #[test]
    fn is_older_works() {
        assert!(is_older("0.13.0", "0.14.0"));
        assert!(!is_older("0.14.0", "0.14.0"));
        assert!(!is_older("0.15.0", "0.14.0"));
    }
}
