# Launch-time model update prompt — design

**Date:** 2026-08-02
**Status:** Proposed (awaiting approval)

## Purpose

When a user launches `whetstone` (the default `headroom wrap claude` path),
whetstone should notice that Anthropic has published a model newer than the one
this project actually runs — or a brand-new model family — and offer, via a
full-screen modal, to **pin that model as the project default**. The user can
also take it for one session only, or dismiss it so it never nags again.

## Constraints / reality check

- Anthropic's `/v1/models` API returns **only a flat list of model IDs**. There
  is no "recommendation" feed. "Recommendations changed" can therefore only
  mean: *the newest model whetstone derives from that list changed vs. what this
  project runs / has already seen.*
- Whetstone already fetches and 12h-caches that list
  (`settings.rs::live_available_models`) and already derives a recommended
  default (`preferred_default_model` = newest Sonnet by id). This feature reuses
  both — no new network path, no marginal cost on a warm cache.
- The prompt only makes sense **interactively**, **inside a v3 project** (one
  with `.claude/whetstone.json`, so we have somewhere to store the pin and the
  dismissal), and **when the model list is actually reachable**. Any of those
  missing ⇒ silently do nothing.

## Success criteria

1. Launching `whetstone` in a v3 project surfaces a modal **only** when there is
   a genuinely newer/new model to offer that hasn't been dismissed.
2. The modal offers: **Pin as project default**, **Use this session only**,
   **Dismiss (don't ask again for this model)**, and **Not now** (Esc — ask
   again next launch).
3. Pinning writes `settings.api_model` into `.claude/whetstone.json`.
4. Dismissing records the model ID in `.claude/whetstone.json`; that model is
   never offered again, but a *newer* one later still triggers the modal.
5. Non-interactive / non-project / offline launches behave exactly as today.
6. No behavior change to `whetstone claude`/explicit `--model` flows: an
   explicit `--model` on the command line still wins and suppresses the prompt.

## What counts as "a model to offer" (the trigger)

The user asked for all three signals to count. Unified into one pure function
`model_offers(effective, available, seen, dismissed) -> Vec<String>`:

- **`effective`** = the project's current model: `settings.api_model` if pinned,
  else `preferred_default_model()` (newest Sonnet) — i.e. what a launch would
  actually use today.
- **Signal 1 — newer within the in-use family.** If the newest model in
  `effective`'s own family (lexically, per `family_order` + descending id) is
  newer than `effective`, offer it. (e.g. pinned `claude-opus-4-6` →
  `claude-opus-4-8` appears.)
- **Signal 2 — recommended default advanced.** If the newest Sonnet is newer
  than `effective`, offer it — but **only** when the project is unpinned or
  pinned to a Sonnet. A project that deliberately pinned a non-Sonnet family is
  not nagged to switch families; it only gets Signals 1 & 3. (Respects an
  intentional family choice.)
- **Signal 3 — brand-new family flagship.** A family flagship (newest id of its
  family) that appears in `available` but is **not in `seen`** is offered. This
  is the only signal that needs a "seen" baseline.

Offers then have `dismissed` and `effective` itself removed, and are deduped.

### First-run seeding

On the first launch after this ships, `seen` is empty, so *every* family would
look "brand new" (Signal 3). To avoid a flood, when `seen` is empty we **seed it
with the current list and suppress Signal 3 for that launch** — Signals 1 & 2
(pure version comparisons, legitimately useful on first run) still fire.

## State: where it lives

Two concerns, two homes:

- **User-meaningful, project-scoped, git-committed** → `.claude/whetstone.json`:
  - `settings.api_model` — the pin (existing field, reused).
  - `dismissed_models: Vec<String>` — **new** top-level manifest field
    (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`), not inside
    `ProjectSettings` (it's bookkeeping, not a knob the settings TUI edits).
- **Ephemeral bookkeeping** → `~/.cache/whetstone/model-seen/<project-hash>.json`:
  - `seen: Vec<String>` — the model IDs last observed for this project. Kept out
    of `whetstone.json` so that file stays a clean, reviewable diff (it churns
    every time Anthropic's list changes, which is not something to commit).

> Open question for review: if you'd rather keep *everything* in
> `whetstone.json` (one file, simpler), we drop the cache file and store `seen`
> in the manifest too — at the cost of a noisier committed file. Recommendation:
> cache file.

## Module layout

New file `src/model_update.rs` (keeps `wrapper.rs` and `settings.rs` focused):

- **Pure core (unit-tested, no I/O):**
  - `fn model_offers(effective: &str, available: &[String], seen: &[String],
    dismissed: &[String]) -> Vec<String>`
  - `fn is_model_newer(candidate: &str, current: &str) -> bool` (same-family
    lexical comparison, reusing the `family_order` idea from `settings.rs`).
- **Effectful shell:**
  - `pub fn maybe_prompt(resolved: &ResolvedSettings) -> ModelDecision` — the
    single entry point `wrap_claude` calls. Runs all guards, loads manifest +
    seen cache, computes offers, seeds on first run, shows the modal, applies
    the chosen action, persists.
  - `enum ModelDecision { UsePinned(String), UseSession(String), NoChange }`.
- **TUI modal:** a small `ratatui` full-screen screen following the
  `settings.rs` pattern (`ratatui::init()` / `ratatui::restore()`), listing the
  offered model(s) with keybindings: `↑↓` select model (if >1),
  `p` pin · `s` session · `d` dismiss · `esc` not now.

Shared helpers currently `fn`-private in `settings.rs`
(`live_available_models`, `family_order`, `newest_sonnet`) become
`pub(crate)` so `model_update` reuses them instead of duplicating.

## Wiring into `wrap_claude`

In `wrapper.rs::wrap_claude`, after `apply_anthropic_api_url` and before
`resolve_model`:

```rust
let user_set_model = args.iter()
    .any(|a| a == "--model" || a.starts_with("--model="));

let model = match (user_set_model, crate::model_update::maybe_prompt(&resolved)) {
    (false, ModelDecision::UsePinned(m)) => m,   // pinned + launch with it
    (false, ModelDecision::UseSession(m)) => m,  // launch with it, not persisted
    _ => resolve_model(resolved.api_model.clone()), // NoChange or explicit --model
};
```

`maybe_prompt` is a no-op returning `NoChange` when non-interactive, no cwd, no
manifest, or the model list is unreachable — so every existing path is
unchanged. When the user set `--model` explicitly we skip the prompt entirely.

## Persistence rules (avoid diff churn)

- The `seen` cache is written **only when the observed list changed**.
- `dismissed_models` / `settings.api_model` are written **only** on an explicit
  Dismiss / Pin action, via the manifest's existing `save` (Pin/Dismiss also
  bump `updated_at`; a plain seen-set update touches only the cache file, never
  the manifest).

## Testing plan

Pure-function unit tests (the bulk, no I/O — mirrors how `settings.rs` and
`update.rs` pin their decision logic):

- `model_offers`:
  - unpinned project, newest Sonnet advanced → offers newest Sonnet.
  - pinned older Sonnet → offers newer Sonnet (Signals 1 & 2 coincide).
  - pinned Opus, newer Opus available → offers newer Opus (Signal 1).
  - pinned Opus, newer Sonnet available → **not** offered (Signal 2 suppressed
    for non-Sonnet pins).
  - brand-new family in `available` not in `seen` → offered (Signal 3).
  - first run (`seen` empty) → Signal 3 suppressed, list returned for seeding.
  - dismissed model excluded; a newer-than-dismissed model still offered.
  - `effective` itself never offered; results deduped.
- `is_model_newer`: within-family ordering, cross-family returns false.
- `ModelDecision` mapping in `wrap_claude` (extend existing `build_claude_args`
  tests): explicit `--model` suppresses; `UsePinned`/`UseSession` set the model.

Effectful paths (manifest load/save round-trip of `dismissed_models`, seen-cache
read/write) covered with `tempfile`, matching existing `config.rs` tests. The
`ratatui` render loop itself is not unit-tested (consistent with `settings.rs`,
whose draw code is also untested); its logic lives in the pure core.

## Out of scope

- Global (cross-project) model recommendations — this is per-project by request.
- A settings-TUI surface for `dismissed_models` (it's bookkeeping; clearing it
  is `whetstone settings` re-pinning, or editing the JSON).
- Changing the default-model resolution order in `resolve_model`.
