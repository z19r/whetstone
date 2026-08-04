//! Pure resolution of the `HEADROOM_*` environment whetstone sets when it
//! launches the proxy. No real env access — presence of an external var is
//! injected via a closure so the whole thing is unit-testable.

use std::collections::BTreeMap;

use crate::config::ProviderTag;

/// Whetstone hardwires the proxy port; letting the map set it would make
/// whetstone launch on one port and health-check `8787`. Dropped + warned.
const RESERVED_DENY: &[&str] = &["HEADROOM_PORT"];

/// Governed by dedicated code paths already; dropped silently to avoid
/// setting the same var twice with conflicting values.
const RESERVED_OWNED: &[&str] =
    &["HEADROOM_SAVINGS_PROFILE", "HEADROOM_TELEMETRY"];

/// The env whetstone will set when launching headroom.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HeadroomEnvPlan {
    /// `(key, value)` pairs to set on the proxy child / exec env.
    pub apply: Vec<(String, String)>,
    /// Reserved keys the user tried to set via the map — surfaced so the
    /// caller can warn. Excludes silently-owned keys.
    pub ignored: Vec<String>,
}

/// Build the launch-env plan. Precedence per key: an existing external env
/// var always wins (we skip it); otherwise a user-map entry wins over a
/// whetstone default. Reserved keys are filtered per the rules above.
pub fn build_headroom_env(
    user_map: &BTreeMap<String, String>,
    provider: ProviderTag,
    env_has: impl Fn(&str) -> bool,
) -> HeadroomEnvPlan {
    let mut plan = HeadroomEnvPlan::default();

    // A whetstone default: applied only when neither the external env nor
    // the user map already speaks for the key.
    let default = |plan: &mut HeadroomEnvPlan, key: &str, val: &str| {
        if env_has(key) || user_map.contains_key(key) {
            return;
        }
        plan.apply.push((key.to_string(), val.to_string()));
    };

    // Unconditional opinionated default: AST-aware compression for code.
    default(&mut plan, "HEADROOM_CODE_AWARE_ENABLED", "1");

    // Provider-gated: ICM owns memory, so suppress Headroom's own tools.
    if provider == ProviderTag::Icm {
        default(&mut plan, "HEADROOM_NO_MEMORY_TOOLS", "1");
        default(&mut plan, "HEADROOM_NO_MEMORY_CONTEXT", "1");
    }

    // User map passthrough (BTreeMap iterates in sorted key order, so the
    // output is deterministic).
    for (key, value) in user_map {
        if RESERVED_OWNED.contains(&key.as_str()) {
            continue;
        }
        if RESERVED_DENY.contains(&key.as_str()) {
            plan.ignored.push(key.clone());
            continue;
        }
        if env_has(key) {
            continue;
        }
        plan.apply.push((key.clone(), value.clone()));
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> bool {
        false
    }

    #[test]
    fn applies_code_aware_default() {
        let plan =
            build_headroom_env(&BTreeMap::new(), ProviderTag::Skip, no_env);
        assert!(plan
            .apply
            .contains(&("HEADROOM_CODE_AWARE_ENABLED".into(), "1".into())));
    }

    #[test]
    fn icm_provider_suppresses_headroom_memory() {
        let plan =
            build_headroom_env(&BTreeMap::new(), ProviderTag::Icm, no_env);
        assert!(plan
            .apply
            .contains(&("HEADROOM_NO_MEMORY_TOOLS".into(), "1".into())));
        assert!(plan
            .apply
            .contains(&("HEADROOM_NO_MEMORY_CONTEXT".into(), "1".into())));
    }

    #[test]
    fn skip_provider_leaves_memory_alone() {
        let plan =
            build_headroom_env(&BTreeMap::new(), ProviderTag::Skip, no_env);
        assert!(!plan
            .apply
            .iter()
            .any(|(k, _)| k.starts_with("HEADROOM_NO_MEMORY")));
    }

    #[test]
    fn external_env_wins_over_default() {
        let plan =
            build_headroom_env(&BTreeMap::new(), ProviderTag::Skip, |k| {
                k == "HEADROOM_CODE_AWARE_ENABLED"
            });
        assert!(!plan
            .apply
            .iter()
            .any(|(k, _)| k == "HEADROOM_CODE_AWARE_ENABLED"));
    }

    #[test]
    fn user_map_wins_over_default() {
        let mut map = BTreeMap::new();
        map.insert("HEADROOM_CODE_AWARE_ENABLED".into(), "0".into());
        let plan = build_headroom_env(&map, ProviderTag::Skip, no_env);
        // default not re-added; the user's 0 is applied exactly once
        let hits: Vec<_> = plan
            .apply
            .iter()
            .filter(|(k, _)| k == "HEADROOM_CODE_AWARE_ENABLED")
            .collect();
        assert_eq!(
            hits,
            vec![&("HEADROOM_CODE_AWARE_ENABLED".into(), "0".into())]
        );
    }

    #[test]
    fn external_env_wins_over_user_map() {
        let mut map = BTreeMap::new();
        map.insert("HEADROOM_RPM".into(), "120".into());
        let plan = build_headroom_env(&map, ProviderTag::Skip, |k| {
            k == "HEADROOM_RPM"
        });
        assert!(!plan.apply.iter().any(|(k, _)| k == "HEADROOM_RPM"));
    }

    #[test]
    fn port_is_denied_and_reported() {
        let mut map = BTreeMap::new();
        map.insert("HEADROOM_PORT".into(), "9999".into());
        let plan = build_headroom_env(&map, ProviderTag::Skip, no_env);
        assert!(!plan.apply.iter().any(|(k, _)| k == "HEADROOM_PORT"));
        assert_eq!(plan.ignored, vec!["HEADROOM_PORT".to_string()]);
    }

    #[test]
    fn owned_keys_dropped_silently() {
        let mut map = BTreeMap::new();
        map.insert("HEADROOM_SAVINGS_PROFILE".into(), "agent-50".into());
        map.insert("HEADROOM_TELEMETRY".into(), "on".into());
        let plan = build_headroom_env(&map, ProviderTag::Skip, no_env);
        assert!(!plan.apply.iter().any(|(k, _)| {
            k == "HEADROOM_SAVINGS_PROFILE" || k == "HEADROOM_TELEMETRY"
        }));
        assert!(plan.ignored.is_empty());
    }

    #[test]
    fn user_map_passthrough_applied() {
        let mut map = BTreeMap::new();
        map.insert("HEADROOM_LOG_MESSAGES".into(), "1".into());
        let plan = build_headroom_env(&map, ProviderTag::Skip, no_env);
        assert!(plan
            .apply
            .contains(&("HEADROOM_LOG_MESSAGES".into(), "1".into())));
    }
}
