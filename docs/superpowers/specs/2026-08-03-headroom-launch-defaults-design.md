# Headroom Launch Defaults + Overrides — Design

**Date:** 2026-08-03
**Status:** Approved for planning

## Goal

Give whetstone sane, opinionated defaults for the Headroom proxy it
launches, and let the end user override every one of them via
`whetstone settings` (layered global/project), by `HEADROOM_*` env var, or
via a raw passthrough escape hatch for the long tail of knobs.

## Background

whetstone launches Headroom two ways:

1. **Detached proxy** — `spawn_proxy_detached` builds a background
   `headroom proxy` process. Env must be set explicitly per-var via
   `.env(...)` on the `Command`.
2. **Wrap exec** — `headroom wrap claude` via Unix `exec`, which inherits
   whetstone's own process env, so `env::set_var(...)` before the exec is
   sufficient.

Headroom exposes ~50 runtime knobs, all backed by `HEADROOM_*` env vars.
whetstone already applies this pattern for two of them:

- `HEADROOM_SAVINGS_PROFILE` via `required_savings_profile()` /
  `resolve_savings_profile()`, defaulting to `agent-90`.
- `ANTHROPIC_TARGET_API_URL` via the **Anthropic API URL** setting, where
  an externally-set env var already takes precedence over the stored value.

This feature generalizes that pattern to a curated set of first-class
settings plus a raw passthrough map.

## Resolution model

For every managed knob, resolve a single effective value with this
precedence (highest first):

1. External `HEADROOM_*` (or `ANTHROPIC_*`) env var already set in the
   environment whetstone inherits.
2. Project setting (`.claude/whetstone.json` `settings` block).
3. Global setting (`~/.whetstone/settings.json`).
4. whetstone built-in default.

The resolved value is applied at **both** launch sinks:

- `.env(KEY, value)` on the detached proxy `Command`.
- `env::set_var(KEY, value)` before the `headroom wrap claude` exec.

External-env-wins is implemented by **not** setting the var when it is
already present in the inherited environment (the proxy/wrap process then
sees the user's value unchanged), mirroring the existing
`apply_anthropic_api_url` behavior.

## First-class settings

Exposed in `whetstone settings`, each scoped Off / Global / Project, with
project-over-global precedence (existing `ResolvedSettings` machinery).

| Setting | Env var | whetstone default | Notes |
|---|---|---|---|
| Savings profile | `HEADROOM_SAVINGS_PROFILE` | `agent-90` | Already wired; folds into this model. |
| Target keep-ratio | `HEADROOM_TARGET_RATIO` | **unset** | Adaptive; the profile drives it. Do not pin a literal. |
| Code-aware | `HEADROOM_CODE_AWARE_ENABLED` | **on** | Valid because default extras include `code`. |
| Log message content | `HEADROOM_LOG_MESSAGES` | **off** | Opinionated flip — Headroom ships it on; whetstone writes source + secrets to disk otherwise. |
| Budget (USD) | `HEADROOM_BUDGET` | unset | |
| Budget period | (Headroom period var) | `hourly` | Only meaningful when a budget is set. |
| Requests/min | (Headroom rpm var) | unset | Single local user needs no self-throttle. |
| Tokens/min | (Headroom tpm var) | unset | |
| Upstream backend | `HEADROOM_BACKEND` (or equiv) | `anthropic` | |
| any-llm provider | (Headroom var) | passthrough | Only bites non-anthropic backends. |
| Cloud region | (Headroom var) | passthrough | Only bites non-anthropic backends. |
| Anthropic API URL | `ANTHROPIC_TARGET_API_URL` | **existing setting** | Reconcile — do NOT create a second knob for Headroom's "Anthropic base URL". |

> Exact Headroom env-var names for budget period, rpm/tpm, backend,
> any-llm provider, and region must be confirmed against Headroom's env
> reference during planning before they are hardcoded.

## Provider-gated default: Headroom memory tools

whetstone's memory story is ICM. Headroom also ships its own memory tools
(`memory_save` / `memory_search`) and context injection, gated behind the
`--memory` flag. To avoid double-tooling when ICM is the provider:

- Manifest provider == `Icm` → default **Disable memory tools** and
  **Disable memory context injection** to **on** (ICM owns memory).
- Manifest provider == `Skip` / none → leave Headroom's own defaults
  (memory active when `--memory` is passed).
- Both remain overrideable per-project/global and via `HEADROOM_*` env.

## Raw passthrough escape

Everything not first-classed is handled by a raw `headroom_env` map on
`ProjectSettings` / `GlobalSettings`:

- Type: `Map<String, String>` (serde `default`, `skip_serializing_if`
  empty).
- Resolved project-over-global, then merged into the launch env at both
  sinks.
- An external `HEADROOM_*` env var still wins over a map entry.

This covers the entire Advanced-tab long tail (HTTP/2, connection pools,
CCR maturation, timeouts, memory paths/top-K, extensions, OpenAI base URL
+ extra headers, etc.) without a dedicated UI field for each.

## Out of scope / whetstone-owned

- **Port** stays hardwired to `8787`. Verified coupling: `wrapper.rs`
  `DEFAULT_PROXY` (:9), `PROXY_HEALTH_URL` (:10), `build_proxy_args`
  (`--port 8787`), and the health probes all assume it. Exposing an
  override without threading the new port through every probe would make
  whetstone launch on one port and health-check another. Not user-facing
  in this feature.
- **Host** stays `127.0.0.1` (local-only; safe default). Available via the
  raw map for advanced users who understand the exposure.

## Testing approach

- Unit tests for the resolution function: env > project > global >
  default, for a representative first-class knob and for a raw-map entry.
- Unit test: external `HEADROOM_*` env var is left untouched (not
  overwritten) at both sinks.
- Unit test: provider-gated memory default flips with `Icm` vs `Skip`.
- Unit tests assert `build_proxy_args` / the detached-proxy env set and
  the pre-exec `set_var` calls carry the resolved values.
- Settings TUI: existing layered-settings tests extended for the new
  first-class fields.

## Open confirmations for planning

1. Exact Headroom env-var names for the non-obvious first-class knobs
   (budget period, rpm/tpm, backend, any-llm provider, region).
2. Confirm Headroom's "Anthropic base URL" endpoint maps to the same
   upstream override as `ANTHROPIC_TARGET_API_URL` (so we truly avoid a
   duplicate knob).
3. Confirm the env-var names for Headroom's memory disables.
