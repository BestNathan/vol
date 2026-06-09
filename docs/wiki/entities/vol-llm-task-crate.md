---
type: entity
category: service
tags: [task-store, persistence, seaorm, sqlite, postgres]
created: 2026-06-09
updated: 2026-06-09
source_count: 5
---

# vol-llm-task Crate

## Overview
`vol-llm-task` provides task data models, task store abstractions, and task persistence implementations for the agent task system.

## Key Facts
- Contains `TaskStore` abstractions and concrete stores for task persistence.
- `DatabaseTaskStore` is re-exported from the crate root for runtime construction by [[vol-llm-runtime-crate]].
- `stores::database::DatabaseTaskStore` uses SeaORM and supports SQLite and Postgres connections; MySQL is recognized but not enabled.
- Schema migrations use SeaORM Rust `MigratorTrait` and are compiled into the binary at runtime.
- Database CRUD and ready-task behavior are verified with tests for create/get/update/delete/list, dependency readiness, and persistence across reconnect.

## SQLite Database Store
Sources: [[seaorm-task-database-store-implementation]], [[seaorm-sqlite-url-normalization-fix]]

The SQLite database store creates parent directories for file-backed SQLite URLs, connects through SeaORM with `ConnectOptions`, and runs the embedded SeaORM migrator during connection setup.

The SeaORM database skeleton normalizes SQLite URLs by appending `mode=rwc` unless an exact query parameter key named `mode` already exists. The query-key check prevents options like `journal_mode=wal` from accidentally suppressing `mode=rwc`.

## SeaORM Entity and Migration

The SeaORM `tasks` entity stores scalar fields as columns, while `dependencies`, `blocks`, and `TaskResult` are serialized as JSON text. Epoch seconds are used for timestamps to avoid cross-DB timezone differences. `id: i64` maps to `TaskId(pub u64)` with overflow checks.

The SeaORM Rust migration creates `tasks` if it does not exist and `idx_tasks_status` on the status column. The migration uses SeaORM/SeaQuery abstractions rather than backend-specific SQL strings.

Task data is stored in a single `tasks` table. `get_ready_tasks` matches existing store semantics by returning pending tasks whose dependencies all resolve to completed task IDs.

[[seaorm-postgres-test-isolation-fix]] updates the Postgres database tests to use the same temp-dir file lock as the runtime Postgres test, preventing table-wide cleanup from racing across cargo test processes.

[[seaorm-postgres-test-url-env-fix]] removes the live Postgres DSN from committed task-store tests. Postgres remains mandatory: tests read `VOL_AGENT_POSTGRES_TEST_URL` and fail with `VOL_AGENT_POSTGRES_TEST_URL must be set for mandatory Postgres task-store tests` when it is absent.

## Related
- [[runtime-task-store-configuration]]
- [[seaorm-task-database-store-implementation]]
- [[seaorm-postgres-test-isolation-fix]]
- [[seaorm-postgres-test-url-env-fix]]
- [[seaorm-sqlite-url-normalization-fix]]
