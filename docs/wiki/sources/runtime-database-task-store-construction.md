---
type: source
source_type: code
date: 2026-06-09
ingested: 2026-06-09
tags: [task-store, runtime, database, testing]
---

# Runtime Database Task Store Construction

**Authors/Creators:** Nathan + Claude
**Date:** 2026-06-09
**Link:** `/Users/admin/Documents/learn/vol-agent/crates/vol-llm-runtime/src/lib.rs`

## TL;DR
Task 6 completed runtime database task-store wiring by making `AgentRuntimeBuilder::build()` construct `DatabaseTaskStore` for `[runtime.task_store] type = "database"`. A review follow-up hardened the runtime builder test so it fails on database store construction errors and verifies create/get persistence through `runtime.task_store` across runtime rebuilds.

## Key Takeaways
- [[vol-llm-runtime-crate]] now builds a real database-backed task store when runtime config selects `database`.
- The runtime builder test uses a valid fake provider config so provider loading does not mask task-store construction behavior.
- The test creates a manual task through `runtime.task_store`, rebuilds the runtime against the same SQLite URL, and asserts the task is still readable.
- The old permissive assertion accepting `failed to create database task store` was removed because it allowed Task 6 to pass while database construction was broken.

## Detailed Summary
`AgentRuntimeBuilder::build()` selects the task-store backend from `TaskStoreConfig`: omitted or `file` config uses the existing file task store under `store_dir/tasks`, while `database` config calls `DatabaseTaskStore::connect(url)` and wraps it as `Arc<dyn TaskStore>`.

The review fix tightened `tests::builder_accepts_database_task_store_config_until_provider_requirement` in `crates/vol-llm-runtime/src/lib.rs`. The test now writes a fake provider TOML into `.agents/providers`, builds the runtime with `sqlite://.../tasks.db`, creates a task via the public `runtime.task_store` field, drops the runtime, rebuilds with the same config, and verifies the persisted task ID, subject, and description.

Verification commands for the amended Task 6 commit:
- `cargo test -p vol-llm-runtime tests::builder_accepts_database_task_store_config_until_provider_requirement -- --exact`
- `cargo check -p vol-llm-runtime`

## Entities Mentioned
- [[vol-llm-runtime-crate]]: owns runtime task-store selection and test coverage for database construction.
- [[vol-llm-task-crate]]: provides `DatabaseTaskStore` and the `TaskStore` trait used by runtime.

## Concepts Covered
- [[runtime-task-store-configuration]]: database selection now maps to real runtime construction and persistence verification.

## Notes
- The check command still reports existing warnings in dependent crates; no new runtime check errors were introduced.
- Unrelated UI CSS changes were intentionally left untouched during this fix.
