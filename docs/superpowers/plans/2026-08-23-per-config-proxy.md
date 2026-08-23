# Per-Config Headroom Proxy Reuse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Key Headroom proxy reuse on a config fingerprint so a project's per-project proxy settings (upstream URL, savings profile, telemetry, `headroom_env`, memory) stop bleeding across projects that share the single fixed-port proxy.

**Architecture:** A new `src/proxy_registry.rs` computes a stable fingerprint over the exact config that would be spawned, keeps a lockfile-guarded registry (`~/.whetstone/proxies.json`) mapping fingerprint → `{port, pid}`, and resolves each launch to either a reused live proxy or a freshly-spawned one on its own port. `src/wrapper.rs` calls this instead of the old single-port `resolve_proxy`, points `ANTHROPIC_BASE_URL` at the selected port, and drops the now-dead memory-conflict prompt. Port 8787 is a best-effort launch-order anchor for backward compatibility.

**Tech Stack:** Rust, `serde`/`serde_json`, `ureq` (health probe), std `TcpListener` (free-port + port-free checks), std `fs` (registry + lockfile). No new dependencies — FNV-1a fingerprint and an `O_EXCL` lockfile guard are hand-rolled.

**Spec:** `docs/superpowers/specs/2026-08-23-per-config-proxy-design.md`

## Global Constraints

- No new crate dependencies — fingerprint hashing and file locking are implemented in-repo.
- `HEADROOM_MEMORY_DB_PATH` is excluded from the fingerprint (globally pinned; identical everywhere).
- `HEADROOM_PORT` stays reserved/denied in `src/headroom_env.rs` `RESERVED_DENY` — unchanged.
- Verify every task with `cargo build && cargo test && cargo clippy` (per CLAUDE.md). `just test` / `just lint` also work.
- Existing external-env-wins precedence and the global `~/.headroom` memory root are unchanged.
- Match the surrounding file style: `anyhow::Result` with `.context(...)`, `ui::` helpers for user output, module-level unit tests in a `#[cfg(test)] mod tests`.

---

### Task 1: Fingerprint (`ProxySpec` + `proxy_fingerprint`)

**Files:**
- Create: `src/proxy_registry.rs`
- Modify: `src/main.rs` (add `mod proxy_registry;`)

**Interfaces:**
- Produces:
  - `pub struct ProxySpec { pub savings_profile: String, pub telemetry: bool, pub memory: bool, pub anthropic_api_url: Option<String>, pub env: std::collections::BTreeMap<String, String> }`
  - `pub fn proxy_fingerprint(spec: &ProxySpec) -> String` — 16-hex-char stable hash.

- [ ] **Step 1: Register the module**

In `src/main.rs`, add alongside the other `mod` declarations:

```rust
mod proxy_registry;
```

- [ ] **Step 2: Write the failing tests**

Create `src/proxy_registry.rs` with only the type, a stub, and the tests:

```rust
//! Per-config Headroom proxy registry: fingerprint the exact proxy config a
//! session would spawn, so two sessions share a proxy iff they would spawn a
//! byte-identical one. Keyed reuse via `~/.whetstone/proxies.json`.

use std::collections::BTreeMap;

/// Everything that varies a spawned Headroom proxy's behavior. Two specs with
/// the same fingerprint may share one proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// FNV-1a 64-bit over a canonical rendering of the spec.
pub fn proxy_fingerprint(_spec: &ProxySpec) -> String {
    unimplemented!()
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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib proxy_registry`
Expected: FAIL — `proxy_fingerprint` panics with `unimplemented!()`.

- [ ] **Step 4: Implement the fingerprint**

Replace the `proxy_fingerprint` stub:

```rust
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

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn proxy_fingerprint(spec: &ProxySpec) -> String {
    format!("{:016x}", fnv1a(canonical(spec).as_bytes()))
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib proxy_registry`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add src/proxy_registry.rs src/main.rs
git commit -m "feat(proxy): add ProxySpec config fingerprint"
```

---

### Task 2: Registry struct + load/save

**Files:**
- Modify: `src/proxy_registry.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub struct ProxyEntry { pub fingerprint: String, pub port: u16, pub pid: u32 }` (derives `Clone, Debug, PartialEq, Eq, Serialize, Deserialize`).
  - `pub struct Registry { pub entries: Vec<ProxyEntry> }` (derives `Default, Debug, Serialize, Deserialize`).
  - `impl Registry`: `pub fn load(path: &std::path::Path) -> Self`, `pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()>`, `pub fn find(&self, fingerprint: &str) -> Option<&ProxyEntry>`.
  - `pub fn registry_path() -> Option<std::path::PathBuf>` → `~/.whetstone/proxies.json`.

- [ ] **Step 1: Write the failing tests**

Add imports at the top of `src/proxy_registry.rs` (merge with existing `use`):

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
```

Add these tests inside `mod tests`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib proxy_registry`
Expected: FAIL — `Registry` / `ProxyEntry` undefined.

- [ ] **Step 3: Implement the registry types**

Add to `src/proxy_registry.rs` (outside `mod tests`):

```rust
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
pub struct Registry {
    #[serde(default)]
    pub entries: Vec<ProxyEntry>,
}

/// `~/.whetstone/proxies.json`, or `None` if the home dir can't be found.
pub fn registry_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(GLOBAL_DIR).join(REGISTRY_FILENAME))
}

impl Registry {
    /// Load the registry, treating a missing or corrupt file as empty — a
    /// stale registry must never block a launch.
    pub fn load(path: &Path) -> Self {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

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

    pub fn find(&self, fingerprint: &str) -> Option<&ProxyEntry> {
        self.entries.iter().find(|e| e.fingerprint == fingerprint)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib proxy_registry`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/proxy_registry.rs
git commit -m "feat(proxy): add proxy registry load/save"
```

---

### Task 3: Lockfile guard

**Files:**
- Modify: `src/proxy_registry.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub struct LockGuard { path: PathBuf }` with `Drop` removing the file.
  - `pub fn acquire_lock(path: &Path, timeout: std::time::Duration, stale_after: std::time::Duration) -> Result<LockGuard>` — spins on `O_EXCL` create; steals a lockfile older than `stale_after`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` (add `use std::time::Duration;` to the test module or fully-qualify):

```rust
    #[test]
    fn lock_is_exclusive_then_released_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("proxies.lock");
        let short = std::time::Duration::from_millis(200);
        let stale = std::time::Duration::from_secs(60);

        let g = acquire_lock(&lock, short, stale).unwrap();
        // Second acquisition times out while the first is held.
        assert!(acquire_lock(&lock, short, stale).is_err());
        drop(g);
        // After release, it succeeds again.
        assert!(acquire_lock(&lock, short, stale).is_ok());
    }

    #[test]
    fn stale_lock_is_stolen() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("proxies.lock");
        std::fs::write(&lock, "old").unwrap();
        // stale_after = 0 → any existing lock is immediately stealable.
        let g = acquire_lock(
            &lock,
            std::time::Duration::from_millis(200),
            std::time::Duration::from_secs(0),
        );
        assert!(g.is_ok());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib proxy_registry`
Expected: FAIL — `acquire_lock` / `LockGuard` undefined.

- [ ] **Step 3: Implement the lock**

Add to `src/proxy_registry.rs`:

```rust
use std::time::{Duration, Instant};

/// Advisory lockfile guard. Holds an `O_EXCL`-created file for the duration of
/// a registry critical section; removes it on drop.
pub struct LockGuard {
    path: PathBuf,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Acquire the registry lock, spinning until it's free or `timeout` elapses. A
/// lockfile older than `stale_after` (a crashed holder) is stolen.
pub fn acquire_lock(
    path: &Path,
    timeout: Duration,
    stale_after: Duration,
) -> Result<LockGuard> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let deadline = Instant::now() + timeout;
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(_) => return Ok(LockGuard { path: path.to_path_buf() }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if lock_is_stale(path, stale_after) {
                    let _ = std::fs::remove_file(path);
                    continue;
                }
                if Instant::now() >= deadline {
                    anyhow::bail!(
                        "timed out acquiring proxy registry lock at {}",
                        path.display()
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!("opening lock {}", path.display())
                })
            }
        }
    }
}

fn lock_is_stale(path: &Path, stale_after: Duration) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    modified.elapsed().map(|age| age >= stale_after).unwrap_or(false)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib proxy_registry`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/proxy_registry.rs
git commit -m "feat(proxy): add lockfile guard for the registry"
```

---

### Task 4: Port allocation (launch-order 8787 anchor)

**Files:**
- Modify: `src/proxy_registry.rs`

**Interfaces:**
- Consumes: `Registry`.
- Produces:
  - `pub const ANCHOR_PORT: u16 = 8787;`
  - `pub fn choose_port(reg: &Registry, port_free: impl Fn(u16) -> bool, free_port: impl Fn() -> Option<u16>) -> Option<u16>` — returns 8787 when it's free and unclaimed, else a fresh free port.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    fn entry(fp: &str, port: u16) -> ProxyEntry {
        ProxyEntry { fingerprint: fp.into(), port, pid: 1 }
    }

    #[test]
    fn choose_port_prefers_anchor_when_free_and_unclaimed() {
        let reg = Registry::default();
        let port = choose_port(&reg, |_| true, || Some(9001));
        assert_eq!(port, Some(ANCHOR_PORT));
    }

    #[test]
    fn choose_port_skips_anchor_when_claimed_by_a_live_entry() {
        let reg = Registry { entries: vec![entry("x", ANCHOR_PORT)] };
        let port = choose_port(&reg, |_| true, || Some(9002));
        assert_eq!(port, Some(9002));
    }

    #[test]
    fn choose_port_skips_anchor_when_port_busy() {
        let reg = Registry::default();
        // Anchor reported busy by the OS even though no entry claims it.
        let port = choose_port(&reg, |p| p != ANCHOR_PORT, || Some(9003));
        assert_eq!(port, Some(9003));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib proxy_registry`
Expected: FAIL — `choose_port` / `ANCHOR_PORT` undefined.

- [ ] **Step 3: Implement**

Add to `src/proxy_registry.rs`:

```rust
/// Backward-compat anchor port. Assigned best-effort to the first proxy
/// whetstone spawns so a bare `claude` launch and doctor's fast-path still
/// find a proxy at `127.0.0.1:8787`.
pub const ANCHOR_PORT: u16 = 8787;

/// Pick a port for a new proxy: the anchor when it's free and unclaimed,
/// otherwise a fresh OS-assigned free port.
pub fn choose_port(
    reg: &Registry,
    port_free: impl Fn(u16) -> bool,
    free_port: impl Fn() -> Option<u16>,
) -> Option<u16> {
    let anchor_claimed = reg.entries.iter().any(|e| e.port == ANCHOR_PORT);
    if !anchor_claimed && port_free(ANCHOR_PORT) {
        return Some(ANCHOR_PORT);
    }
    free_port()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib proxy_registry`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/proxy_registry.rs
git commit -m "feat(proxy): launch-order 8787 port anchor"
```

---

### Task 5: Prune + resolve orchestrator

**Files:**
- Modify: `src/proxy_registry.rs`

**Interfaces:**
- Consumes: `Registry`, `ProxySpec`, `proxy_fingerprint`, `choose_port`.
- Produces:
  - `pub enum ProxyOutcome { Reused(u16), Spawned(u16), Failed }`
  - `pub struct ResolveDeps<'a> { pub probe: &'a dyn Fn(u16) -> bool, pub port_free: &'a dyn Fn(u16) -> bool, pub free_port: &'a dyn Fn() -> Option<u16>, pub spawn: &'a dyn Fn(u16, u32) -> Option<u32> }`
    - `spawn(port, existing_pid_placeholder)`: spawn a proxy for the current spec on `port`, poll to readiness; return `Some(pid)` on success, `None` on failure. (The second arg is unused — see note; kept `u32` to avoid a lifetime-bearing closure type. **Correction:** use the simpler signature below.)
  - `pub fn resolve(reg: &mut Registry, spec: &ProxySpec, deps: &ResolveDeps) -> ProxyOutcome` — mutates `reg` (prunes dead, records a spawn); caller persists it.

> Interface correction (authoritative): `spawn` takes only the port:
> `pub spawn: &'a dyn Fn(u16) -> Option<u32>` returning the new pid on success.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    struct Stub {
        alive: std::cell::RefCell<Vec<u16>>, // ports that answer /health
        spawned: std::cell::RefCell<Vec<u16>>,
        next_free: u16,
    }

    fn deps_from<'a>(
        probe: &'a dyn Fn(u16) -> bool,
        port_free: &'a dyn Fn(u16) -> bool,
        free_port: &'a dyn Fn() -> Option<u16>,
        spawn: &'a dyn Fn(u16) -> Option<u32>,
    ) -> ResolveDeps<'a> {
        ResolveDeps { probe, port_free, free_port, spawn }
    }

    #[test]
    fn resolve_reuses_a_live_matching_proxy() {
        let spec = base();
        let fp = proxy_fingerprint(&spec);
        let mut reg = Registry { entries: vec![ProxyEntry { fingerprint: fp, port: 8801, pid: 7 }] };

        let probe = |_p: u16| true; // 8801 answers
        let port_free = |_p: u16| true;
        let free_port = || Some(9100u16);
        let spawn = |_p: u16| -> Option<u32> { panic!("must not spawn on reuse") };
        let deps = deps_from(&probe, &port_free, &free_port, &spawn);

        let outcome = resolve(&mut reg, &spec, &deps);
        assert_eq!(outcome, ProxyOutcome::Reused(8801));
        assert_eq!(reg.entries.len(), 1);
    }

    #[test]
    fn resolve_prunes_dead_entry_and_spawns() {
        let spec = base();
        let fp = proxy_fingerprint(&spec);
        let mut reg = Registry { entries: vec![ProxyEntry { fingerprint: fp.clone(), port: 8801, pid: 7 }] };

        let probe = |_p: u16| false; // nothing answers → 8801 is dead
        let port_free = |_p: u16| true;
        let free_port = || Some(9100u16);
        let spawned: std::cell::Cell<Option<u16>> = std::cell::Cell::new(None);
        let spawn = |p: u16| -> Option<u32> { spawned.set(Some(p)); Some(555) };
        let deps = deps_from(&probe, &port_free, &free_port, &spawn);

        let outcome = resolve(&mut reg, &spec, &deps);
        // Dead 8801 pruned; anchor free & unclaimed → spawns on 8787.
        assert_eq!(outcome, ProxyOutcome::Spawned(ANCHOR_PORT));
        assert_eq!(spawned.get(), Some(ANCHOR_PORT));
        assert_eq!(reg.find(&fp).map(|e| (e.port, e.pid)), Some((ANCHOR_PORT, 555)));
    }

    #[test]
    fn resolve_spawns_new_port_when_anchor_taken() {
        let spec = base();
        let fp = proxy_fingerprint(&spec);
        // A different fingerprint holds a live anchor.
        let mut reg = Registry { entries: vec![ProxyEntry { fingerprint: "other".into(), port: ANCHOR_PORT, pid: 1 }] };

        let probe = |_p: u16| true; // anchor's holder is alive
        let port_free = |_p: u16| true;
        let free_port = || Some(9100u16);
        let spawn = |_p: u16| -> Option<u32> { Some(556) };
        let deps = deps_from(&probe, &port_free, &free_port, &spawn);

        let outcome = resolve(&mut reg, &spec, &deps);
        assert_eq!(outcome, ProxyOutcome::Spawned(9100));
        assert_eq!(reg.find(&fp).map(|e| e.port), Some(9100));
    }

    #[test]
    fn resolve_reports_failed_when_spawn_fails() {
        let spec = base();
        let mut reg = Registry::default();
        let probe = |_p: u16| false;
        let port_free = |_p: u16| true;
        let free_port = || Some(9100u16);
        let spawn = |_p: u16| -> Option<u32> { None };
        let deps = deps_from(&probe, &port_free, &free_port, &spawn);

        assert_eq!(resolve(&mut reg, &spec, &deps), ProxyOutcome::Failed);
        assert!(reg.find(&proxy_fingerprint(&spec)).is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib proxy_registry`
Expected: FAIL — `resolve` / `ProxyOutcome` / `ResolveDeps` undefined.

- [ ] **Step 3: Implement**

Add to `src/proxy_registry.rs`:

```rust
/// The result of resolving a launch to a proxy port.
#[derive(Debug, PartialEq, Eq)]
pub enum ProxyOutcome {
    /// An already-running proxy matched the fingerprint.
    Reused(u16),
    /// A fresh proxy was spawned on this port.
    Spawned(u16),
    /// No proxy could be brought up; caller should soft-fall-back.
    Failed,
}

/// Injected side effects so `resolve` is unit-testable without a real proxy.
pub struct ResolveDeps<'a> {
    /// Does a proxy answer `/health` on this port?
    pub probe: &'a dyn Fn(u16) -> bool,
    /// Can we bind this port right now?
    pub port_free: &'a dyn Fn(u16) -> bool,
    /// Ask the OS for an unused port.
    pub free_port: &'a dyn Fn() -> Option<u16>,
    /// Spawn a proxy for the current spec on this port; return its pid when it
    /// comes up ready, else `None`.
    pub spawn: &'a dyn Fn(u16) -> Option<u32>,
}

/// Prune dead entries, reuse a live matching proxy, or spawn a new one.
/// Mutates `reg`; the caller persists it under the lock.
pub fn resolve(
    reg: &mut Registry,
    spec: &ProxySpec,
    deps: &ResolveDeps,
) -> ProxyOutcome {
    reg.entries.retain(|e| (deps.probe)(e.port));

    let fingerprint = proxy_fingerprint(spec);
    if let Some(entry) = reg.find(&fingerprint) {
        return ProxyOutcome::Reused(entry.port);
    }

    let Some(port) = choose_port(reg, deps.port_free, deps.free_port) else {
        return ProxyOutcome::Failed;
    };
    match (deps.spawn)(port) {
        Some(pid) => {
            reg.entries.push(ProxyEntry { fingerprint, port, pid });
            ProxyOutcome::Spawned(port)
        }
        None => ProxyOutcome::Failed,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib proxy_registry`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/proxy_registry.rs
git commit -m "feat(proxy): prune + reuse/spawn resolve orchestrator"
```

---

### Task 6: Wire the registry into `wrapper.rs` + docs

This task replaces the single-port proxy logic with the registry, removes the memory-conflict prompt, and updates CLAUDE.md. It has no new pure unit under test of its own beyond a `build_proxy_spec` helper; the behavior is covered by the registry tests plus a `build_proxy_spec` test and the existing `build_claude_args` tests. Verification is a green `cargo test` for the whole crate with the four deleted `parse_proxy_health` tests gone.

**Files:**
- Modify: `src/wrapper.rs`
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: `crate::proxy_registry::{ProxySpec, Registry, ResolveDeps, ProxyOutcome, registry_path, acquire_lock, ANCHOR_PORT, proxy_fingerprint}`.
- Produces: internal helpers `build_proxy_spec`, `probe_port`, `select_proxy_port`; changed signatures `spawn_proxy_detached(port: &str, memory: bool)` and `spawn_proxy_ready(port: &str, memory: bool) -> Option<u32>`.

- [ ] **Step 1: Add the `build_proxy_spec` failing test**

In `src/wrapper.rs` `mod tests`, add:

```rust
    #[test]
    fn build_proxy_spec_excludes_memory_db_path() {
        use std::collections::BTreeMap;
        let mut apply = vec![
            ("HEADROOM_CODE_AWARE_ENABLED".to_string(), "1".to_string()),
            (
                "HEADROOM_MEMORY_DB_PATH".to_string(),
                "/home/u/.headroom/memory.db".to_string(),
            ),
        ];
        // Sorted apply set is fine; the helper filters by key.
        apply.sort();
        let spec = build_proxy_spec_from(
            "agent-90".to_string(),
            true,  // telemetry
            true,  // memory
            Some("https://up.test".to_string()),
            &apply,
        );
        assert_eq!(spec.savings_profile, "agent-90");
        assert!(spec.telemetry);
        assert!(spec.memory);
        assert_eq!(spec.anthropic_api_url.as_deref(), Some("https://up.test"));
        assert!(spec.env.contains_key("HEADROOM_CODE_AWARE_ENABLED"));
        assert!(!spec.env.contains_key("HEADROOM_MEMORY_DB_PATH"));
        let _ = BTreeMap::<String, String>::new();
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --lib build_proxy_spec_excludes_memory_db_path`
Expected: FAIL — `build_proxy_spec_from` undefined.

- [ ] **Step 3: Add the pure spec-builder**

In `src/wrapper.rs`, add (near `headroom_env_plan`):

```rust
/// Testable core: assemble a `ProxySpec` from resolved inputs + the apply set.
fn build_proxy_spec_from(
    savings_profile: String,
    telemetry: bool,
    memory: bool,
    anthropic_api_url: Option<String>,
    apply: &[(String, String)],
) -> crate::proxy_registry::ProxySpec {
    let env = apply
        .iter()
        .filter(|(k, _)| k != "HEADROOM_MEMORY_DB_PATH")
        .cloned()
        .collect();
    crate::proxy_registry::ProxySpec {
        savings_profile,
        telemetry,
        memory,
        anthropic_api_url: anthropic_api_url
            .filter(|s| !s.trim().is_empty()),
        env,
    }
}

/// Production wrapper: reads live settings, the applied env plan, and the
/// effective upstream URL.
fn build_proxy_spec(
    resolved: &crate::config::ResolvedSettings,
    hr_plan: &crate::headroom_env::HeadroomEnvPlan,
    memory: bool,
) -> crate::proxy_registry::ProxySpec {
    build_proxy_spec_from(
        required_savings_profile(),
        resolved.headroom_telemetry,
        memory,
        env::var(ANTHROPIC_TARGET_API_URL).ok(),
        &hr_plan.apply,
    )
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test --lib build_proxy_spec_excludes_memory_db_path`
Expected: PASS.

- [ ] **Step 5: Generalize probe + spawn to a port, add the file-level selector**

In `src/wrapper.rs`:

Replace `fn probe_proxy() -> bool { ... }` with a port-parameterized probe plus a compat wrapper on the anchor:

```rust
fn probe_port(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/health");
    ureq::get(&url).timeout(PROXY_PROBE_TIMEOUT).call().is_ok()
}

fn probe_proxy() -> bool {
    probe_port(crate::proxy_registry::ANCHOR_PORT)
}
```

Change `spawn_proxy_detached` to take a port (replace its `PROXY_PORT` use):

```rust
fn spawn_proxy_detached(port: &str, memory: bool) -> std::io::Result<()> {
```

and inside it pass `port` to `build_proxy_args(port, ...)` instead of `PROXY_PORT`.

Add a port-aware readiness spawn (generalizing the old `start_proxy_detached_ready`):

```rust
/// Spawn a proxy on `port` and poll until it answers or times out. Returns the
/// child pid on success (read back from the port's own report is unnecessary —
/// we return the spawned pid), else `None`.
fn spawn_proxy_ready(port: u16, memory: bool) -> Option<u32> {
    let pid = spawn_proxy_detached_pid(&port.to_string(), memory)?;
    let deadline = Instant::now() + PROXY_READY_TIMEOUT;
    while Instant::now() < deadline {
        if probe_port(port) {
            return Some(pid);
        }
        std::thread::sleep(PROXY_POLL_INTERVAL);
    }
    eprintln!(
        "[WARN] whetstone: proxy on 127.0.0.1:{port} did not respond in time"
    );
    None
}
```

Change `spawn_proxy_detached` to return the child pid so `spawn_proxy_ready` can record it — rename the spawning body to `spawn_proxy_detached_pid`:

```rust
fn spawn_proxy_detached_pid(port: &str, memory: bool) -> Option<u32> {
    // ...existing body of spawn_proxy_detached, but:
    //   let child = cmd.spawn().ok()?;
    //   Some(child.id())
}
```

(Delete the old `spawn_proxy_detached` returning `io::Result<()>`; nothing else calls it once Step 6 lands.)

Add the file-level selector that locks, loads, resolves, saves:

```rust
/// Lock the registry, reconcile it against the running proxies, and return the
/// port this session should use (spawning one if needed). Falls back to `None`
/// so the caller can let `headroom wrap` try its own proxy.
fn select_proxy_port(spec: &crate::proxy_registry::ProxySpec) -> Option<u16> {
    use crate::proxy_registry as reg;
    let path = reg::registry_path()?;
    let lock_path = path.with_extension("lock");
    // Lock window must outlast a full readiness wait.
    let _guard = reg::acquire_lock(
        &lock_path,
        Duration::from_secs(25),
        Duration::from_secs(45),
    )
    .ok()?;

    let mut registry = reg::Registry::load(&path);
    let probe = |p: u16| probe_port(p);
    let port_free = |p: u16| free_port_is_bindable(p);
    let free = || free_port();
    let spawn = |p: u16| spawn_proxy_ready(p, spec.memory);
    let deps = reg::ResolveDeps {
        probe: &probe,
        port_free: &port_free,
        free_port: &free,
        spawn: &spawn,
    };

    let outcome = reg::resolve(&mut registry, spec, &deps);
    let _ = registry.save(&path);
    match outcome {
        reg::ProxyOutcome::Reused(p) | reg::ProxyOutcome::Spawned(p) => Some(p),
        reg::ProxyOutcome::Failed => None,
    }
}

/// Can we bind this specific port right now?
fn free_port_is_bindable(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}
```

- [ ] **Step 6: Replace `resolve_proxy` and delete the memory-conflict path**

In `wrap_claude`, replace:

```rust
    let want_memory = memory_flag || resolved.headroom_memory;
    let decision = resolve_proxy(want_memory);
```

with:

```rust
    let want_memory = memory_flag || resolved.headroom_memory;
    let spec = build_proxy_spec(&resolved, &hr_plan, want_memory);
    let selected = select_proxy_port(&spec);
    if let Some(port) = selected {
        env::set_var(
            "ANTHROPIC_BASE_URL",
            format!("http://127.0.0.1:{port}"),
        );
    }
    let proxy_ready = selected.is_some();
    let wrap_memory = if proxy_ready { false } else { want_memory };
```

Then update the `build_claude_args(...)` call to pass `proxy_ready` and `wrap_memory` (replacing `decision.proxy_ready` / `decision.wrap_memory`).

Delete these items entirely (and their now-orphaned helpers):
- `struct ProxyDecision`
- `fn resolve_proxy`
- `fn start_detached_decision`
- `fn resolve_memory_conflict`
- `fn kill_proxy` — **move** its SIGTERM-by-pid logic into Task 7's uninstall path; delete from `wrapper.rs`.
- `struct ProxyHealth`, `fn probe_proxy_health`, `fn parse_proxy_health`
- the four `#[cfg(test)]` fns: `parse_proxy_health_reads_memory_and_pid`, `parse_proxy_health_defaults_memory_false_when_absent`, `parse_proxy_health_handles_missing_config`, `parse_proxy_health_rejects_garbage`
- the old `fn start_proxy_detached_ready` (superseded by `spawn_proxy_ready`)

Keep `probe_proxy()` (now delegating to `probe_port(ANCHOR_PORT)`) — it's still used by `check_proxy_starts`. Keep `PROXY_PORT`/`DEFAULT_PROXY`/`PROXY_HEALTH_URL` **only if** still referenced after edits; otherwise delete the unused consts (clippy will flag them). Update `set_proxy_env`'s doc/behavior note: it still seeds a default `ANTHROPIC_BASE_URL`, but the per-session override in `wrap_claude` now wins.

`check_proxy_starts` / `smoke_start_proxy` continue to call `build_proxy_args(&port, ...)` and `headroom_env_plan()` unchanged (doctor still smoke-tests on a free port).

- [ ] **Step 7: Update CLAUDE.md**

In `CLAUDE.md`, update the **"Global Headroom memory root"** key-design-decision bullet to describe per-config keyed reuse. Replace the parenthetical about "a single shared process" with wording that states: whetstone now runs *one proxy per distinct resolved config*, keyed by a fingerprint in `~/.whetstone/proxies.json`; identical configs still share a proxy; port 8787 is a best-effort launch-order anchor for bare-`claude`/doctor; the global memory root is unchanged. Add a one-line note under the CLI/Architecture prose that the memory-conflict prompt was removed (memory on/off is now part of the fingerprint, so conflicting sessions get separate proxies).

- [ ] **Step 8: Build, lint, and run the full test suite**

Run: `cargo build && cargo test && cargo clippy`
Expected: PASS; no clippy warnings; the four `parse_proxy_health` tests are gone; all registry + `build_proxy_spec` + `build_claude_args` tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/wrapper.rs CLAUDE.md
git commit -m "feat(proxy): per-config proxy selection; drop memory-conflict prompt"
```

---

### Task 7: `global uninstall` teardown

**Files:**
- Modify: `src/uninstall.rs`
- Modify: `src/proxy_registry.rs` (add `kill_all` helper + test)

**Interfaces:**
- Consumes: `Registry`, `registry_path`.
- Produces: `pub fn kill_all(reg: &Registry, kill: impl Fn(u32))` in `proxy_registry.rs`; `run_global` calls a new `remove_proxies()` in `uninstall.rs`.

- [ ] **Step 1: Write the failing test (pure kill_all)**

Add to `proxy_registry.rs` `mod tests`:

```rust
    #[test]
    fn kill_all_signals_every_recorded_pid() {
        let reg = Registry {
            entries: vec![
                ProxyEntry { fingerprint: "a".into(), port: 8787, pid: 11 },
                ProxyEntry { fingerprint: "b".into(), port: 8801, pid: 22 },
            ],
        };
        let killed = std::cell::RefCell::new(Vec::new());
        kill_all(&reg, |pid| killed.borrow_mut().push(pid));
        assert_eq!(*killed.borrow(), vec![11, 22]);
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --lib kill_all_signals_every_recorded_pid`
Expected: FAIL — `kill_all` undefined.

- [ ] **Step 3: Implement `kill_all`**

Add to `proxy_registry.rs`:

```rust
/// Signal every recorded proxy pid via the injected killer.
pub fn kill_all(reg: &Registry, kill: impl Fn(u32)) {
    for entry in &reg.entries {
        kill(entry.pid);
    }
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test --lib kill_all_signals_every_recorded_pid`
Expected: PASS.

- [ ] **Step 5: Wire into `run_global`**

In `src/uninstall.rs`, add a helper and call it from `run_global` (before the closing `ui::ok`):

```rust
fn remove_proxies() {
    use crate::proxy_registry as reg;
    let Some(path) = reg::registry_path() else { return };
    let registry = reg::Registry::load(&path);
    if registry.entries.is_empty() {
        return;
    }
    ui::info("stopping whetstone-spawned Headroom proxies...");
    reg::kill_all(&registry, |pid| {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    });
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(path.with_extension("lock"));
    ui::ok("stopped registered proxies and removed the registry");
}
```

Call `remove_proxies();` inside `run_global()` (e.g. right after `remove_bins();`). Confirm `fs` and `crate::proxy_registry` are in scope (add `use` if needed).

- [ ] **Step 6: Build, lint, test**

Run: `cargo build && cargo test && cargo clippy`
Expected: PASS, no warnings.

- [ ] **Step 7: Commit**

```bash
git add src/proxy_registry.rs src/uninstall.rs
git commit -m "feat(proxy): kill registered proxies on global uninstall"
```

---

## Self-Review

**1. Spec coverage:**
- Fingerprint over exact spawn config, memory in fingerprint, MEMORY_DB_PATH excluded → Task 1 (+ `build_proxy_spec` Task 6).
- Registry `~/.whetstone/proxies.json` → Task 2.
- Lockfile-guarded critical section → Task 3, applied in Task 6 `select_proxy_port`.
- Launch-order 8787 anchor → Task 4.
- Prune-dead + reuse-or-spawn resolve → Task 5.
- `resolve_proxy` rewrite, `ANTHROPIC_BASE_URL` override, `spawn_proxy_detached` port arg, delete memory-conflict prompt + `probe_proxy_health` parsing → Task 6.
- CLAUDE.md design-decision update → Task 6 Step 7.
- `global uninstall` kills registered proxies + removes registry → Task 7.
- Known-limitation (orphaned live proxies not reaped): documented in spec, no task — intentional YAGNI.
- doctor `:8787` fast-path unchanged: `probe_proxy()` retained delegating to `ANCHOR_PORT` (Task 6 Step 5); `HEADROOM_PORT` reserved key untouched (Global Constraints).

**2. Placeholder scan:** No "TBD"/"add error handling"/"similar to Task N". One in-plan interface *correction* is called out explicitly (Task 5 `spawn` signature is `Fn(u16) -> Option<u32>`, authoritative over the struct-comment draft). All code steps carry real code.

**3. Type consistency:**
- `ProxySpec` fields identical in Tasks 1, 5, 6.
- `spawn` closure is `Fn(u16) -> Option<u32>` in Task 5 impl, its tests, and Task 6's `select_proxy_port` (`spawn_proxy_ready(p, spec.memory) -> Option<u32>`). ✓
- `ResolveDeps` fields (`probe`, `port_free`, `free_port`, `spawn`) match between Task 5 definition and Task 6 construction. ✓
- `ProxyOutcome::{Reused, Spawned, Failed}` used identically in Tasks 5 and 6. ✓
- `registry_path()`/`acquire_lock`/`ANCHOR_PORT`/`kill_all` signatures consistent across Tasks 2–7. ✓
- `probe_port(u16) -> bool` and `free_port_is_bindable(u16) -> bool` introduced in Task 6 and used only there. ✓
