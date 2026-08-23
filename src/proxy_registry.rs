//! Per-config Headroom proxy registry: fingerprint the exact proxy config a
//! session would spawn, so two sessions share a proxy iff they would spawn a
//! byte-identical one. Keyed reuse via `~/.whetstone/proxies.json`.

use std::collections::BTreeMap;

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

#[cfg(test)]
mod tests {
    use super::*;

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
