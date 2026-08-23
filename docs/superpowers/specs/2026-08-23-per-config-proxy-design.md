# Per-Config Headroom Proxy Reuse — Design

**Date:** 2026-08-23
**Status:** Approved for planning

## Goal

Make the Headroom proxy whetstone selects for a session match *this
project's* resolved Headroom config, instead of silently inheriting
whatever config the first project to launch a proxy baked in. Projects
whose proxy config is identical keep sharing one proxy; a project whose
config diverges gets its own.

## Background: the bug

whetstone runs a Headroom proxy as a **persistent daemon**.
`spawn_proxy_detached` (`src/wrapper.rs`) spawns `headroom proxy` with
null stdio and forgets it; nothing kills it when Claude exits (only the
memory-conflict path ever calls `kill_proxy`). Everything is pinned to a
fixed port `8787` — `DEFAULT_PROXY`, `PROXY_PORT`, `PROXY_HEALTH_URL`,
`build_proxy_args`, and every probe assume it.

`resolve_proxy` probes `127.0.0.1:8787/health`. If *any* proxy answers
(and its memory flag matches what this session wants), it reuses it by
passing `--no-proxy` to `headroom wrap claude`. Only if nothing answers
does it spawn one.

Consequence: the **first project to ever launch headroom owns the proxy
config**, and every subsequent project — sequential *or* concurrent —
reuses it until the proxy dies or the box reboots. It is not a
concurrency race; it is "first launcher wins, indefinitely."

Everything the *proxy* reads is baked in by that first launcher and
bleeds into later projects:

- `ANTHROPIC_TARGET_API_URL` — a custom upstream. Project B's requests
  flow through A's proxy → **A's upstream**.
- `HEADROOM_SAVINGS_PROFILE` — compression aggressiveness.
- `HEADROOM_CODE_AWARE_ENABLED` and any `headroom_env` passthrough key
  (`HEADROOM_RPM`, `HEADROOM_LOG_MESSAGES`, …).
- `HEADROOM_NO_MEMORY_TOOLS` / `HEADROOM_NO_MEMORY_CONTEXT` — if project A
  is ICM and B is Skip, the wrong gating sticks.

What does **not** bleed today: the memory on/off flag (the one thing
`/health` reconciles, via `probe_proxy_health` + `resolve_memory_conflict`)
and `HEADROOM_MEMORY_DB_PATH` (deliberately pinned to a global root, so it
never diverges).

The per-project settings machinery in `src/settings.rs` /
`src/headroom_env.rs` exists precisely so projects *can* diverge (custom
upstream, custom savings profile, raw `headroom_env`). The shared proxy
quietly defeats that half of the feature. The gap is invisible until a
project actually sets a divergent proxy-level config — then it is wrong,
persistently.

## Approach: per-config keyed reuse

Two sessions share a proxy **iff they would spawn a byte-identical
proxy.** whetstone computes a *fingerprint* over exactly the inputs that
determine a spawned proxy's behavior, keeps a registry of the proxies it
has spawned keyed by that fingerprint, reuses a live matching proxy when
one exists, and otherwise spawns a new proxy on its own port and points
this session at it.

This was chosen over strict "one proxy per project directory" because
per-project spawns N daemons even when 10 projects are identical, and
collides head-on with the deliberately-global memory root. Per-config
gives correctness with the minimum number of proxies.

## The fingerprint

`proxy_fingerprint` is a stable hash over the exact `(args, env)`
whetstone would launch the proxy with:

- `build_proxy_args` output — the `--savings-profile`, `--no-telemetry`,
  `--memory` flags.
- `HEADROOM_SAVINGS_PROFILE` (the resolved `required_savings_profile()`).
- `HEADROOM_TELEMETRY` (on/off).
- `ANTHROPIC_TARGET_API_URL` (resolved, if any).
- The resolved `headroom_env_plan().apply` set, **excluding**
  `HEADROOM_MEMORY_DB_PATH` (globally pinned; identical everywhere).

Properties:

- Deterministic: same resolved config → same fingerprint, independent of
  launch order or which project spawned first.
- Sensitive: any change to a proxy-affecting field yields a different
  fingerprint; the memory flag flips it.
- Future-proof: because it hashes the *would-be spawn command*, any new
  proxy knob added later feeds the fingerprint automatically, so this
  class of bug cannot silently reappear.

Fold the memory flag into the fingerprint — it stops being a special case
reconciled via `/health` and becomes just another input.

## The registry

A small JSON file `~/.whetstone/proxies.json`:

```json
[ { "fingerprint": "…", "port": 8801, "pid": 12345 } ]
```

- Guarded by a file lock (advisory `flock` on a sibling lockfile) around
  the whole read → decide → spawn → write critical section, so two
  concurrent launches of the same *new* fingerprint don't both allocate a
  port and spawn.
- Entries are pruned when found dead (see lifecycle).
- Lives beside `~/.whetstone/settings.json`; created lazily.

## Launch flow (replaces `resolve_proxy`)

`resolve_proxy` / `start_detached_decision` / `resolve_memory_conflict`
are replaced by:

1. Compute the fingerprint from resolved settings (memory flag included).
2. Acquire the registry lock.
3. **Prune** dead entries: probe each registered port's `/health`; drop
   any that don't answer.
4. Live entry for this fingerprint?
   - **Yes** → reuse: set `ANTHROPIC_BASE_URL=http://127.0.0.1:<port>`,
     release the lock, and pass `--no-proxy` to `headroom wrap claude`.
   - **No** → allocate a port (see below), spawn `headroom proxy` with
     *this* config's args + env, wait for `/health` to answer within
     `PROXY_READY_TIMEOUT`, record `{fingerprint, port, pid}` in the
     registry, set `ANTHROPIC_BASE_URL` to that port.
5. Release the lock.

The `ProxyDecision` returned to `build_claude_args` collapses to a single
`proxy_ready` bit: whetstone always owns the proxy (reused or freshly
spawned) and always passes `--no-proxy`. The `wrap_memory` fallback path
(letting `headroom wrap` bring up its own session-bound proxy) is only
reached when a spawn fails to come up — same soft-fallback semantics as
today.

### Port allocation (launch-order anchor)

Port **8787** is the *anchor* port, assigned best-effort to whichever
proxy whetstone spawns first:

- Spawning a new proxy: if 8787 is free and unclaimed by a live registry
  entry, use 8787; otherwise take a port from `free_port()`.
- Every fingerprint records its actual assigned port in the registry.

Correctness never depends on which fingerprint holds 8787 — reuse is
always fingerprint → recorded port. 8787's only job is the backward-compat
conveniences, and they only need *a* live proxy there:

- The shell-profile `export ANTHROPIC_BASE_URL=…:8787` (written by
  `src/shell.rs` at setup) still points a bare, non-whetstone `claude`
  launch at a live proxy.
- doctor's `:8787` fast-path (`check_proxy_starts` →
  `ProxyStartCheck::AlreadyRunning`) still recognizes a running proxy as
  proof headroom starts.

(Planning refinement: an earlier draft pinned 8787 to "the default-config
fingerprint," but the ICM-vs-Skip provider gating makes a single canonical
"default config" ill-defined. The launch-order anchor reaches the same
compat goals without computing one.)

## `ANTHROPIC_BASE_URL` ownership change

Today `set_proxy_env` sets `ANTHROPIC_BASE_URL` only when it is unset,
deferring to the shell-profile export. Under this design whetstone
**overrides** it to the port it selected for the session, because
whetstone now owns proxy selection.

This is safe: a user's deliberate custom *upstream* is expressed through
`ANTHROPIC_TARGET_API_URL` (which feeds the fingerprint and is honored by
the spawned proxy), **not** through `ANTHROPIC_BASE_URL`.
`ANTHROPIC_BASE_URL` only ever names the local proxy endpoint, which is
whetstone's to assign. The override applies to the `headroom wrap claude`
exec env; whetstone does not rewrite the shell profile.

## What this removes

- **The memory-conflict prompt.** `resolve_memory_conflict` and its 3-way
  "restart the proxy with memory / session-only / cancel" prompt, plus the
  `kill_proxy` path it drives, exist *only* because there was one shared
  proxy that might lack memory. With the memory flag in the fingerprint,
  memory-on and memory-off are different fingerprints → different proxies.
  A memory session that finds no memory-proxy simply spawns one; it never
  kills or replaces a proxy other sessions may share. Delete the prompt,
  the kill path, and the non-interactive memory-conflict warning.
- **`probe_proxy_health` memory parsing.** Reuse is keyed on the registry,
  not on parsing `config.memory` out of `/health`. A plain liveness probe
  (`probe_proxy` against a specific port) is all the prune/reuse checks
  need. `kill_proxy` may still be retained for `global uninstall` (below).

## Lifecycle / reaping

- **Prune-dead-on-launch:** every launch, while holding the lock, probes
  each registry entry and drops dead ones. Keeps the file from
  accumulating stale entries across reboots.
- **Live proxies persist** (as today) so a divergent-config proxy stays
  available for reuse across sessions.
- **`global uninstall`** kills every registered proxy (by pid) and removes
  the registry file, so teardown doesn't leave orphaned daemons.

**Known limitation (deliberate YAGNI cut):** changing a project's config
strands its old-fingerprint proxy — still alive, now unused — until the
box reboots or `global uninstall` runs. Idle-reaping of live-but-orphaned
proxies is out of scope for v1. Documented, not built.

## doctor and reserved keys

- doctor's `:8787` fast-path and its `free_port()` smoke test are
  unchanged for v1: 8787 remains the default-config port, and the smoke
  test already uses a throwaway free port. A future refinement could check
  the current project's fingerprint-specific proxy; out of scope here.
- `HEADROOM_PORT` stays reserved/denied in `headroom_env.rs`
  (`RESERVED_DENY`). whetstone now allocates ports dynamically, so the
  rationale ("whetstone owns the proxy port") is only more true. No change
  beyond possibly refreshing the warning text.

## Files touched

- **new `src/proxy_registry.rs`** — `proxy_fingerprint`, the registry
  struct, load/save, file lock, prune, and port allocation. Keeps the new
  surface out of `wrapper.rs`.
- **`src/wrapper.rs`** — rewrite `resolve_proxy` and the decision helpers
  around the registry; delete `resolve_memory_conflict` and the memory
  branch of `probe_proxy_health`; make `set_proxy_env` / launch set
  `ANTHROPIC_BASE_URL` to the selected port; `spawn_proxy_detached` takes a
  port argument.
- **`src/headroom_env.rs`** — expose the resolved `apply` set (and the
  proxy-arg/profile/telemetry inputs) in a form the fingerprint can hash.
- **`src/uninstall.rs`** (or wherever `global uninstall` lives) — kill
  registered proxies and remove the registry file.
- **CLAUDE.md** — update the "Global Headroom memory root / single shared
  proxy" design-decision note to describe per-config keyed reuse and the
  8787 default anchor.

## Testing approach

Mostly pure, in `proxy_registry.rs`:

- **Fingerprint stability:** identical resolved config → identical
  fingerprint, regardless of ordering of the `apply` map.
- **Fingerprint sensitivity:** changing the savings profile, the upstream
  URL, a `headroom_env` key, or the memory flag each changes the
  fingerprint; changing only `HEADROOM_MEMORY_DB_PATH` does not.
- **Registry reuse:** a live matching entry is reused (no spawn); a stale
  (dead-port) entry is pruned and a fresh proxy is allocated.
- **Port pinning:** the default fingerprint maps to 8787; a divergent
  fingerprint gets a distinct free port.
- **Concurrency:** two allocations of the same new fingerprint serialize
  on the lock — the second observes the first's registry entry and reuses
  rather than double-spawning. (Tested at the registry API level with a
  stubbed spawn.)
- **Memory prompt gone:** a memory-wanting session with only a
  non-memory proxy live spawns a *second* proxy instead of prompting.
- **`ANTHROPIC_BASE_URL` override:** a non-default fingerprint sets
  `ANTHROPIC_BASE_URL` to the allocated port even when the inherited env
  already had `:8787`.

Health-probe/spawn are injected behind a trait or fn pointer so the
registry logic tests without a real headroom.

## Out of scope

- Strict one-proxy-per-directory isolation (rejected in favor of
  per-config).
- Idle-reaping of orphaned live proxies.
- Per-project memory roots — the global `~/.headroom` memory root and its
  consolidation behavior are unchanged.
- Rewriting the shell-profile `ANTHROPIC_BASE_URL` export.
