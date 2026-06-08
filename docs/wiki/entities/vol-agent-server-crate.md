---
type: entity
category: service
tags: [server, config, json-rpc, task-store]
created: 2026-06-09
updated: 2026-06-09
source_count: 1
---

# vol-agent-server Crate

## Overview
`vol-agent-server` is the standalone server crate that loads TOML configuration and launches the agent JSON-RPC backend service.

## Key Facts
- Config is loaded from an explicit path, `~/.vol/agent-server.toml`, or defaults.
- Runtime path settings include `working_dir` and `store_dir` with tilde expansion.
- The crate depends on [[vol-llm-runtime-crate]] for shared task store config types.

## Runtime Task Store Config Parsing
Source: [[task-store-config-parsing]]

`RuntimeSection` now includes optional `task_store: Option<vol_llm_runtime::TaskStoreConfig>`. When omitted, config defaults preserve the existing file-backed task store path.

`ServerConfig::load` validates parsed config before returning it. `ServerConfig::validate` delegates to `TaskStoreConfig::validate`, giving early errors for invalid `[runtime.task_store]` TOML.

Covered test cases:
- Parses database config with `url = "sqlite:///tmp/vol-agent/tasks.db"`.
- Rejects `type = "database"` without `url`.
- Rejects `type = "file"` with `url`.
- Rejects unknown database scheme such as `oracle://`.

## Related
- [[vol-llm-runtime-crate]]
- [[runtime-task-store-configuration]]
- [[task-store-config-parsing]]
