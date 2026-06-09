---
type: source
source_type: code
date: 2026-06-09
ingested: 2026-06-09
tags: [task-store, database, seaorm, runtime, server-config, sqlite, postgres]
---

# SeaORM Task Database Store Implementation

**Authors/Creators:** Nathan + Claude
**Date:** 2026-06-09
**Link:** `docs/superpowers/specs/2026-06-09-seaorm-task-database-store-design.md`, `docs/superpowers/plans/2026-06-09-seaorm-task-database-store.md`, and implementation files under `crates/`

## TL;DR
The task system's database-backed task store was replaced from SQLx raw queries to **SeaORM + SeaORM Migration**. SQLite and Postgres are both implemented; mandatory Postgres tests read the DSN from `VOL_AGENT_POSTGRES_TEST_URL`. The external `[runtime.task_store]` config, `DatabaseTaskStore::connect(url)`, `TaskStore` trait, and single-global `runtime.task_store` semantics all remain unchanged.

## Key Takeaways
- [[vol-llm-task-crate]] now uses SeaORM entity (`tasks`), SeaORM Rust migration, and mapping helpers instead of raw SQLx queries.
- The old SQLx migration file (`migrations/sqlite/0001_create_tasks.sql`) is deleted; SeaORM `MigratorTrait` creates the table and `idx_tasks_status` index at runtime.
- SQLite and Postgres are first-class supported backends; `mysql://` returns a clear not-enabled-yet error.
- Mandatory Postgres tests use `VOL_AGENT_POSTGRES_TEST_URL`; tests fail clearly when unset, not skip.
- Cross-process file lock serializes Postgres tests across `cargo test --workspace` and separate test processes.
- `[[vol-llm-runtime-crate]]` builds either file or database store from `TaskStoreConfig` and exposes one global `runtime.task_store`.
- `[[vol-agent-server-crate]]` parses, validates, logs, and passes `[runtime.task_store]` through `AgentServerCoreBuilder`.
- `[[vol-llm-agent-channel-crate]]` transports the config into runtime construction; JSON-RPC `TaskHandler` shares the same `runtime.task_store`.

## Detailed Summary
The implementation keeps the original architectural constraint that `AgentRuntime` is the single source of truth for shared agent resources. `AgentRuntimeBuilder::build()` maps omitted task-store config and `type = "file"` to the existing file-backed store. When config uses `type = "database"`, it calls `DatabaseTaskStore::connect(url)` and registers the same `Arc<dyn TaskStore>` with the CLI-style `task` tool.

`DatabaseTaskStore` holds a `sea_orm::DatabaseConnection` and inferred `DatabaseBackend`. `connect(url)` infers backend from URL scheme:
- `sqlite://...` creates parent directory if needed, connects via SeaORM `ConnectOptions`, and runs SeaORM migration.
- `postgres://...` / `postgresql://...` connects to existing database and runs SeaORM migration.
- `mysql://...` returns recognized-but-not-enabled error.

The SeaORM entity `tasks` matches the existing schema: scalar fields as columns, `dependencies`, `blocks`, and `TaskResult` stored as JSON text. Epoch seconds are used for timestamps to avoid cross-DB timezone differences. `id: i64` maps to `TaskId(pub u64)` with overflow checks.

CRUD uses SeaORM `ActiveModel` (insert with `id = NotSet` for auto-increment), `Entity::find_by_id`, `Entity::delete_by_id`, and `Entity::find().order_by_asc()`. `get_ready_tasks` loads tasks and filters in Rust, matching existing store semantics.

The implementation was verified with focused task-store tests (SQLite and Postgres), runtime construction tests (SQLite and Postgres), server config tests, and final focused checks:
- `cargo test -p vol-llm-task`
- `cargo test -p vol-agent-server`
- `VOL_AGENT_POSTGRES_TEST_URL=<url> cargo test -p vol-llm-runtime`
- `cargo check -p vol-llm-task -p vol-llm-runtime -p vol-llm-agent-channel -p vol-agent-server`

## Entities Mentioned
- [[vol-llm-task-crate]]: owns `TaskStore`, `DatabaseTaskStore`, SeaORM entity/migration/mapping, and database tests.
- [[vol-llm-runtime-crate]]: owns task-store config types and constructs the global store.
- [[vol-agent-server-crate]]: parses, validates, logs, and passes task-store config.
- [[vol-llm-agent-channel-crate]]: transports the server builder config into runtime construction; `TaskHandler` uses shared `runtime.task_store`.

## Concepts Covered
- [[runtime-task-store-configuration]]: shared TOML contract and runtime behavior for file/database task persistence, including SeaORM SQLite URL normalization and Postgres test isolation.

## Notes
- The design intentionally omits `.agents/task-providers`, per-agent task stores, `tool_config.task` overrides, UI store selection, and automatic file-to-database migration.
- Existing deployments remain compatible because omitted `[runtime.task_store]` keeps the file store default.
- Existing SQLite database files from the SQLx version remain compatible because SeaORM migration uses `if_not_exists`.
- No direct SQLx usage remains in `vol-llm-task` source (SeaORM pulls it transitively as its driver).
