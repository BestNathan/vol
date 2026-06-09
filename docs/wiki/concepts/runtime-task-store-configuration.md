---
type: concept
category: architecture
tags: [task-store, configuration, runtime, validation]
created: 2026-06-09
updated: 2026-06-09
source_count: 5
---

# Runtime Task Store Configuration

## Definition
Runtime task store configuration is the shared `[runtime.task_store]` TOML contract used to select the global task persistence backend for the agent runtime.

## How It Works
Source: [[task-store-config-parsing]]

The config is defined in [[vol-llm-runtime-crate]] and parsed by [[vol-agent-server-crate]]. This keeps the backend selection contract close to the runtime that owns the task store while allowing the server to validate user config early.

Supported shape:

```toml
[runtime.task_store]
type = "file"
```

or:

```toml
[runtime.task_store]
type = "database"
url = "sqlite:///tmp/vol-agent/tasks.db"
```

## Validation Rules
- Omitted `[runtime.task_store]` means default file store behavior remains active.
- `type = "file"` rejects `url` because file store location is derived from `runtime.store_dir`.
- `type = "database"` requires `url`.
- Database URL schemes accepted at config time: `sqlite`, `postgres`, `postgresql`, `mysql`.
- Unknown or missing schemes produce explicit errors.

## Design Notes
This is intentionally SQL-independent. Config parsing and validation can land before SQLx dependencies, database migrations, or runtime database store construction. Later tasks can wire the validated config through builders and instantiate concrete stores without changing the TOML contract.

SQLite database task-store initialization is covered by [[task-store-sqlite-embedded-migrations]]: the `vol-llm-task` SQLite migrator is embedded at compile time so runtime database selection does not require source-tree migration files to be deployed.

SQLite URL normalization must append create mode only when no exact `mode` query key is present. [[seaorm-sqlite-url-normalization-fix]] documents the SeaORM skeleton review fix that made `journal_mode=wal` coexist with an appended `mode=rwc`.

Runtime construction is covered by [[runtime-database-task-store-construction]] and [[task-database-store-implementation]]: `AgentRuntimeBuilder::build()` now turns database config into a real `DatabaseTaskStore`, and the builder test asserts task persistence across runtime rebuilds instead of accepting database construction failures.

The completed implementation keeps one global task store. `AgentServerCoreBuilder` only forwards config into runtime construction; it does not create per-agent stores or patch the tool registry. The unified `task` tool and JSON-RPC `TaskHandler` both share `runtime.task_store`.

## Related
- [[vol-llm-runtime-crate]]
- [[vol-llm-task-crate]]
- [[vol-agent-server-crate]]
- [[task-store-config-parsing]]
- [[task-store-sqlite-embedded-migrations]]
- [[runtime-database-task-store-construction]]
- [[task-database-store-implementation]]
