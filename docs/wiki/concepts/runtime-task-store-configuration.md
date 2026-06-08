---
type: concept
category: architecture
tags: [task-store, configuration, runtime, validation]
created: 2026-06-09
updated: 2026-06-09
source_count: 2
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

## Related
- [[vol-llm-runtime-crate]]
- [[vol-agent-server-crate]]
- [[task-store-config-parsing]]
