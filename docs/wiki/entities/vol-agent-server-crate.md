---
type: entity
category: service
tags: [server, config, json-rpc, task-store, session-store]
created: 2026-06-09
updated: 2026-06-10
source_count: 3
---

# vol-agent-server Crate

## Overview
`vol-agent-server` is the standalone server crate that loads TOML configuration and launches the agent JSON-RPC backend service.

## Key Facts
- Config is loaded from an explicit path, `~/.vol/agent-server.toml`, or defaults.
- Runtime path settings include `working_dir` and `store_dir` with tilde expansion.
- The crate depends on [[vol-llm-runtime-crate]] for shared task store and session store config types.

## Runtime Task Store Config Parsing
Sources: [[task-store-config-parsing]], [[task-database-store-implementation]]

`RuntimeSection` now includes optional `task_store: Option<vol_llm_runtime::TaskStoreConfig>`. When omitted, config defaults preserve the existing file-backed task store path.

`ServerConfig::load` validates parsed config before returning it. `ServerConfig::validate` delegates to `TaskStoreConfig::validate`, giving early errors for invalid `[runtime.task_store]` TOML.

Covered test cases:
- Parses database config with `url = "sqlite:///tmp/vol-agent/tasks.db"`.
- Rejects `type = "database"` without `url`.
- Rejects `type = "file"` with `url`.
- Rejects unknown database scheme such as `oracle://`.

`vol-agent-server` logs whether it is using the default file task store or a configured store type, then passes `config.runtime.task_store.clone()` into `AgentServerCore::builder(...).with_task_store_config(...)`.

## Runtime Session Store Config Parsing
Source: [[session-database-store-implementation]]

`RuntimeSection` now includes optional `session_store: Option<vol_llm_runtime::SessionStoreConfig>`. Server validation delegates to `SessionStoreConfig::validate`, so invalid `[runtime.session_store]` TOML is rejected before server startup.

Covered test cases include parsing a SQLite database session-store URL and rejecting `type = "database"` when `url` is missing. Startup logging mirrors task-store logging and the builder chain passes `config.runtime.session_store.clone()` into `AgentServerCore::builder(...).with_session_store_config(...)`.

## Related
- [[vol-llm-runtime-crate]]
- [[runtime-task-store-configuration]]
- [[runtime-session-store-configuration]]
- [[session-database-store-implementation]]
- [[task-store-config-parsing]]
- [[task-database-store-implementation]]
