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
Edit code → Test (tiered) → Quality gate → Coverage check → Wiki ingest → Done
```

### Step 1: Test (tiered — fast to slow)

| Tier | Command | When to use |
|------|---------|-------------|
| Compile check | `cargo check -p <crate>` | During development, after each edit |
| Test compile | `make test-compile` | Quick gate, ~3s warm |
| Unit tests | `make test-unit` | After completing a function/module |
| Integration tests | `make test-integration` | Before claiming done; includes `tests/` dir |
| E2E tests | `make test-e2e` | Only when changing external service interactions |

Always start at the fastest tier. Run the full suite with `make test` before claiming completion.

### Step 2: Quality Gate

**Fast gate** (local development, before committing):

```bash
make quality
```

This runs: `fmt-check` + `clippy` + `test-compile` + `no-doc-tests`

**Full gate** (pre-PR, CI equivalent):

```bash
make quality-full
```

This runs: `fmt-check` + `clippy-strict` + `test-unit` + `no-doc-tests` + `check-agent-boundaries.sh`

**Always run `make quality` before committing.** The full gate runs in CI; running it locally catches issues early.

### Step 3: Coverage Check

```bash
make coverage-threshold PKG=<crate>
```

**Requirement:** line coverage ≥ 80% for every crate. Exception: `main.rs`, `app.rs`, `health.rs` are exempt.

If coverage is below threshold:
1. Check which functions/lines are uncovered: `make coverage-html PKG=<crate>`
2. Add tests for uncovered paths
3. Re-run threshold check

### Step 4: Wiki Ingest

After completing any non-trivial backend change, **invoke `wiki-ingest`** to update `docs/wiki/`. This is not optional — the wiki is the project's persistent knowledge base.

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

Verify with: `./scripts/check-no-doc-tests.sh` (also part of `make quality`).

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
| Fast quality gate | `make quality` |
| Full quality gate | `make quality-full` |
| All tests | `make test` |
| Coverage summary | `make coverage PKG=<crate>` |
| Coverage gate (80%) | `make coverage-threshold PKG=<crate>` |
| Coverage HTML | `make coverage-html PKG=<crate>` |
| No doc tests check | `./scripts/check-no-doc-tests.sh` |
| Crate boundary check | `./scripts/check-agent-boundaries.sh` |
| Vulnerability scan | `make audit` |
| Rust workspace build | `cargo build -p <crate> --release` |

## Common Mistakes

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Doc tests in doc comments | `check-no-doc-tests.sh` fails | Convert to `#[cfg(test)]` or use ` ```text` |
| Committing without `make quality` | CI fails on fmt/clippy | Run `make quality` before every commit |
| Claiming done without coverage | Coverage below 80% | Run `make coverage-threshold PKG=<crate>` |
| Registering tools outside `AgentRuntimeBuilder::build()` | Duplicate or missed registrations | Use `AgentRuntimeBuilder::build()` as single source of truth |
| Wire types in `vol-agent-server` | Boundary check fails | Move to `vol-llm-agent-protocol::agent_server_protocol` |
| Importing `vol-agent-server` in `vol-llm-runtime` | Boundary check fails | Refactor to avoid dependency |
| Forgetting wiki-ingest | Wiki goes stale | Invoke `wiki-ingest` after every non-trivial change |
| Running full test suite for quick check | Slow feedback loop | Use tiered approach: compile check first, then unit, then integration |
| Adding protocol op without `vol-protocol` skill | Silent runtime failure | Invoke `vol-protocol` before adding any operation |
| Skipping `cargo fmt` | CI fmt-check fails | Run `cargo fmt --all` or `make quality` |

## Red Flags — STOP and Check

- "I'll run tests later"
- "One doc test won't hurt"
- "Coverage is probably fine"
- "I can register this tool directly in the data-plane handler"
- "Wiki ingest can wait"
- "The crate boundary check is probably not needed for this change"

**All of these mean: follow the workflow. Run quality gates now.**
