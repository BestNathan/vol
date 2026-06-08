---
type: entity
category: service
tags: [task-store, persistence, sqlite, sqlx]
created: 2026-06-09
updated: 2026-06-09
source_count: 1
---

# vol-llm-task Crate

## Overview
`vol-llm-task` provides task data models, task store abstractions, and task persistence implementations for the agent task system.

## Key Facts
- Contains `TaskStore` abstractions and concrete stores for task persistence.
- `stores::database::DatabaseTaskStore` currently supports SQLite connections and recognizes future PostgreSQL/MySQL URL schemes as not-yet-enabled backends.
- SQLite schema migrations live under `crates/vol-llm-task/migrations/sqlite` and are embedded into the binary at compile time.

## SQLite Database Store
Source: [[task-store-sqlite-embedded-migrations]]

The SQLite database store opens SQLx SQLite pools with `create_if_missing(true)`, creates parent directories for file-backed SQLite URLs, and runs an embedded static SQLx migrator during connection setup.

The migrator is compiled into the crate with `sqlx::migrate!("./migrations/sqlite")`, avoiding runtime dependence on a source-tree `migrations/sqlite` directory. This makes release binaries and containers self-contained for SQLite task-store initialization.

## Related
- [[runtime-task-store-configuration]]
- [[task-store-sqlite-embedded-migrations]]
