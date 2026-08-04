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

All env-var names below were confirmed against `headroom proxy --help`.

| Setting | Env var | whetstone default | Notes |
|---|---|---|---|
| Savings profile | `HEADROOM_SAVINGS_PROFILE` | `agent-90` | Already wired; folds into this model. |
| Target keep-ratio | `HEADROOM_TARGET_RATIO` | **unset** | Adaptive; the profile drives it. Do not pin a literal. |
| Code-aware | `HEADROOM_CODE_AWARE_ENABLED` | **on** (`1`) | Opinionated default — right for a coding tool. Valid because default extras include `code`; see caveat below. |
| Log message content | `HEADROOM_LOG_MESSAGES` | **off (unset)** | NOT a flip — Headroom's `--log-messages` is an enable flag that already defaults off. First-class opt-in; default = leave the var unset. |
| Budget (USD) | `HEADROOM_BUDGET` | unset | |
| Budget period | `HEADROOM_BUDGET_PERIOD` | unset | Only meaningful when a budget is set; leave to Headroom's default rather than pinning `hourly`. |
| Requests/min | `HEADROOM_RPM` | unset | Single local user needs no self-throttle. |
| Tokens/min | `HEADROOM_TPM` | unset | |
| Upstream backend | `HEADROOM_BACKEND` | unset (`anthropic`) | Headroom already defaults to anthropic; leave unset. |
| any-llm provider | `HEADROOM_ANYLLM_PROVIDER` | unset | Only bites non-anthropic backends. |
| Cloud region | `HEADROOM_REGION` | unset | Only bites non-anthropic backends. |
| Anthropic API URL | `ANTHROPIC_TARGET_API_URL` | **existing setting** | Confirmed: Headroom's `--anthropic-api-url` reads exactly this var. Reuse whetstone's existing `anthropic_api_url` — do NOT create a second knob. |

### Opinionated defaults whetstone actively sets

The audit found only **two** knobs where whetstone imposes a value that
differs from "leave Headroom alone":

1. `HEADROOM_SAVINGS_PROFILE=agent-90` — already required at launch.
2. `HEADROOM_CODE_AWARE_ENABLED=1` — AST-aware compression, right for a
   coding tool.

Everything else stays unset (Headroom's own default / adaptive) unless the
user sets it. In particular, **log message content is NOT flipped** — it is
already off by default in Headroom.

**Code-aware caveat:** `HEADROOM_CODE_AWARE_ENABLED=1` requires the
`headroom-ai[code]` extra. whetstone's default extras include `code`, but a
user who installed with `--headroom-extras none` won't have it. Planning
must decide whether to gate this default on the presence of the extra or
accept Headroom warning/ignoring it. Simplest: set it unconditionally and
let Headroom no-op when the extra is absent (confirm Headroom degrades
gracefully rather than erroring).

## Provider-gated default: Headroom memory tools

whetstone's memory story is ICM. Headroom also ships its own memory tools
(`memory_save` / `memory_search`) and context injection, gated behind the
`--memory` flag. To avoid double-tooling when ICM is the provider:

- Manifest provider == `Icm` → set `HEADROOM_NO_MEMORY_TOOLS=1` and
  `HEADROOM_NO_MEMORY_CONTEXT=1` (ICM owns memory).
- Manifest provider == `Skip` / none → leave both unset (Headroom's own
  defaults; memory active when `--memory` is passed).
- Both remain overrideable per-project/global and via `HEADROOM_*` env.

This is the one *provider-conditional* opinionated default, distinct from
the two unconditional ones above.

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

## Confirmed against `headroom proxy --help`

All open questions from brainstorming are now resolved:

1. Env-var names for every first-class knob confirmed:
   `HEADROOM_TARGET_RATIO`, `HEADROOM_CODE_AWARE_ENABLED`,
   `HEADROOM_LOG_MESSAGES`, `HEADROOM_BUDGET`, `HEADROOM_BUDGET_PERIOD`,
   `HEADROOM_RPM`, `HEADROOM_TPM`, `HEADROOM_BACKEND`,
   `HEADROOM_ANYLLM_PROVIDER`, `HEADROOM_REGION`.
2. Headroom's `--anthropic-api-url` reads `ANTHROPIC_TARGET_API_URL` —
   same var whetstone already exports. No duplicate knob.
3. Memory disables: `HEADROOM_NO_MEMORY_TOOLS`,
   `HEADROOM_NO_MEMORY_CONTEXT`.
4. `--log-messages` is an enable flag (default off) — corrected the spec's
   original "flip it off" premise.

Remaining decision deferred to the plan: whether to gate
`HEADROOM_CODE_AWARE_ENABLED=1` on the `[code]` extra being installed (see
code-aware caveat).
