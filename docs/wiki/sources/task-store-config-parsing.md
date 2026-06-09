---
type: source
source_type: code
date: 2026-06-09
ingested: 2026-06-09
tags: [task-store, runtime, config, validation]
---

# Runtime Task Store Config Parsing

**Authors/Creators:** Nathan + Claude
**Date:** 2026-06-09
**Link:** `/Users/admin/Documents/learn/vol-agent/crates/vol-llm-runtime/src/lib.rs`, `/Users/admin/Documents/learn/vol-agent/crates/vol-agent-server/src/config.rs`

## TL;DR
Task 1 of the task database store plan added SQL-independent runtime task store configuration types and server TOML parsing/validation for `[runtime.task_store]`. The server can now deserialize `type = "file"` or `type = "database"`, validate database URL schemes, and reject invalid combinations before runtime wiring or SQLx-backed storage exists.

## Key Takeaways
- [[vol-llm-runtime-crate]] now owns `TaskStoreType`, `TaskStoreConfig`, and `validate_database_url_scheme` as public config primitives.
- [[vol-agent-server-crate]] now depends on [[vol-llm-runtime-crate]] for shared config types and parses optional `runtime.task_store`.
- Database task stores require a URL; file task stores reject URL configuration.
- Recognized database schemes are `sqlite`, `postgres`, `postgresql`, and `mysql`; unsupported or missing schemes return explicit validation errors.
- `ServerConfig::load` validates parsed config before returning it.
- Server config tests cover valid database parsing and rejected missing URL, file URL, and unknown scheme cases.

## Detailed Summary
This change establishes the configuration surface for future database-backed task storage without adding SQLx or a `DatabaseTaskStore`. In `crates/vol-llm-runtime/src/lib.rs`, the runtime crate defines `TaskStoreType` with `File` and `Database` variants and `TaskStoreConfig` with a serde-renamed `type` field plus optional `url`. Validation is intentionally SQL-independent and checks only config shape and URL scheme.

In `crates/vol-agent-server/src/config.rs`, `RuntimeSection` now has `task_store: Option<vol_llm_runtime::TaskStoreConfig>`, defaulting to `None` to preserve the file-store default. `ServerConfig::load` now parses, validates, and returns the config, while `ServerConfig::validate` delegates task store validation to runtime-owned config types.

The TDD sequence first added `test_parse_database_task_store_config`, observed the expected compile failure from the missing dependency/field, then implemented the minimal config types and server parsing. Additional tests assert exact validation errors for database-without-url, file-with-url, and unsupported database scheme.

## Entities Mentioned
- [[vol-llm-runtime-crate]]: owns shared agent runtime resources and now task store config types.
- [[vol-agent-server-crate]]: parses server TOML config and validates `[runtime.task_store]`.

## Concepts Covered
- [[runtime-task-store-configuration]]: shared runtime-owned task store backend selection and validation.

## Notes
- This task deliberately does not add SQLx, runtime builder wiring, or a database task store implementation.
- `Cargo.lock` gained the related `vol-llm-runtime` dependency entry for `vol-agent-server`.
