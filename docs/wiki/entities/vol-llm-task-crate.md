---
type: entity
category: service
tags: [task-store, persistence, seaorm, sqlite, postgres, task-id]
created: 2026-06-09
updated: 2026-09-02
source_count: 7
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
- `TaskId(pub u64)` has hand-written `Serialize`/`Deserialize`/`Display`/`FromStr` [[taskid-unification-session-task-binding]]. Canonical serialized form is the decimal string `"1"`; `Display` emits bare digits (no `t` prefix); `FromStr` strips one `t`. Deserialization is **lenient** — accepts bare integers (legacy data), canonical strings, and single-`t`-prefixed strings (historical tool output) — so old rows in both stores keep loading without a migration [[lenient-serde-zero-migration]].
- `TaskId` is re-exported as `vol_llm_task::TaskId`; `ParseTaskIdError` is the `FromStr::Err` type. `vol-llm-agent` and `vol-llm-agent-protocol` both depend on `vol-llm-task`.
- The CLI parser uses `parse_task_id_arg` (in `cli/parser.rs`) which routes through `TaskId::from_str`, so `--id 1`, `--id t1`, and `--id "1"` all work.

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
- [[cli-style-tool-pattern]] — the `task` CLI is the first implementation of this pattern; the `fs` tool in [[vol-llm-fs-crate]] is modeled on it
