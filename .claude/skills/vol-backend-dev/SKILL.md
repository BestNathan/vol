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
Edit code → Quality gate (MANDATORY) → Wiki ingest → Done
```

Development happens in iterations: write code, run the quality gate, fix what the gate
reports, re-run the gate. Only proceed to commit/push/PR after the gate passes with
zero failures.

---

## Quality Gate (MANDATORY — run before every commit)

The quality gate mirrors what CI runs. It is NOT optional. A commit that hasn't passed
the gate locally will fail in CI.

### Tier 1: Fast gate (run after every significant edit)

```bash
make quality
```

| Check | Command | Catches |
|-------|---------|---------|
| Formatting | `cargo fmt --all -- --check` | Style violations, inconsistent indentation |
| Clippy | `cargo clippy --workspace` | Redundant code, non-idiomatic patterns, potential bugs |
| Test compile | `cargo test --no-run --workspace` | Code that doesn't compile in test configuration |
| No doc tests | `./scripts/check-no-doc-tests.sh` | Doc comments with ` ```rust` instead of ` ```text` |

### Tier 2: Full gate (run before pushing / creating PR)

```bash
make quality-full
```

Includes Tier 1 checks plus:

| Check | Command | Catches |
|-------|---------|---------|
| Clippy strict | `cargo clippy --workspace -- -D warnings` | All warnings treated as errors |
| All tests | `cargo test --workspace --no-fail-fast` | Broken tests in ANY crate, not just the one you changed |
| Crate boundaries | `./scripts/check-agent-boundaries.sh` | Forbidden inter-crate dependencies |

### Tier 3: Coverage gate (run before claiming completeness)

```bash
make coverage-threshold PKG=<crate>
```

Line coverage must be ≥ 80%. Exempt files: `main.rs`, `app.rs`, `health.rs`.

If coverage is below threshold:
1. `make coverage-html PKG=<crate>` to see uncovered lines
2. Add tests for uncovered paths
3. Re-run `make coverage-threshold PKG=<crate>`

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
| `make coverage-threshold` | `FAIL: line coverage X% is below 80%` | Add tests for uncovered functions/paths |

### What changed? Searching for downstream impact

Before committing any change that modifies a public API, output format, or function
signature, grep the workspace for callers and consumers that may need updating:

```bash
grep -rn "<old name / old output / old signature>" crates/ --include="*.rs"
```

Run the full test suite (`cargo test --workspace`) after grep-assisted fixes to
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
| Committing without running quality gate | CI fails on fmt/clippy/tests | Run `make quality` before every commit |
| Changing public API without grepping callers | Downstream tests break in other crates | Grep workspace for old name/signature, update all callers |
| Claiming done without coverage | Coverage below 80% | Run `make coverage-threshold PKG=<crate>` |
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
