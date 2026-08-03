//! Launch-time model update prompt.
//!
//! On the default `headroom wrap claude` launch, whetstone notices that
//! Anthropic has published a model newer than the one this project runs — or a
//! brand-new model family — and offers, via a full-screen modal, to pin it as
//! the project default, use it for one session, or dismiss it permanently.
//!
//! The decision logic here is pure and exhaustively unit-tested; the effectful
//! shell (guards, state I/O, TUI) lives alongside it but is kept thin.

use std::collections::HashSet;

use crate::settings::family_order;

/// `family_order` value assigned to models we don't recognize; never matches.
const UNKNOWN_FAMILY: u8 = 4;

/// What the caller does with the model list after the prompt resolves.
#[allow(dead_code)] // consumed by later tasks (maybe_prompt / wrap_claude)
pub enum ModelDecision {
    /// Pin as project default and launch with it.
    UsePinned(String),
    /// Launch with it once; nothing persisted.
    UseSession(String),
    /// Leave resolution to the existing default path.
    NoChange,
}

/// Two ids belong to the same recognized model family.
fn same_family(a: &str, b: &str) -> bool {
    let fa = family_order(a);
    fa != UNKNOWN_FAMILY && fa == family_order(b)
}

/// `candidate` is a newer release of `current`'s own family.
fn is_model_newer(candidate: &str, current: &str) -> bool {
    same_family(candidate, current) && candidate > current
}

/// Newest id in `available` that shares `model`'s family (lexical max).
fn newest_in_family(available: &[String], model: &str) -> Option<String> {
    available
        .iter()
        .filter(|id| same_family(id, model))
        .max()
        .cloned()
}

/// Models worth offering, per the trigger rule in the design doc:
/// a newer release within `effective`'s own family, plus the flagship of any
/// recognized family present in `available` but absent from `seen`. `effective`
/// and `dismissed` ids are removed and the result is deduped in stable order.
/// On first run (`seen` empty) the brand-new-family signal is suppressed.
// Wired into `maybe_prompt` in a later task; its callees are reachable through
// it, so this single allow keeps the whole pure core clippy-clean until then.
#[allow(dead_code)]
fn model_offers(
    effective: &str,
    available: &[String],
    seen: &[String],
    dismissed: &[String],
) -> Vec<String> {
    let mut offers: Vec<String> = Vec::new();

    // Signal: a newer release within the in-use family.
    if let Some(newest) = newest_in_family(available, effective) {
        if is_model_newer(&newest, effective) {
            offers.push(newest);
        }
    }

    // Signal: a brand-new family flagship (only after seeding).
    if !seen.is_empty() {
        let seen_families: HashSet<u8> = seen.iter().map(|id| family_order(id)).collect();
        let mut new_families: Vec<u8> = available
            .iter()
            .map(|id| family_order(id))
            .filter(|f| *f != UNKNOWN_FAMILY && !seen_families.contains(f))
            .collect();
        new_families.sort_unstable();
        new_families.dedup();
        for f in new_families {
            if let Some(flagship) = available.iter().filter(|id| family_order(id) == f).max() {
                offers.push(flagship.clone());
            }
        }
    }

    let mut deduped = HashSet::new();
    offers
        .into_iter()
        .filter(|m| m != effective && !dismissed.contains(m))
        .filter(|m| deduped.insert(m.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn is_model_newer_within_family() {
        assert!(is_model_newer("claude-sonnet-5", "claude-sonnet-4-6"));
    }

    #[test]
    fn is_model_newer_cross_family_false() {
        assert!(!is_model_newer("claude-opus-4-8", "claude-sonnet-4-6"));
    }

    #[test]
    fn is_model_newer_same_id_false() {
        assert!(!is_model_newer("claude-sonnet-5", "claude-sonnet-5"));
    }

    #[test]
    fn offers_family_upgrade_for_pinned_older_sonnet() {
        let available = s(&["claude-sonnet-5", "claude-sonnet-4-6"]);
        let seen = s(&["claude-sonnet-4-6"]);
        let offers = model_offers("claude-sonnet-4-6", &available, &seen, &[]);
        assert_eq!(offers, s(&["claude-sonnet-5"]));
    }

    #[test]
    fn offers_family_upgrade_for_pinned_opus() {
        let available = s(&["claude-opus-4-8", "claude-opus-4-6"]);
        let seen = s(&["claude-opus-4-6"]);
        let offers = model_offers("claude-opus-4-6", &available, &seen, &[]);
        assert_eq!(offers, s(&["claude-opus-4-8"]));
    }

    #[test]
    fn no_sonnet_offer_for_opus_pin_when_sonnet_family_seen() {
        let available = s(&["claude-opus-4-6", "claude-sonnet-5"]);
        let seen = s(&["claude-opus-4-6", "claude-sonnet-4-6"]);
        let offers = model_offers("claude-opus-4-6", &available, &seen, &[]);
        assert!(offers.is_empty());
    }

    #[test]
    fn offers_brand_new_family_flagship() {
        let available = s(&["claude-opus-4-6", "claude-fable-5"]);
        let seen = s(&["claude-opus-4-6"]);
        let offers = model_offers("claude-opus-4-6", &available, &seen, &[]);
        assert_eq!(offers, s(&["claude-fable-5"]));
    }

    #[test]
    fn first_run_suppresses_brand_new_family() {
        // seen empty ⇒ everything looks new; only family-upgrade may fire.
        let available = s(&["claude-opus-4-6", "claude-opus-4-8", "claude-fable-5"]);
        let offers = model_offers("claude-opus-4-6", &available, &[], &[]);
        assert_eq!(offers, s(&["claude-opus-4-8"]));
    }

    #[test]
    fn dismissed_excluded_but_newer_still_offered() {
        let available = s(&["claude-sonnet-4-6", "claude-sonnet-6"]);
        let seen = s(&["claude-sonnet-4-6"]);
        // Dismissing an older candidate does not block a newer one.
        let offers = model_offers(
            "claude-sonnet-4-6",
            &available,
            &seen,
            &s(&["claude-sonnet-5"]),
        );
        assert_eq!(offers, s(&["claude-sonnet-6"]));
        // Dismissing the actual newest yields nothing.
        let offers = model_offers(
            "claude-sonnet-4-6",
            &available,
            &seen,
            &s(&["claude-sonnet-6"]),
        );
        assert!(offers.is_empty());
    }

    #[test]
    fn effective_never_offered_and_results_deduped() {
        // effective is already newest in family ⇒ not offered.
        let available = s(&["claude-sonnet-5"]);
        let seen = s(&["claude-sonnet-5"]);
        assert!(model_offers("claude-sonnet-5", &available, &seen, &[]).is_empty());

        // Family-upgrade and brand-new-family compute the same flagship for a
        // family absent from `seen`; result is deduped.
        let available = s(&["claude-opus-4-6", "claude-opus-4-8"]);
        let seen = s(&["claude-sonnet-5"]);
        let offers = model_offers("claude-opus-4-6", &available, &seen, &[]);
        assert_eq!(offers, s(&["claude-opus-4-8"]));
    }
}
