---
type: entity
category: service
tags: [runtime, agents, tools, task-store]
created: 2026-06-09
updated: 2026-06-09
source_count: 4
---

# vol-llm-runtime Crate

## Overview
`vol-llm-runtime` provides `AgentRuntime`, the authoritative owner of shared agent resources: LLM providers, tool registry, task store, MCP manager, sandbox registry, skills, agent definitions, and agent status tracking.

## Key Facts
- `AgentRuntimeBuilder::build()` is the primary assembly point for runtime resources.
- Tool registration belongs in the runtime builder so transport wrappers inherit the same registry.
- Runtime task store config primitives are defined here, not in the server crate, so downstream server/channel code can share one config contract.

## Task Store Configuration
Source: [[task-store-config-parsing]]

The crate defines SQL-independent configuration types:
- `TaskStoreType`: `File` or `Database`.
- `TaskStoreConfig`: serde-deserializable config with renamed `type` field and optional `url`.
- `validate_database_url_scheme`: accepts `sqlite`, `postgres`, `postgresql`, and `mysql`; rejects missing or unsupported schemes with explicit messages.

Validation rules:
- `type = "file"` must not include `url`.
- `type = "database"` must include `url` and use a recognized scheme.

## Runtime Database Task Store
Sources: [[runtime-database-task-store-construction]], [[task-database-store-implementation]]

`AgentRuntimeBuilder::build()` now maps `TaskStoreType::Database` to `DatabaseTaskStore::connect(url)` and exposes the result through `runtime.task_store` as `Arc<dyn TaskStore>`. The same `runtime.task_store` is passed to the unified `task` tool and to transport-layer task handlers, preserving a single global task store rather than adding per-agent routing.

The runtime builder test uses valid provider config, creates a task through the database-backed store, rebuilds the runtime against the same SQLite URL, and verifies the task persists.

For Postgres integration coverage, [[seaorm-postgres-test-isolation-fix]] adds a shared cross-process lock with `vol-llm-task` database tests plus marker-based cleanup before and after runtime rebuild assertions.

## Related
- [[vol-agent-server-crate]]
- [[vol-llm-task-crate]]
- [[runtime-task-store-configuration]]
- [[task-store-config-parsing]]
- [[runtime-database-task-store-construction]]
- [[task-database-store-implementation]]
- [[seaorm-postgres-test-isolation-fix]]
