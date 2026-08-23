//! Per-config Headroom proxy registry: fingerprint the exact proxy config a
//! session would spawn, so two sessions share a proxy iff they would spawn a
//! byte-identical one. Keyed reuse via `~/.whetstone/proxies.json`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Everything that varies a spawned Headroom proxy's behavior. Two specs with
/// the same fingerprint may share one proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ProxySpec {
    /// Resolved `HEADROOM_SAVINGS_PROFILE`.
    pub savings_profile: String,
    /// Whether telemetry is enabled (drives `--no-telemetry` / env).
    pub telemetry: bool,
    /// Whether the proxy is started with `--memory`.
    pub memory: bool,
    /// Resolved `ANTHROPIC_TARGET_API_URL`, if any.
    pub anthropic_api_url: Option<String>,
    /// The `HEADROOM_*` apply set, EXCLUDING `HEADROOM_MEMORY_DB_PATH`.
    pub env: BTreeMap<String, String>,
}

#[allow(dead_code)]
fn canonical(spec: &ProxySpec) -> String {
    let mut s = String::new();
    s.push_str("sp=");
    s.push_str(&spec.savings_profile);
    s.push('\n');
    s.push_str("tel=");
    s.push(if spec.telemetry { '1' } else { '0' });
    s.push('\n');
    s.push_str("mem=");
    s.push(if spec.memory { '1' } else { '0' });
    s.push('\n');
    s.push_str("url=");
    s.push_str(spec.anthropic_api_url.as_deref().unwrap_or(""));
    s.push('\n');
    for (k, v) in &spec.env {
        s.push_str("e:");
        s.push_str(k);
        s.push('=');
        s.push_str(v);
        s.push('\n');
    }
    s
}

#[allow(dead_code)]
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// FNV-1a 64-bit over a canonical rendering of the spec.
#[allow(dead_code)]
pub fn proxy_fingerprint(spec: &ProxySpec) -> String {
    format!("{:016x}", fnv1a(canonical(spec).as_bytes()))
}

const GLOBAL_DIR: &str = ".whetstone";
const REGISTRY_FILENAME: &str = "proxies.json";

/// One whetstone-spawned proxy: its config fingerprint, bound port, and pid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyEntry {
    pub fingerprint: String,
    pub port: u16,
    pub pid: u32,
}

/// The set of live whetstone-spawned proxies. Persisted to
/// `~/.whetstone/proxies.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Registry {
    #[serde(default)]
    pub entries: Vec<ProxyEntry>,
}

/// `~/.whetstone/proxies.json`, or `None` if the home dir can't be found.
#[allow(dead_code)]
pub fn registry_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(GLOBAL_DIR).join(REGISTRY_FILENAME))
}

impl Registry {
    /// Load the registry, treating a missing or corrupt file as empty — a
    /// stale registry must never block a launch.
    #[allow(dead_code)]
    pub fn load(path: &Path) -> Self {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let pretty = serde_json::to_string_pretty(self)
            .context("serializing proxy registry")?;
        std::fs::write(path, pretty)
            .with_context(|| format!("writing {}", path.display()))
    }

    #[allow(dead_code)]
    pub fn find(&self, fingerprint: &str) -> Option<&ProxyEntry> {
        self.entries.iter().find(|e| e.fingerprint == fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("proxies.json");
        let reg = Registry::load(&path);
        assert!(reg.entries.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("proxies.json");
        let reg = Registry {
            entries: vec![ProxyEntry {
                fingerprint: "abc123".into(),
                port: 8801,
                pid: 4242,
            }],
        };
        reg.save(&path).unwrap();
        let back = Registry::load(&path);
        assert_eq!(back.entries, reg.entries);
        assert_eq!(back.find("abc123").map(|e| e.port), Some(8801));
        assert!(back.find("nope").is_none());
    }

    #[test]
    fn load_corrupt_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("proxies.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(Registry::load(&path).entries.is_empty());
    }

    fn base() -> ProxySpec {
        ProxySpec {
            savings_profile: "agent-90".into(),
            telemetry: false,
            memory: false,
            anthropic_api_url: None,
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn fingerprint_is_stable_and_16_hex() {
        let fp = proxy_fingerprint(&base());
        assert_eq!(fp.len(), 16);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(fp, proxy_fingerprint(&base()));
    }

    #[test]
    fn fingerprint_is_order_independent_for_env() {
        let mut a = base();
        a.env.insert("HEADROOM_RPM".into(), "1".into());
        a.env.insert("HEADROOM_CODE_AWARE_ENABLED".into(), "1".into());
        // BTreeMap canonicalizes order; inserting in the other order matches.
        let mut b = base();
        b.env.insert("HEADROOM_CODE_AWARE_ENABLED".into(), "1".into());
        b.env.insert("HEADROOM_RPM".into(), "1".into());
        assert_eq!(proxy_fingerprint(&a), proxy_fingerprint(&b));
    }

    #[test]
    fn each_field_changes_the_fingerprint() {
        let f0 = proxy_fingerprint(&base());

        let mut sp = base();
        sp.savings_profile = "agent-50".into();
        assert_ne!(f0, proxy_fingerprint(&sp));

        let mut tel = base();
        tel.telemetry = true;
        assert_ne!(f0, proxy_fingerprint(&tel));

        let mut mem = base();
        mem.memory = true;
        assert_ne!(f0, proxy_fingerprint(&mem));

        let mut url = base();
        url.anthropic_api_url = Some("https://example.test".into());
        assert_ne!(f0, proxy_fingerprint(&url));

        let mut env = base();
        env.env.insert("HEADROOM_RPM".into(), "120".into());
        assert_ne!(f0, proxy_fingerprint(&env));
    }
}
