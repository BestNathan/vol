---
name: vol-backend-dev
description: Use when implementing Rust backend changes in the vol workspace — adding features, fixing bugs, registering tools, or modifying crate code. Triggered by cargo commands, Rust source edits, or any backend task that needs quality gates and wiki ingest before completion.
---

# Vol Backend Development

## Overview

The vol workspace has ~30 Rust crates with strict conventions around quality gates, testing, crate boundaries, and documentation. Every backend change must pass a tiered quality pipeline before it is considered complete.

## When to Use

Use for any Rust change in `crates/`. Symptoms that trigger this skill:

- You are editing `.rs` files in any crate
- You are running `cargo build`, `cargo test`, `cargo check`
- You are adding a new `pub fn`, handler, or tool
- You are adding a new protocol operation (also invoke `vol-protocol`)
- You are about to claim backend work is "done"

## Development Workflow

```
Edit code → Pre-commit hook (auto) → Quality gate → Wiki ingest → Done
```

Development happens in iterations: write code, commit (hooks catch issues early),
run the full quality gate before pushing, fix what the gate reports, re-run.
Only proceed to push/PR after the full gate passes with zero failures.

### Git hooks — first line of defense

The project has a `.githooks/pre-commit` hook that runs before every commit.
Verify it is configured:

```bash
git config core.hooksPath   # must output ".githooks"
```

If it is not set, configure it once:

```bash
git config core.hooksPath .githooks
```

The pre-commit hook runs on staged Rust files (via just recipes):
- `just fmt-check` — `cargo fmt --all -- --check`
- `just clippy` — `cargo clippy --workspace`
- `just no-clippy-allow` — no new `#[allow(clippy::...)]`

The pre-push hook runs unit tests for changed crates (`just test-unit-crates`)
plus frontend tests (`just fe-test`). Integration tests, coverage, and e2e run
in CI — not in hooks.

If the hook blocks your commit, read the output. Each check prints what failed
and how to fix it. Fix the issues, `git add` the fixes, and commit again.

---

## Quality Gate (MANDATORY — run before every commit)

The quality gate mirrors what CI runs. It is NOT optional. A commit that hasn't passed
the gate locally will fail in CI.

Test tiers and where each runs:

| Tier | Where | Command |
|------|-------|---------|
| Unit tests | pre-push hook (changed crates only) | `just test-unit-crates <crate...>` / `just test-unit` |
| Integration tests | CI (`quality.yml`) | `just test-integration` |
| Coverage | CI report-only (`quality.yml` coverage jobs, no gate); local gate before claiming done | `just cover-gate <crate> 80` |
| E2E (external services) | manual `e2e.yml` workflow (secrets-gated) | `just test-e2e` / `just test-e2e-crate <crate>` |
| Frontend unit | pre-push (`just fe-test`), CI (`quality.yml`) | `just fe-test-unit` (vitest `--project unit`, node) |
| Frontend integration | CI (`quality.yml`) | `just fe-test-integration` (vitest `--project integration`, jsdom + testing-library) |
| Frontend e2e | manual `e2e.yml` (never in quality.yml) | `just fe-e2e` (Playwright, mock backend) |

All e2e tests carry `#[ignore = "e2e: ..."]` markers and in-test guards that
skip cleanly when their prerequisites are missing — safe to run anywhere.

There is no umbrella "quality-all" recipe — each scenario (hook / CI) composes
atomic `just` recipes itself.

### Fast checks (run after every significant edit)

```bash
just fmt-check
just clippy
just test-compile
just no-doc-tests
```

| Check | Command | Catches |
|-------|---------|---------|
| Formatting | `cargo fmt --all -- --check` | Style violations, inconsistent indentation |
| Clippy | `cargo clippy --workspace` | Redundant code, non-idiomatic patterns, potential bugs |
| Test compile | `cargo test --no-run --workspace` | Code that doesn't compile in test configuration |
| No doc tests | `./scripts/check-no-doc-tests.sh` | Doc comments with ` ```rust` instead of ` ```text` |

### Full checks (before pushing / creating PR)

```bash
just test-unit           # unit tests, whole workspace (--lib)
just test-integration    # tests/ integration tests only (-E 'kind(test)')
just clippy-strict       # warnings denied
```

| Check | Command | Catches |
|-------|---------|---------|
| Clippy strict | `cargo clippy --workspace -- -D warnings` | All warnings treated as errors |
| Unit tests | `just test-unit` | Broken `#[cfg(test)]` tests in ANY crate |
| Integration tests | `just test-integration` | Broken `tests/` integration tests |
| Crate boundaries | `./scripts/check-agent-boundaries.sh` | Forbidden inter-crate dependencies |

### Coverage gate (run before claiming completeness)

```bash
just cover-gate <crate> 80
```

Line coverage must be ≥ 80%. Exempt files: `main.rs`, `app.rs`, `health.rs`.

If coverage is below threshold:
1. `just cover-html <crate>` to see uncovered lines
2. Add tests for uncovered paths
3. Re-run `just cover-gate <crate> 80`

### Gate failure protocol

When any gate check fails, do NOT proceed to commit. Read the output of the failing
check. Each tool (fmt, clippy, rustc, nextest) prints the exact file, line number,
and problem description. Fix the reported issues, then re-run the gate from the
beginning. Never skip a failing check — the same check runs in CI and will block
the PR.

| Tool | Failure looks like | How to fix |
|------|-------------------|------------|
| `cargo fmt` | Diff showing lines that differ from canonical format | Run `cargo fmt --all` to auto-fix |
| `cargo clippy` | `error: <description>` with file:line | Read the suggestion (often includes `help:` with the fix) |
| `cargo test` | `test ... FAILED` or `error: test failed` | Read the assertion failure; fix the code or update the test |
| `check-no-doc-tests.sh` | Lists files with doc tests | Convert ` ```rust` to ` ```text` or move code to `#[cfg(test)]` |
| `check-agent-boundaries.sh` | Lists forbidden dependency paths | Remove the forbidden import; restructure code if needed |
| `just cover-gate <crate> 80` | `FAIL: line coverage X% is below 80%` | Add tests for uncovered functions/paths |

### What changed? Searching for downstream impact

Before committing any change that modifies a public API, output format, or function
signature, grep the workspace for callers and consumers that may need updating:

```bash
grep -rn "<old name / old output / old signature>" crates/ --include="*.rs"
```

Run the full test suite (`just test`) after grep-assisted fixes to
catch any callers that grep missed (e.g., dynamic dispatch, trait objects).

---

## Wiki Ingest

After completing any non-trivial backend change, **invoke `wiki-ingest`** to update
`docs/wiki/`. This is not optional — the wiki is the project's persistent knowledge
base.

## Code Conventions

### Tests

**No doc tests.** Write `#[cfg(test)]` unit tests or `tests/` integration tests. Doc comment code examples must use ` ```text` (not ` ```rust`).

```rust
// ✅ correct: doc comment with text code block
/// Returns the sum.
/// ```text
/// add(1, 2) → 3
/// ```

// ✅ correct: unit test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
    }
}

// ❌ wrong: doc test (compiled by cargo test)
/// ```
/// assert_eq!(add(1, 2), 3);
/// ```
```

Verify with: `just no-doc-tests` (also runs in CI `quality.yml`).

**Every new `pub fn` or handler → at least one test.**

### Crate Boundaries

These boundaries are enforced by CI (`check-agent-boundaries.sh`):

| Rule | Check |
|------|-------|
| `vol-llm-agent-protocol` must NOT depend on `vol-agent-server` | `./scripts/check-agent-boundaries.sh` |
| `vol-llm-runtime` must NOT depend on `vol-agent-server` | same script |
| No `vol-agent-control-plane` crate | control-plane lives in `vol-agent-server::control_plane` |

**vol-llm-agent-protocol owns all wire types** — `Operation`, `Payload`, `control.*`, JSON-RPC codec. Never define wire types in `vol-agent-server`.

**vol-llm-runtime knows nothing about control-plane** — no `NodeRegistry` / `ControlRouter` imports there.

### Tool Registration

**`AgentRuntimeBuilder::build()` is the primary registration point.** `DataPlaneServerCoreBuilder` inherits from it — do not duplicate tool/skill registrations in both places.

When adding a new tool or skill:
1. Add registration in `AgentRuntimeBuilder::build()`
2. `DataPlaneServerCoreBuilder` inherits it automatically
3. Do NOT add a second registration in data-plane code

### Adding Protocol Operations

If you are adding a new JSON-RPC operation (e.g., `agent.new_operation`), **invoke `vol-protocol` skill**. It covers 6 files that must be updated — missing `operation_codec.rs` causes silent runtime failures.

## Quick Reference

| Task | Command |
|------|---------|
| Build check | `cargo check -p <crate>` |
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --workspace` |
| All non-e2e tests | `just test` |
| Unit tests | `just test-unit` |
| Unit tests (specific crates) | `just test-unit-crates <crate...>` |
| Integration tests | `just test-integration` |
| E2E tests (external services) | `just test-e2e` |
| Coverage summary | `just cover <crate>` |
| Coverage gate (80%) | `just cover-gate <crate> 80` |
| Coverage HTML | `just cover-html <crate>` |
| No doc tests check | `just no-doc-tests` |
| Crate boundary check | `./scripts/check-agent-boundaries.sh` |
| Vulnerability scan | `just audit` |
| Rust workspace build | `cargo build -p <crate> --release` |

## Common Mistakes

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Doc tests in doc comments | `check-no-doc-tests.sh` fails | Convert to `#[cfg(test)]` or use ` ```text` |
| Committing without running quality gate | CI fails on fmt/clippy/tests | Run `just fmt-check && just clippy && just test` before committing |
| Changing public API without grepping callers | Downstream tests break in other crates | Grep workspace for old name/signature, update all callers |
| Claiming done without coverage | Coverage below 80% | Run `just cover-gate <crate> 80` |
| Registering tools outside `AgentRuntimeBuilder::build()` | Duplicate or missed registrations | Use `AgentRuntimeBuilder::build()` as single source of truth |
| Wire types in `vol-agent-server` | Boundary check fails | Move to `vol-llm-agent-protocol::agent_server_protocol` |
| Importing `vol-agent-server` in `vol-llm-runtime` | Boundary check fails | Refactor to avoid dependency |
| Forgetting wiki-ingest | Wiki goes stale | Invoke `wiki-ingest` after every non-trivial change |
| Adding protocol op without `vol-protocol` skill | Silent runtime failure | Invoke `vol-protocol` before adding any operation |
| Adding dep to sub-crate Cargo.toml not in workspace | Build fails | Check workspace `[dependencies]` first; add there if missing |

## Red Flags — STOP and Check

- "I'll run tests later"
- "One doc test won't hurt"
- "Coverage is probably fine"
- "I can register this tool directly in the data-plane handler"
- "Wiki ingest can wait"
- "The crate boundary check is probably not needed for this change"

**All of these mean: run the quality gate. Do not proceed until it passes.**
