---
type: source
source_type: code
date: 2026-06-09
ingested: 2026-06-09
tags: [seaorm, postgres, task-store, tests, env-var, credentials]
---

# SeaORM Postgres Test URL Env Var Fix

**Authors/Creators:** Claude Code implementer
**Date:** 2026-06-09
**Link:** `/Users/admin/Documents/learn/vol-agent/crates/vol-llm-task/src/stores/database/mod.rs`, `/Users/admin/Documents/learn/vol-agent/crates/vol-llm-runtime/src/lib.rs`, `/Users/admin/Documents/learn/vol-agent/docs/superpowers/specs/2026-06-09-seaorm-task-database-store-design.md`, `/Users/admin/Documents/learn/vol-agent/docs/superpowers/plans/2026-06-09-seaorm-task-database-store.md`

## TL;DR
The SeaORM Postgres task-store tests no longer commit a live credential-bearing Postgres URL. Both `vol-llm-task` database tests and the `vol-llm-runtime` Postgres builder test read the mandatory DSN from `VOL_AGENT_POSTGRES_TEST_URL` and fail clearly if it is unset.

## Key Takeaways
- Postgres task-store tests are still mandatory; they do not skip when configuration is absent.
- `VOL_AGENT_POSTGRES_TEST_URL` supplies the live DSN at test runtime.
- The unset-env failure message is `VOL_AGENT_POSTGRES_TEST_URL must be set for mandatory Postgres task-store tests`.
- Superpowers spec and plan docs use the placeholder DSN `postgres://USER:PASSWORD@HOST:5432/DATABASE`.
- `config.vol-agent.example.toml` keeps its placeholder URL and does not expose a live DSN.

## Detailed Summary
`crates/vol-llm-task/src/stores/database/mod.rs` now defines a test helper that calls `std::env::var("VOL_AGENT_POSTGRES_TEST_URL")` and `expect`s the mandatory-test message. All Postgres store tests connect through that helper while retaining the shared temp-dir file lock used to serialize table cleanup.

`crates/vol-llm-runtime/src/lib.rs` mirrors the same helper for the runtime Postgres builder test. The test still builds a runtime with `TaskStoreType::Database`, creates a marker task, rebuilds against the same database URL, verifies persistence, and cleans marker rows through the public `TaskStore` API.

The SeaORM design spec and implementation plan were updated to describe the env-var requirement and use `postgres://USER:PASSWORD@HOST:5432/DATABASE` wherever an example DSN is needed. This keeps mandatory Postgres integration coverage while preventing committed credentials or environment-specific hosts from appearing in source and docs.

## Entities Mentioned
- [[vol-llm-task-crate]]: owns the SeaORM `DatabaseTaskStore` Postgres integration tests that read `VOL_AGENT_POSTGRES_TEST_URL`.
- [[vol-llm-runtime-crate]]: owns the runtime Postgres database task-store builder test that reads the same env var.

## Concepts Covered
- [[runtime-task-store-configuration]]: documents mandatory Postgres database-store test configuration through an environment variable.

## Notes
The review fix intentionally leaves `config.vol-agent.example.toml` on a placeholder DSN and does not introduce test skipping. Local and CI runs must provide `VOL_AGENT_POSTGRES_TEST_URL` when running mandatory Postgres task-store tests.
