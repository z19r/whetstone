//! Launch-time model update prompt.
//!
//! On the default `headroom wrap claude` launch, whetstone notices that
//! Anthropic has published a model newer than the one this project runs — or a
//! brand-new model family — and offers, via a full-screen modal, to pin it as
//! the project default, use it for one session, or dismiss it permanently.
//!
//! The decision logic here is pure and exhaustively unit-tested; the effectful
//! shell (guards, state I/O, TUI) lives alongside it but is kept thin.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::{ResolvedSettings, WhetstoneManifest};
use crate::settings::family_order;

/// `family_order` value assigned to models we don't recognize; never matches.
const UNKNOWN_FAMILY: u8 = 4;

/// What the caller does with the model list after the prompt resolves.
#[derive(Debug, PartialEq, Eq)]
pub enum ModelDecision {
    /// Pin as project default and launch with it.
    UsePinned(String),
    /// Launch with it once; nothing persisted.
    UseSession(String),
    /// Leave resolution to the existing default path.
    NoChange,
}

/// The action a user picks in the modal, orthogonal to which model is
/// highlighted (that id is supplied separately to [`apply_action`]).
// Variants are constructed by the TUI modal (Task 6); allow until then.
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ModalAction {
    /// Pin the highlighted model as the project default.
    Pin,
    /// Use the highlighted model for this session only.
    Session,
    /// Never offer the highlighted model again.
    Dismiss,
    /// Do nothing; ask again next launch.
    NotNow,
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

/// Stable hex hash of a project directory, used to name its seen-cache file.
/// Canonicalizes when possible so `.`/symlinks map to one file; falls back to
/// the raw path (e.g. for a not-yet-existing dir in tests).
fn hash_project_dir(project_dir: &Path) -> String {
    let canon = std::fs::canonicalize(project_dir).unwrap_or_else(|_| project_dir.to_path_buf());
    let mut hasher = DefaultHasher::new();
    canon.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Path to a project's seen-cache file under an arbitrary cache base.
fn seen_cache_path_in(base: &Path, project_dir: &Path) -> PathBuf {
    base.join("model-seen")
        .join(format!("{}.json", hash_project_dir(project_dir)))
}

/// Real cache base: `~/.cache/whetstone`.
fn seen_cache_base() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".cache").join("whetstone"))
}

/// Read the seen baseline under an arbitrary base. Missing or garbage ⇒ `[]`.
fn read_seen_in(base: &Path, project_dir: &Path) -> Vec<String> {
    let path = seen_cache_path_in(base, project_dir);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Best-effort write of the seen baseline under an arbitrary base.
fn write_seen_in(base: &Path, project_dir: &Path, models: &[String]) {
    let path = seen_cache_path_in(base, project_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(models) {
        let _ = std::fs::write(path, json);
    }
}

/// Seen baseline for `project_dir` from the real cache; `[]` if unavailable.
fn read_seen(project_dir: &Path) -> Vec<String> {
    match seen_cache_base() {
        Some(base) => read_seen_in(&base, project_dir),
        None => Vec::new(),
    }
}

/// Persist the seen baseline for `project_dir` to the real cache (best-effort).
fn write_seen(project_dir: &Path, models: &[String]) {
    if let Some(base) = seen_cache_base() {
        write_seen_in(&base, project_dir, models);
    }
}

/// The model a launch would use today: the project's pin, else the newest
/// available Sonnet. `None` only when neither is determinable.
fn effective_model(resolved: &ResolvedSettings) -> Option<String> {
    resolved
        .api_model
        .clone()
        .or_else(crate::settings::preferred_default_model)
}

/// Persist (or not) the user's modal choice and map it to a [`ModelDecision`].
/// `selected` is the highlighted model id the action applies to.
fn apply_action(
    action: ModalAction,
    selected: &str,
    manifest_path: &Path,
) -> Result<ModelDecision> {
    match action {
        ModalAction::Pin => {
            let Some(mut manifest) = WhetstoneManifest::load(manifest_path)? else {
                return Ok(ModelDecision::NoChange);
            };
            manifest.settings.api_model = Some(selected.to_string());
            manifest.touch_and_save(manifest_path)?;
            Ok(ModelDecision::UsePinned(selected.to_string()))
        }
        ModalAction::Session => Ok(ModelDecision::UseSession(selected.to_string())),
        ModalAction::Dismiss => {
            let Some(mut manifest) = WhetstoneManifest::load(manifest_path)? else {
                return Ok(ModelDecision::NoChange);
            };
            if !manifest.dismissed_models.iter().any(|d| d == selected) {
                manifest.dismissed_models.push(selected.to_string());
                manifest.touch_and_save(manifest_path)?;
            }
            Ok(ModelDecision::NoChange)
        }
        ModalAction::NotNow => Ok(ModelDecision::NoChange),
    }
}

/// Launch-time entry point. Returns [`ModelDecision::NoChange`] on any guard
/// (non-interactive, no cwd, no v3 manifest, offline) so every existing launch
/// path is unaffected. Otherwise computes offers, seeds the seen baseline, and
/// — when there is something to offer — drives the modal and applies the choice.
// Wired into `wrap_claude` in Task 7; keeps the reachable core clippy-clean.
#[allow(dead_code)]
pub fn maybe_prompt(resolved: &ResolvedSettings) -> ModelDecision {
    if !crate::ui::is_interactive() {
        return ModelDecision::NoChange;
    }
    let Ok(cwd) = std::env::current_dir() else {
        return ModelDecision::NoChange;
    };
    let manifest_path = WhetstoneManifest::path_for(&cwd);
    let Ok(Some(manifest)) = WhetstoneManifest::load(&manifest_path) else {
        return ModelDecision::NoChange;
    };
    let Some(available) = crate::settings::live_available_models() else {
        return ModelDecision::NoChange;
    };

    let effective =
        effective_model(resolved).unwrap_or_else(|| crate::wrapper::DEFAULT_MODEL.to_string());
    let seen = read_seen(&cwd);
    let offers = model_offers(&effective, &available, &seen, &manifest.dismissed_models);

    // Record the current list as the new baseline whenever it changed (this is
    // also the first-run seeding that suppresses brand-new-family until later).
    if seen != available {
        write_seen(&cwd, &available);
    }

    if offers.is_empty() {
        return ModelDecision::NoChange;
    }

    let (action, selected) = prompt_modal(&offers);
    apply_action(action, &selected, &manifest_path).unwrap_or(ModelDecision::NoChange)
}

/// Full-screen modal implemented in a later task; returns the chosen action
/// and the highlighted model id.
fn prompt_modal(_offered: &[String]) -> (ModalAction, String) {
    todo!("TUI modal implemented in Task 6")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ToolVersions, WhetstoneManifest};
    use crate::memory::MemoryProvider;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn seed_manifest(path: &Path) {
        WhetstoneManifest::new(MemoryProvider::Icm, ToolVersions::default())
            .save(path)
            .unwrap();
    }

    #[test]
    fn apply_pin_writes_api_model_and_returns_use_pinned() {
        let f = tempfile::NamedTempFile::new().unwrap();
        seed_manifest(f.path());
        let decision = apply_action(ModalAction::Pin, "claude-opus-4-8", f.path()).unwrap();
        assert_eq!(decision, ModelDecision::UsePinned("claude-opus-4-8".into()));
        let loaded = WhetstoneManifest::load(f.path()).unwrap().unwrap();
        assert_eq!(
            loaded.settings.api_model.as_deref(),
            Some("claude-opus-4-8")
        );
    }

    #[test]
    fn apply_dismiss_appends_to_dismissed_models_and_returns_no_change() {
        let f = tempfile::NamedTempFile::new().unwrap();
        seed_manifest(f.path());
        let decision = apply_action(ModalAction::Dismiss, "claude-fable-5", f.path()).unwrap();
        assert_eq!(decision, ModelDecision::NoChange);
        let loaded = WhetstoneManifest::load(f.path()).unwrap().unwrap();
        assert_eq!(loaded.dismissed_models, s(&["claude-fable-5"]));
    }

    #[test]
    fn apply_session_persists_nothing_returns_use_session() {
        let f = tempfile::NamedTempFile::new().unwrap();
        seed_manifest(f.path());
        let decision = apply_action(ModalAction::Session, "claude-sonnet-5", f.path()).unwrap();
        assert_eq!(
            decision,
            ModelDecision::UseSession("claude-sonnet-5".into())
        );
        let loaded = WhetstoneManifest::load(f.path()).unwrap().unwrap();
        assert!(loaded.settings.api_model.is_none());
        assert!(loaded.dismissed_models.is_empty());
    }

    #[test]
    fn apply_not_now_persists_nothing_returns_no_change() {
        let f = tempfile::NamedTempFile::new().unwrap();
        seed_manifest(f.path());
        let decision = apply_action(ModalAction::NotNow, "claude-sonnet-5", f.path()).unwrap();
        assert_eq!(decision, ModelDecision::NoChange);
        let loaded = WhetstoneManifest::load(f.path()).unwrap().unwrap();
        assert!(loaded.settings.api_model.is_none());
        assert!(loaded.dismissed_models.is_empty());
    }

    #[test]
    fn dismiss_does_not_duplicate_existing_ids() {
        let f = tempfile::NamedTempFile::new().unwrap();
        seed_manifest(f.path());
        apply_action(ModalAction::Dismiss, "claude-fable-5", f.path()).unwrap();
        apply_action(ModalAction::Dismiss, "claude-fable-5", f.path()).unwrap();
        let loaded = WhetstoneManifest::load(f.path()).unwrap().unwrap();
        assert_eq!(loaded.dismissed_models, s(&["claude-fable-5"]));
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
    fn read_seen_missing_returns_empty() {
        let base = tempfile::tempdir().unwrap();
        let proj = Path::new("/some/project");
        assert!(read_seen_in(base.path(), proj).is_empty());
    }

    #[test]
    fn write_then_read_round_trips() {
        let base = tempfile::tempdir().unwrap();
        let proj = Path::new("/some/project");
        let models = s(&["claude-opus-4-8", "claude-sonnet-5"]);
        write_seen_in(base.path(), proj, &models);
        assert_eq!(read_seen_in(base.path(), proj), models);
    }

    #[test]
    fn seen_path_stable_for_same_dir() {
        let base = tempfile::tempdir().unwrap();
        let proj = Path::new("/some/project");
        assert_eq!(
            seen_cache_path_in(base.path(), proj),
            seen_cache_path_in(base.path(), proj)
        );
    }

    #[test]
    fn seen_path_differs_across_dirs() {
        let base = tempfile::tempdir().unwrap();
        let a = seen_cache_path_in(base.path(), Path::new("/proj/a"));
        let b = seen_cache_path_in(base.path(), Path::new("/proj/b"));
        assert_ne!(a, b);
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
