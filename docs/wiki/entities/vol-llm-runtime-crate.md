---
type: entity
category: service
tags: [runtime, agents, tools, task-store, session-store, data-plane]
created: 2026-06-09
updated: 2026-06-10
source_count: 9
---

# vol-llm-runtime Crate

## Overview
`vol-llm-runtime` provides `AgentRuntime`, the authoritative owner of shared agent resources: LLM providers, tool registry, task store, MCP manager, sandbox registry, skills, agent definitions, and agent status tracking.

## Key Facts
- `AgentRuntimeBuilder::build()` is the primary assembly point for runtime resources.
- Tool registration belongs in the runtime builder so transport wrappers inherit the same registry.
- Runtime task store config primitives are defined here, not in the server crate, so downstream server/channel code can share one config contract.
- Runtime session store config primitives are also defined here; `AgentRuntimeBuilder::build()` constructs the shared `Arc<dyn SessionManager>` from `[runtime.session_store]` [[session-database-store-implementation]].

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
Sources: [[runtime-database-task-store-construction]], [[seaorm-task-database-store-implementation]]

`AgentRuntimeBuilder::build()` now maps `TaskStoreType::Database` to `DatabaseTaskStore::connect(url)` and exposes the result through `runtime.task_store` as `Arc<dyn TaskStore>`. The same `runtime.task_store` is passed to the unified `task` tool and to transport-layer task handlers, preserving a single global task store rather than adding per-agent routing.

The runtime builder test uses valid provider config, creates a task through the database-backed store, rebuilds the runtime against the same database URL, and verifies the task persists.

For Postgres integration coverage, [[seaorm-postgres-test-isolation-fix]] adds a shared cross-process lock with `vol-llm-task` database tests plus marker-based cleanup before and after runtime rebuild assertions. [[seaorm-postgres-test-url-env-fix]] removes the committed live DSN from this runtime test; it now reads `VOL_AGENT_POSTGRES_TEST_URL` and fails clearly if the mandatory Postgres URL is absent.

## Runtime Session Store
Source: [[session-database-store-implementation]]

`AgentRuntime` now owns `session_manager: Arc<dyn SessionManager>` in addition to `task_store`. `SessionStoreConfig` mirrors the task-store config shape with `File` and `Database` variants. File/default config builds `FileSessionManager`; database config calls `DatabaseSessionManager::connect(url)`.

Agent registration uses `session_manager.entry_store_for_agent(agent_id)`, so the runtime's active agent sessions write to the same backend that JSON-RPC session-domain operations read from.

## Data-plane Capability Source
Source: [[agent-server-control-data-plane-architecture]]

In the proposed control/data-plane architecture, `AgentRuntime` is the authoritative source for data-plane `CapabilitySnapshot` data. `DataPlaneReporter` reads `runtime.tool_registry.definitions()`, `runtime.mcp_manager.server_tools()`, `runtime.skill_loader.list_all()`, and `runtime.agent_defs` to build capability snapshots. This avoids duplicating registry metadata in hand-written config files or control-plane config.

## Related
- [[agent-server-control-data-plane]]
- [[vol-agent-server-crate]]
- [[vol-llm-task-crate]]
- [[runtime-task-store-configuration]]
- [[runtime-session-store-configuration]]
- [[session-database-store-implementation]]
- [[task-store-config-parsing]]
- [[runtime-database-task-store-construction]]
- [[seaorm-task-database-store-implementation]]
- [[seaorm-postgres-test-isolation-fix]]
- [[seaorm-postgres-test-url-env-fix]]
