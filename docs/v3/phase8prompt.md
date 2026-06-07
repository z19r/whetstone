# Phase 8 Prompt — Testing, Release, Rollout

## Context
whetstone v3 (see Phase 0). Final phase: prove it, ship a release candidate, dogfood, then cut `3.0.0`. **Depends on all prior phases.**

## House rules
- Rust 2021; must pass `just release-check` and the full CI matrix in `.github/workflows/`.
- Real API calls in any e2e tests should be isolated and pinned; no flakiness in the gate.

## Objective
Comprehensive tests, a clean release-candidate path, and a documented breaking release.

## Tasks
- [ ] **8.1 Unit tests** — `integrations.rs` (init invocation + arg shaping), `doctor.rs` (hook-ordering normalization), `migrate.rs` (detection, importance mapping, idempotency, rollback), and the update-refresh version-diff logic.
- [ ] **8.2 Integration test** — the Phase 3 fixture project: full `migrate` + `--rollback` round-trip. Add a native-Windows no-op test (RTK hook absent there — migration must skip hook steps gracefully).
- [ ] **8.3 E2E smoke** — run on the same OS/arch matrix as `release.yml` (linux x86_64/aarch64, macOS x86_64/aarch64).
- [ ] **8.4 Release** — `just release set 3.0.0`, but ship `3.0.0-rc.1` first. The `verify-release` job currently asserts not-prerelease; add a prerelease path so the rc flows through CI, or run the rc from the `v3` branch.
- [ ] **8.5 CHANGELOG** — add an explicit **BREAKING** section: AutoMem removed, `whetstone db` removed, hooks now tool-managed, migration required → link the Migration Guide (Phase 7.4).

## Files likely touched
`tests/*` (new integration tests + fixtures), `src/*` (unit tests inline), `.github/workflows/release.yml` (prerelease path), `justfile`, `CHANGELOG.md`, `VERSION`, `Cargo.toml`.

## Deliverable / Done when
`3.0.0-rc.1` is dogfooded on at least one real v2 project (yours) with a successful migrate + rollback round-trip; the CI matrix is green; the CHANGELOG documents the breaking changes and links the migration guide; then `3.0.0` is released.

## Milestone check (whole project)
- [ ] M1 — fresh v3 install works
- [ ] M2 — all confirmed bugs fixed + version single-source-of-truth
- [ ] M3 — `migrate` dry-run / full / rollback green on the fixture
- [ ] M4 — update-refresh + installer + cleanup done
- [ ] M5 — docs/site honest, rc shipped, 3.0.0 released
