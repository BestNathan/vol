---
type: concept
category: architecture
tags: [testing, just, git-hooks, ci, quality-gates]
created: 2026-08-18
updated: 2026-08-19
source_count: 3
---

# Test Tiers

## Definition

The workspace splits tests into three explicit tiers and maps each tier to the scenario that runs it — fast local hooks run the cheap tier, CI runs the expensive tiers, and each scenario composes its own list of atomic `just` recipes instead of calling an umbrella script.

## Key Points

| Tier | Command selector | Runs at |
|------|------------------|---------|
| unit | `--lib` (src inline `#[cfg(test)]`) | pre-push (changed crates only), CI |
| integration | `-E 'kind(test)'` (`tests/` dirs) | CI only |
| coverage | `cargo llvm-cov` per-crate ≥80% | CI only |
| e2e | `--ignored` (external services) | `just test-e2e` / `just test-e2e-crate <crate>`; manual `e2e.yml` workflow |

- **E2E marker convention (2026-08-19):** `#[ignore]` means ONLY "needs external service", with the reason standardized as `#[ignore = "e2e: <requirement>"]`. Every e2e test carries an in-test guard (env var non-empty check or TCP probe) that skips cleanly with a `SKIP (e2e): ...` message — including on CI, where unconfigured secrets arrive as empty strings. Broken/disabled tests must be fixed or deleted, never `#[ignore]`d.
- **Frontend e2e:** Playwright with a mock backend (self-contained) runs on every frontend PR in quality.yml (`npm run test:e2e`) and via `just fe-e2e`; also available manually in e2e.yml.
- **Frontend vitest tiers (2026-08-19):** vitest runs as two projects — `unit` (node, `tests/unit/`) and `integration` (jsdom + @testing-library/react, `tests/integration/`, jest-dom setup with ResizeObserver/matchMedia stubs). Component integration tests render real components with a real jotai store and a mocked `@/lib/panel-client` (no live WS). Commands: `just fe-test-unit` / `just fe-test-integration`; CI runs the two projects as separate steps.

- **No umbrella recipes.** `quality` / `quality-strict` / `quality-full` / `test-all` were removed; nothing in `justfile` composes others. A scenario is the composition point: `.githooks/pre-push` calls `just no-clippy-allow && just test-unit-crates <crates>`; `quality.yml` has separate `just test-unit` and `just test-integration` steps.
- **Hooks are thin shells over just.** Git runs hooks with cwd = repo root so `just` finds the justfile. Only the push-range / changed-file detection stays in bash — just recipes cannot read git's stdin refs.
- **`test-unit-crates *CRATES`** (shebang recipe) builds `-p` flags per crate and falls back from nextest to cargo test, mirroring the existing `2>/dev/null || cargo test` convention.
- **Fallback, not silent skip:** if Rust changed but no `crates/` paths match (workspace `Cargo.toml` only), pre-push runs the full workspace unit tier.
- **Tier separation is enforced by target kind**, not by directory convention guessing: `--lib` is strictly the inline unit tests, `-E 'kind(test)'` is strictly the integration test targets — fixing the old `test-integration` recipe that actually ran everything. (nextest's `--tests` flag means ALL targets, verified empirically — do not use it for tiering.)

## How It Works

```
pre-commit (fast):   just fmt-check | just clippy | just no-clippy-allow
                     just fe-fmt-check | just fe-lint | just fe-type
pre-push (unit):     just no-clippy-allow
                     just test-unit-crates <changed crates>   # fallback: just test-unit
                     just fe-test
CI (integration):    just test-unit   +   just test-integration
                     coverage job (llvm-cov ≥80% gate)
                     quality-frontend: vitest + Playwright e2e (mock backend)
e2e:                 just test-e2e | just test-e2e-crate <crate>  (manual)
                     .github/workflows/e2e.yml (workflow_dispatch, secrets-gated,
                     tests degrade to clean skips when prerequisites are absent)
```

Each check is a single recipe (`fmt-check`, `clippy`, `no-clippy-allow`, `fe-fmt-check`, …) so any scenario can pick the subset it needs.

## Related Concepts

- [[coverage-gate-work]] — the per-crate ≥80% llvm-cov gate that now runs in CI only.
- [[cli-style-tool-pattern]] — same "atomic primitives composed at the call site" philosophy applied to tools.

## Source

- [[test-tiering-hooks]]
- [[test-tiering-e2e-completion]]
- [[frontend-test-tiering]]
