# Implementation Plan: Launch-time model update prompt

**Design doc:** `docs/superpowers/specs/2026-08-02-model-update-prompt-design.md`
**Date:** 2026-08-02

## Goal

On `whetstone` launch (default `headroom wrap claude` path), detect that a model
newer than the one the project runs — or a brand-new model family — is available
in Anthropic's `/v1/models` list, and offer via a full-screen TUI modal to pin
it as the project default, use it for one session, or dismiss it permanently.
No-op when non-interactive, not a v3 project, offline, or `--model` was given.

## Architecture

- Pure decision core (`model_offers`, `is_model_newer`, family helpers) with
  zero I/O, exhaustively unit-tested — mirrors how `settings.rs` / `update.rs`
  isolate their decision logic.
- Effectful shell (`maybe_prompt`) runs guards, loads/saves state, and drives a
  `ratatui` modal following the `settings.rs` init/restore pattern.
- Per-project pin + dismissals live in `.claude/whetstone.json`; the ephemeral
  "seen" baseline lives in `~/.cache/whetstone/model-seen/<hash>.json` to keep
  the committed manifest a clean diff.

## Tech Stack

Rust; `serde`/`serde_json`, `ureq` (reused via `settings.rs`), `ratatui` +
`crossterm` (reused from `settings.rs`/`dashboard.rs`), `anyhow`, `dirs`,
`tempfile` (dev). No new dependencies.

## Constraints

- Max line length 80 chars, all files (linter-enforced house rule).
- `just release-check` (fmt + clippy + test) must pass; verify with
  `cargo test` / `cargo clippy` after each task.
- Immutability preferred; pure functions return new values, no in-place mutation
  of inputs.
- No AI-attribution trailers in any commit.
- Files stay < 800 lines; `model_update.rs` is a new focused module.

## Precise trigger rule (refines design doc)

Given `effective` (project's current model), `available` (current list), `seen`
(baseline), `dismissed`:

- **Family upgrade:** the newest id sharing `effective`'s family, if newer than
  `effective` (lexical, descending id). This subsumes the design doc's "Signal 1"
  and "Signal 2" — for a Sonnet-pinned or Sonnet-default project, newest-in-family
  *is* newest Sonnet, and a non-Sonnet pin is never offered a Sonnet here.
- **Brand-new family:** for any family with **no** id present in `seen`, its
  newest id in `available`. A new Sonnet *point release* is NOT this (the Sonnet
  family is already in `seen`) — that path is the family-upgrade above. This
  fires only for a genuinely unseen family, and only after seeding.
- Remove `dismissed` and `effective` itself; dedup; stable order.
- **First run** (`seen` empty): brand-new-family contributes nothing (everything
  looks new); only family-upgrade can fire. Caller seeds `seen` with `available`.

---

## Task 1: Manifest `dismissed_models` field

**Files:**
- Modify: `src/config.rs` (add field to `WhetstoneManifest`, ~line 146-160)
- Test: `src/config.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- `WhetstoneManifest.dismissed_models: Vec<String>`, annotated
  `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
- Initialized to `Vec::new()` in `WhetstoneManifest::new`.

**Steps:**
1. Write test `manifest_round_trips_dismissed_models` — build a manifest, push
   two ids, `save`/`load` via `NamedTempFile`, assert equality; and
   `legacy_manifest_without_dismissed_models_parses` — deserialize a JSON blob
   lacking the key, assert `dismissed_models.is_empty()`.
2. Run `cargo test config:: -- dismissed` — FAIL (field missing).
3. Add the field + `new()` init.
4. Run the tests — PASS.
5. `cargo clippy` clean; confirm `skip_serializing_if` keeps empty vecs out of
   the JSON (assert serialized string omits the key when empty).

## Task 2: Expose `settings.rs` helpers to the crate

**Files:**
- Modify: `src/settings.rs` (`family_order` ~106, `newest_sonnet` ~185,
  `live_available_models` ~164; `preferred_default_model` already `pub`)

**Interfaces:**
- `pub(crate) fn family_order(id: &str) -> u8`
- `pub(crate) fn newest_sonnet(models: &[String]) -> Option<String>`
- `pub(crate) fn live_available_models() -> Option<Vec<String>>`

**Steps:**
1. Change the three `fn` to `pub(crate) fn`. No behavior change.
2. `cargo test settings::` — PASS (existing tests unaffected).
3. `cargo clippy` clean (no dead-code warnings, since Task 3+ consume them; if
   ordering causes a transient warning, proceed — later tasks resolve it).

## Task 3: Pure decision core in `model_update.rs`

**Files:**
- Create: `src/model_update.rs`
- Modify: `src/main.rs` (add `mod model_update;`)
- Test: `src/model_update.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- `pub enum ModelDecision { UsePinned(String), UseSession(String), NoChange }`
- `fn same_family(a: &str, b: &str) -> bool` — both current-gen and equal
  `family_order` (family_order == 4 "unknown" never matches).
- `fn is_model_newer(candidate: &str, current: &str) -> bool` — `same_family`
  AND `candidate > current` lexically.
- `fn newest_in_family(available: &[String], model: &str) -> Option<String>`
- `fn model_offers(effective: &str, available: &[String], seen: &[String],
  dismissed: &[String]) -> Vec<String>` — the precise trigger rule above.

**Steps:**
1. Write unit tests first (RED):
   - `is_model_newer_within_family` / `_cross_family_false` /
     `_same_id_false`.
   - `offers_family_upgrade_for_pinned_older_sonnet` (pinned `sonnet-4-6`,
     available has `sonnet-5` → `["claude-sonnet-5"]`).
   - `offers_family_upgrade_for_pinned_opus` (pinned `opus-4-6` +
     `opus-4-8` → offered).
   - `no_sonnet_offer_for_opus_pin_when_sonnet_family_seen` (pinned `opus-4-6`,
     seen contains a sonnet, newer sonnet present → empty).
   - `offers_brand_new_family_flagship` (seen has no `fable`; available adds
     `claude-fable-5` → offered even to an opus pin).
   - `first_run_suppresses_brand_new_family` (`seen` empty → brand-new path
     contributes nothing; only family-upgrade can appear).
   - `dismissed_excluded_but_newer_still_offered`.
   - `effective_never_offered_and_results_deduped`.
2. `cargo test model_update::` — FAIL (module/functions absent).
3. Implement the helpers + `model_offers`, reusing `crate::settings::family_order`
   / `newest_sonnet`. Keep functions pure; no I/O.
4. `cargo test model_update::` — PASS.
5. `cargo clippy` clean; lines ≤ 80.

## Task 4: Seen-baseline cache (per-project)

**Files:**
- Modify: `src/model_update.rs` (add cache read/write)
- Test: `src/model_update.rs`

**Interfaces:**
- `fn seen_cache_path(project_dir: &Path) -> Option<PathBuf>` —
  `~/.cache/whetstone/model-seen/<hash>.json`, where `<hash>` is a stable hash
  of the canonicalized project dir (use `std::hash` / hex of the path string;
  no new dep).
- `fn read_seen(project_dir: &Path) -> Vec<String>` — missing/garbage ⇒ `[]`.
- `fn write_seen(project_dir: &Path, models: &[String])` — best-effort, ignores
  errors (like `settings.rs::write_models_cache`).

**Steps:**
1. Write tests (RED): `read_seen_missing_returns_empty`;
   `write_then_read_round_trips` (point `HOME`/cache at a `tempfile::tempdir`
   via a path-injection seam — factor the base dir into a helper param so the
   test doesn't touch the real home). `seen_path_stable_for_same_dir` and
   `seen_path_differs_across_dirs`.
2. `cargo test model_update::seen` — FAIL.
3. Implement; keep a testable pure `seen_cache_path_in(base, project_dir)` and a
   thin wrapper that supplies the real cache base.
4. PASS; clippy clean.

## Task 5: `maybe_prompt` orchestration (no TUI yet)

**Files:**
- Modify: `src/model_update.rs`
- Test: `src/model_update.rs`

**Interfaces:**
- `pub fn maybe_prompt(resolved: &ResolvedSettings) -> ModelDecision` — the
  entry point. Internally:
  - `fn effective_model(resolved: &ResolvedSettings) -> Option<String>` —
    `resolved.api_model` else `settings::preferred_default_model()`.
  - `fn apply_action(action, offered, manifest_path, project_dir) -> ModelDecision`
    — pure-ish glue: Pin writes `settings.api_model` + `touch_and_save`; Dismiss
    appends `offered` to `dismissed_models` + `touch_and_save`; Session/NotNow
    persist nothing. Returns the mapped `ModelDecision`.
- A small `enum ModalAction { Pin(String), Session(String), Dismiss, NotNow }`
  produced by the TUI (Task 6) and consumed by `apply_action`.

**Guards in `maybe_prompt` (return `NoChange` on any):** not
`ui::is_interactive()`; no `current_dir`; no manifest at
`WhetstoneManifest::path_for(cwd)`; `live_available_models()` is `None`.
On no offers: `write_seen` (if list changed) and return `NoChange`.

**Steps:**
1. Write tests (RED) for `apply_action` against a `tempfile` manifest:
   - `apply_pin_writes_api_model_and_returns_use_pinned`.
   - `apply_dismiss_appends_to_dismissed_models_and_returns_no_change`.
   - `apply_session_persists_nothing_returns_use_session`.
   - `apply_not_now_persists_nothing_returns_no_change`.
   - `dismiss_does_not_duplicate_existing_ids`.
2. `cargo test model_update::apply` — FAIL.
3. Implement `apply_action` + `effective_model`. Leave the modal call behind a
   function `prompt_modal(offered: &[String]) -> ModalAction` (Task 6) so
   `maybe_prompt` compiles with a `todo!()`/stub only in the TUI seam — but keep
   `apply_action` fully tested here.
4. PASS for the `apply_*` tests; clippy clean.

## Task 6: Full-screen TUI modal

**Files:**
- Modify: `src/model_update.rs` (add `ratatui` modal)

**Interfaces:**
- `fn prompt_modal(offered: &[String]) -> ModalAction` — `ratatui::init()`,
  render loop, `ratatui::restore()`. Keys: `↑↓`/`j`/`k` select model (when
  >1 offered), `p` Pin, `s` Session, `d` Dismiss, `esc`/`q` Not now. Enter =
  Pin (the highlighted model).

**Steps:**
1. Implement the modal following `settings.rs` (`draw`, `run_loop`,
   `KeyEventKind::Press` filtering, `init`/`restore`). The render loop itself is
   not unit-tested (consistent with `settings.rs`); all decision logic already
   lives in the pure core.
2. Wire `prompt_modal` into `maybe_prompt`, replacing the Task-5 stub, mapping
   the returned `ModalAction` through `apply_action`.
3. `cargo build` + manual smoke per the Verification section.
4. `cargo clippy` clean; lines ≤ 80.

## Task 7: Wire into `wrap_claude` + docs

**Files:**
- Modify: `src/wrapper.rs` (`wrap_claude` ~52-71; `build_claude_args` unchanged)
- Modify: `CLAUDE.md` (CLI Reference note), `docs/cli-reference.md`,
  `docs/configuration.md` (document the prompt + `dismissed_models`),
  `CHANGELOG.md` (Unreleased entry)
- Test: `src/wrapper.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- In `wrap_claude`, compute `user_set_model` (reuse the existing check from
  `build_claude_args`, lifted to a `pub(crate) fn user_set_model(args) -> bool`
  or duplicated locally), then:
  ```rust
  let model = if user_set_model {
      resolve_model(resolved.api_model.clone())
  } else {
      match crate::model_update::maybe_prompt(&resolved) {
          ModelDecision::UsePinned(m) | ModelDecision::UseSession(m) => m,
          ModelDecision::NoChange => resolve_model(resolved.api_model.clone()),
      }
  };
  ```

**Steps:**
1. Write test (RED): factor the "explicit --model suppresses prompt" decision
   into a pure `fn choose_model(user_set: bool, decision: ModelDecision,
   fallback: &str) -> String` and test all branches (`UsePinned`, `UseSession`,
   `NoChange`, and `user_set=true` ignoring a non-`NoChange` decision).
2. `cargo test wrapper::choose_model` — FAIL.
3. Implement `choose_model`; call it from `wrap_claude` with
   `resolve_model(...)` as the fallback and `maybe_prompt` result.
4. `cargo test` (all) — PASS.
5. Update the four docs + CHANGELOG (concise; note per-project scope, dismissal,
   and that `--model`/non-interactive/offline skip it).
6. `just release-check` — fmt + clippy + full test suite PASS.

---

## Verification (manual smoke, Task 6/7)

Run `cargo build` then, in a v3 project (has `.claude/whetstone.json`):

1. With `settings.api_model` pinned to an older id and a valid
   `ANTHROPIC_API_KEY`, delete the seen cache, run the built binary as
   `whetstone` — modal appears offering the newer model. Choose Pin; confirm
   `whetstone.json` gained `api_model`. Re-run — no modal (already newest).
2. Choose Dismiss on an offer; confirm `dismissed_models` gained the id and a
   re-run shows no modal.
3. Run with `--model claude-x` — no modal.
4. Run with `ANTHROPIC_API_KEY` unset (offline) — no modal, launch proceeds.
5. Pipe non-interactively (`echo | whetstone ...`) — no modal.

## Execution

Implement task-by-task via `superpowers:executing-plans`. Each task is
TDD: failing test → implementation → green → clippy/fmt. Do not proceed to a
task until the previous task's tests and `cargo clippy` are green.
