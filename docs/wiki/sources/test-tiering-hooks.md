---
type: source
source_type: design
date: 2026-08-18
ingested: 2026-08-18
tags: [testing, git-hooks, just, ci, unit-test, integration-test, e2e, quality-gates]
---

# Three-Tier Test Split: Hooks Run Unit Only, CI Runs Integration

**Authors/Creators:** BestNathan + Claude
**Date:** 2026-08-18
**Link:** `justfile`, `.githooks/pre-*`, `.github/workflows/quality.yml`

## TL;DR

Restructured the workspace test pipeline into three explicit tiers — unit / integration / e2e — and mapped each tier to a scenario: pre-commit runs fmt/lint/type, pre-push runs only unit tests for changed crates, CI runs unit + integration + coverage. The slow per-crate llvm-cov check was removed from pre-push (CI already has a coverage job). All logic now lives in atomic `just` recipes; both git hooks are thin shells that call `just` directly. E2E stays manual for now, with a dedicated workflow planned as follow-up.

## Key Takeaways

- Tier mapping: unit (`--lib`, src inline `#[cfg(test)]`) → pre-push; integration (`-E 'kind(test)'`, `tests/` dirs) → CI; e2e (`--ignored`, external services) → manual / future dedicated workflow.
- No umbrella recipes: `quality`, `quality-strict`, `quality-full`, `test-all` were deleted. Each scenario (hook file / CI yml) composes its own list of atomic recipes.
- Hooks are thin shells: only push-range/changed-file detection stays in bash (just recipes cannot read git's stdin refs). Everything else is `just <recipe>`.
- `test-integration` was misnamed and fixed: it ran the whole suite; now it runs only the `-E 'kind(test)'` filter (tests/ dirs), truly layered above `test-unit`. Verified empirically that nextest's `--tests` flag means ALL targets (97 lib tests all reappear), so the kind filter is required; cargo test has no equivalent filter, so the fallback runs the full non-ignored suite.
- `test-unit-crates *CRATES` (new): shebang recipe building `-p` flags per crate; nextest with `cargo test` fallback — used by pre-push for changed crates.
- Fallback: `RUST_CHANGED` without `crates/` paths (e.g. workspace `Cargo.toml` only) → full workspace `just test-unit`, no silent skip.
- Coverage moved to CI-only in the hook tiering; pre-push latency drops from llvm-cov instrumented builds to plain unit test runs.
- Six superseded check scripts deleted (`check-rust-fmt.sh`, `check-rust-clippy.sh`, `check-fe-format.sh`, `check-fe-lint.sh`, `check-fe-type.sh`, `check-fe-test.sh`); frontend checks are now `fe-fmt-check` / `fe-lint` / `fe-type` / `fe-test` recipes. `check-rust-coverage.sh` kept for manual use.

## Detailed Summary

### Tier table

| Tier | What runs | Where |
|------|-----------|-------|
| unit | `cargo nextest run --lib` (src `#[cfg(test)]`) | pre-push (changed crates only via `test-unit-crates`); CI |
| integration | `cargo nextest run -E 'kind(test)'` (tests/ dirs) | CI (`quality.yml` quality-rust job) |
| coverage | `cargo llvm-cov` per-crate ≥80% gate | CI (`quality.yml` coverage job) |
| e2e | `cargo test -- --ignored`, needs external services | manual `just test-e2e`; dedicated workflow planned |

### Hook split

- **pre-commit** (fast): `just fmt-check`, `just clippy`, `just no-clippy-allow`; frontend `just fe-fmt-check`, `just fe-lint`, `just fe-type`. Change detection via `scripts/detect-changes.sh` (kept — it is stdin-driven).
- **pre-push**: `just no-clippy-allow`, then `just test-unit-crates <changed crates>` (fallback `just test-unit` on workspace-level-only changes), then `just fe-test` for frontend. Both hooks guard for `just` on PATH and give a clear install error.

### CI

- `quality.yml` quality-rust job: `just test` split into two steps `just test-unit` + `just test-integration` (compilation shared, failure attribution per tier). Coverage job unchanged.
- Stale `make quality`-style references in `.claude/skills/vol-backend-dev/SKILL.md` updated to the new recipe names.

### Verification

- `just --list` parses; `just test-unit-crates vol-llm-tool vol-llm-sandbox` exit 0 (nextest 0.9.143, 92 tests in vol-llm-tool direct-run comparison).
- Hook simulation via piped refs: crates-change path detected `vol-llm-provider` and passed; workspace-only path fell back to full `test-unit` (triggered correctly; cold full-workspace run exceeded 10 min on this machine — warm runs are much faster).
- Pre-commit hook exercised for real during a throwaway branch commit (fmt + clippy + allow passed).

## Entities Mentioned

- [[vol-repository]]: workspace-level justfile and githooks.
- [[vol-agent-server-crate]]: covered by the CI tiers as the reference crate.

## Concepts Covered

- [[test-tiers]]: the tiering pattern this source establishes.
- [[coverage-gate-work]]: per-crate coverage gate now runs in CI only, no longer pre-push.

## Notes

- E2E dedicated workflow deferred by explicit decision — follow-up item.
- Cold full-workspace `test-unit` is >10 min in this environment; the pre-push fallback (workspace `Cargo.toml` changes) rarely triggers.
- `docs/superpowers/specs/2026-08-08-git-hooks-quality-gates.md` describes the previous hook design; this source supersedes its pre-push coverage section.
- Pre-existing failure found during integration-tier verification (NOT caused by this refactor; reproduces identically on the old `cargo test` path): `tests/bash_tool_test.rs` `test_bash_timeout` / `test_bash_timeout_kills_process` get SIGTERM — the tool's timeout kill escalates to the test runner's own process group in this environment. Workspace integration tier: 398 tests, 396 passed, 2 failed (these two), 24 skipped. The old pre-push coverage check (llvm-cov) would have hit the same tests; the new pre-push (unit-only) avoids them.
