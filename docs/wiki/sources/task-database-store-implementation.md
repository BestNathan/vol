---
type: source
source_type: code
date: 2026-06-09
ingested: 2026-06-09
tags: [task-store, database, sqlx, runtime, server-config]
---

# Task Database Store Implementation

**Authors/Creators:** Nathan + Claude
**Date:** 2026-06-09
**Link:** `docs/superpowers/specs/2026-06-09-task-database-store-design.md`, `docs/superpowers/plans/2026-06-09-task-database-store.md`, and implementation files under `crates/`

## TL;DR
The task system now supports a single global database-backed task store selected by `[runtime.task_store] type = "database"`. The runtime still owns one shared `runtime.task_store`, and the task tool plus JSON-RPC task handler continue to use that same store. SQLite is implemented with SQLx and embedded migrations; PostgreSQL/MySQL URL schemes are recognized for clear not-yet-enabled errors.

## Key Takeaways
- [[vol-llm-task-crate]] now exposes `DatabaseTaskStore`, a SQLx SQLite implementation of the existing `TaskStore` trait.
- SQLite migrations are embedded with `sqlx::migrate!("./migrations/sqlite")`, so release binaries do not need source-tree migration files at runtime.
- [[vol-llm-runtime-crate]] owns the shared `TaskStoreConfig`/`TaskStoreType` contract and builds either the existing file store or the new database store.
- [[vol-agent-server-crate]] parses and validates `[runtime.task_store]` before constructing `AgentServerCore`.
- [[vol-llm-agent-channel-crate]] passes task-store config through `AgentServerCoreBuilder` into `AgentRuntimeBuilder`; it does not patch or clone the task registry.

## Detailed Summary
The implementation keeps the original architectural constraint that `AgentRuntime` is the single source of truth for shared agent resources. `AgentRuntimeBuilder::build()` maps omitted task-store config and `type = "file"` to the existing file-backed store. When config uses `type = "database"`, it calls `DatabaseTaskStore::connect(url)` and registers the same `Arc<dyn TaskStore>` with the CLI-style `task` tool.

`DatabaseTaskStore` stores task fields in a single SQLite `tasks` table. Scalar task fields are columns, while `dependencies`, `blocks`, and `TaskResult` are serialized as JSON. `get_ready_tasks` loads tasks, collects completed dependency IDs, and returns pending tasks whose dependencies are all completed, matching the file and in-memory store semantics.

Server config validation happens before runtime construction. `type = "database"` requires `url`; `type = "file"` rejects `url`; recognized URL schemes are `sqlite`, `postgres`, `postgresql`, and `mysql`. At runtime, only SQLite is implemented. PostgreSQL/MySQL produce explicit recognized-but-not-enabled store errors.

The implementation was verified with focused task-store tests, runtime construction tests, server config tests, and final focused checks:
- `cargo test -p vol-llm-task`
- `cargo test -p vol-agent-server`
- `cargo check -p vol-llm-runtime -p vol-llm-agent-channel -p vol-agent-server`
- `cargo test -p vol-llm-task stores::database::tests::tasks_persist_across_reconnect -- --exact --nocapture`

## Entities Mentioned
- [[vol-llm-task-crate]]: owns `TaskStore`, `DatabaseTaskStore`, SQLite schema, embedded migrations, and task persistence tests.
- [[vol-llm-runtime-crate]]: owns task-store config types and constructs the global store.
- [[vol-agent-server-crate]]: parses, validates, logs, and passes task-store config.
- [[vol-llm-agent-channel-crate]]: transports the server builder config into runtime construction and keeps task handlers on the shared `runtime.task_store`.

## Concepts Covered
- [[runtime-task-store-configuration]]: shared TOML contract and runtime behavior for file/database task persistence.

## Notes
- The design intentionally omits `.agents/task-providers`, per-agent task stores, `tool_config.task` overrides, UI store selection, and automatic file-to-database migration.
- Existing deployments remain compatible because omitted `[runtime.task_store]` keeps the file store default.
