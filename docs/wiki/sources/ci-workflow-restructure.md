---
type: source
source_type: design
date: 2026-08-19
ingested: 2026-08-19
tags: [ci, workflows, just, coverage, e2e, testing]
---

# CI Workflow Restructure: Gates vs Reports, just-Only Workflow Calls

**Authors/Creators:** BestNathan + Claude
**Date:** 2026-08-19
**Link:** .github/workflows/quality.yml, .github/workflows/e2e.yml

## TL;DR

Three workflow rules landed: (1) quality.yml never runs e2e — all e2e lives in e2e.yml; (2) unit + integration passing is the CI gate, coverage jobs are report-only (upload artifacts, never fail on percentage); (3) every workflow step that runs project logic is a `just` recipe — any script logic lives in `scripts/`.

## Key Takeaways

- **Gates and reports are separate jobs.** `quality-rust` (fmt, clippy, no-doc-tests, no-clippy-allow, unit, integration, boundaries) and `quality-frontend` (tsc, vitest unit, vitest integration) block the PR. `coverage` and `coverage-frontend` only produce reports.
- **E2E has exactly one home: e2e.yml** (manual `workflow_dispatch`). The Playwright steps were removed from quality.yml; it no longer runs on every PR.
- **Workflows call just, not scripts.** New recipes: `test-e2e-ci CRATE=""` (dispatches to `test-e2e`/`test-e2e-crate` with `--nocapture`), `cover-ci *PKG` (→ `scripts/ci-coverage-report.sh`), `fe-install`, `fe-pw-install`, `boundaries` (alias for `check-agent-boundaries.sh`).
- **The coverage gate moved to local dev.** `just cover-gate <crate> 80` remains the CLAUDE.md convention before claiming work done; CI just reports.
- **Env-setup one-liners stay inline** (e.g. `rm -f .cargo/config.toml` because CI cannot reach rsproxy.cn) — everything else is a recipe or a `scripts/` script.

## Detailed Summary

### quality.yml (rewritten)

- `quality-rust` unchanged as a gate: `just fmt-check`, `just clippy-strict`, `just no-doc-tests`, `just no-clippy-allow`, `just test-unit`, `just test-integration`, and now `just boundaries` instead of calling the script directly.
- `quality-frontend`: `just fe-install` (npm ci), `just fe-type`, `just fe-test-unit`, `just fe-test-integration`. Playwright steps deleted.
- `coverage` (rust): `just cover-ci` → `scripts/ci-coverage-report.sh` (llvm-cov `--summary-only` over the 11 core LLM crates), output teed to `target/coverage-summary.txt` and uploaded as an artifact (7-day retention). No threshold check. Script supports `COV_PACKAGES` and `COV_OUTPUT` env overrides for quick local runs.
- `coverage-frontend` (new): `just fe-install` + `just fe-test` (vitest run --coverage), uploads `frontend/coverage/` as an artifact. The vitest config's baseline thresholds (17/47/50/17, set 2026-08-08) are far below the measured coverage, so this is report-only in practice.

### e2e.yml

- `e2e-rust`: the inline if-else crate dispatch moved into the `test-e2e-ci` recipe; the step is now `just test-e2e-ci "${{ github.event.inputs.crate }}"` (empty string → whole workspace). `--nocapture` is baked into the recipe so `SKIP (e2e): ...` guard messages reach the CI log.
- `e2e-frontend`: `just fe-install`, `just fe-pw-install` (playwright install --with-deps chromium), `just fe-e2e`.
- Header comment updated: Playwright no longer claims to run in quality.yml.

### Verification

- `bash -n` on the new script clean; both workflows parse as YAML; `just --list` shows all five new recipes.
- Full CI semantics (actions running on ubuntu-24.04) not runnable locally; the Rust/frontend test suites themselves were unchanged by this restructure.

## Entities Mentioned

- [[vol-repository]]: justfile recipes added; CI workflows restructured.

## Concepts Covered

- [[test-tiers]]: gates vs report-only coverage split; e2e consolidated in e2e.yml; just-only workflow convention.
- [[coverage-gate-work]]: superseded in CI — the ≥80% llvm-cov gate is now a local-dev gate only.

## Notes

- Supersedes parts of [[test-tiering-e2e-completion]]: the "Playwright in quality.yml PR gate" and the "CI coverage gate (llvm-cov ≥80%)" no longer exist.
- If CI coverage reports are later wanted as PR comments or badges, the artifacts are already being produced — only presentation work remains.
- Running e2e in CI for real still requires repo secrets (ANTHROPIC_AUTH_TOKEN) and reachable services; unconfigured, the workflow degrades to clean skips by design.
