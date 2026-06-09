---
type: entity
category: service
tags: [task-store, persistence, sqlite, sqlx, seaorm]
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
- `stores::database::DatabaseTaskStore` currently supports SQLite connections and recognizes future PostgreSQL/MySQL URL schemes as not-yet-enabled backends.
- SQLite schema migrations live under `crates/vol-llm-task/migrations/sqlite` and are embedded into the binary at compile time.
- Database CRUD and ready-task behavior are verified with tests for create/get/update/delete/list, dependency readiness, and persistence across reconnect.

## SQLite Database Store
Sources: [[task-store-sqlite-embedded-migrations]], [[task-database-store-implementation]], [[seaorm-sqlite-url-normalization-fix]]

The SQLite database store opens SQLx SQLite pools with `create_if_missing(true)`, creates parent directories for file-backed SQLite URLs, and runs an embedded static SQLx migrator during connection setup.

The SeaORM database skeleton normalizes SQLite URLs by appending `mode=rwc` unless an exact query parameter key named `mode` already exists. The query-key check prevents options like `journal_mode=wal` from accidentally suppressing `mode=rwc`.

The migrator is compiled into the crate with `sqlx::migrate!("./migrations/sqlite")`, avoiding runtime dependence on a source-tree `migrations/sqlite` directory. This makes release binaries and containers self-contained for SQLite task-store initialization.

Task data is stored in a single `tasks` table. `dependencies`, `blocks`, and `TaskResult` are serialized as JSON, while scalar fields such as status, kind, subject, summary, timestamps, and output path are stored as columns. `get_ready_tasks` matches existing store semantics by returning pending tasks whose dependencies all resolve to completed task IDs.

[[seaorm-postgres-test-isolation-fix]] updates the Postgres database tests to use the same temp-dir file lock as the runtime Postgres test, preventing table-wide cleanup from racing across cargo test processes.

[[seaorm-postgres-test-url-env-fix]] removes the live Postgres DSN from committed task-store tests. Postgres remains mandatory: tests read `VOL_AGENT_POSTGRES_TEST_URL` and fail with `VOL_AGENT_POSTGRES_TEST_URL must be set for mandatory Postgres task-store tests` when it is absent.

## Related
- [[runtime-task-store-configuration]]
- [[task-store-sqlite-embedded-migrations]]
- [[task-database-store-implementation]]
- [[seaorm-postgres-test-isolation-fix]]
- [[seaorm-postgres-test-url-env-fix]]
