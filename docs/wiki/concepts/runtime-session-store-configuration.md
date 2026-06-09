---
type: concept
category: architecture
tags: [session-store, configuration, runtime, persistence]
created: 2026-06-10
updated: 2026-06-10
source_count: 1
---

# Runtime Session Store Configuration

## Definition
Runtime session store configuration is the shared `[runtime.session_store]` TOML contract used to select file-backed or database-backed session persistence for agent conversations.

## How It Works
Source: [[session-database-store-implementation]]

The configuration is defined in [[vol-llm-runtime-crate]] and parsed by [[vol-agent-server-crate]]. Runtime construction produces one shared `Arc<dyn SessionManager>`, which is then used by registered agents and by JSON-RPC session-domain handlers in [[vol-llm-agent-channel-crate]].

Supported file-backed shape:

```toml
[runtime.session_store]
type = "file"
```

Supported database-backed shape:

```toml
[runtime.session_store]
type = "database"
url = "sqlite://data/sessions.db"
```

Postgres uses the same shape with a Postgres URL:

```toml
[runtime.session_store]
type = "database"
url = "postgres://vol_agent:vol_agent@localhost:5432/vol_agent_sessions"
```

## Validation Rules
- Omitted `[runtime.session_store]` keeps the default file-backed JSONL behavior.
- `type = "file"` rejects `url` because the file location is derived from `runtime.store_dir` and agent IDs.
- `type = "database"` requires `url`.
- Database URL schemes accepted at config time: `sqlite`, `postgres`, `postgresql`, and `mysql`.
- SQLite and Postgres are implemented; MySQL is recognized but not enabled.
- Unknown or missing schemes produce explicit validation errors.

## Runtime Behavior
`AgentRuntimeBuilder::build()` chooses the concrete manager:

- file/default config → `FileSessionManager`
- database config → `DatabaseSessionManager::connect(url)`

`AgentRuntime::register_agent()` uses `session_manager.entry_store_for_agent(agent_id)`, so new live agent sessions write to the configured backend. `AgentServerCore` and `SessionHandler` inherit the runtime-owned manager instead of creating their own stores.

## Database Model
The SeaORM database store uses two tables:

| Table | Purpose |
| --- | --- |
| `sessions` | One row per session, including `agent_id`, creation/update timestamps, `entry_count`, and metadata. |
| `session_entries` | Ordered entry stream containing messages, checkpoints, and summaries. |

The database store creates session metadata on first entry write, validates agent scope for all scoped operations, and updates `entry_count` atomically.

## Related
- [[session-database-store-implementation]]
- [[vol-session]]
- [[vol-llm-runtime-crate]]
- [[vol-agent-server-crate]]
- [[vol-llm-agent-channel-crate]]
- [[session-as-ssot]]
- [[runtime-task-store-configuration]]
