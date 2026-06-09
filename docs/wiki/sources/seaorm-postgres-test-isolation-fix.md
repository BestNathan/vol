---
type: source
source_type: code
date: 2026-06-09
ingested: 2026-06-09
tags: [seaorm, postgres, task-store, tests, isolation]
---

# SeaORM Postgres Test Isolation Fix

**Authors/Creators:** Claude Code implementer
**Date:** 2026-06-09
**Link:** `/Users/admin/Documents/learn/vol-agent/crates/vol-llm-runtime/src/lib.rs`, `/Users/admin/Documents/learn/vol-agent/crates/vol-llm-task/src/stores/database/mod.rs`, `/Users/admin/Documents/learn/vol-agent/config.vol-agent.example.toml`

## TL;DR
The SeaORM Task 6 review fix makes the runtime Postgres task-store test safe against concurrent `cargo test --workspace` runs by sharing an OS temp-dir file lock with `vol-llm-task` Postgres database tests, using a UUID subject marker, and deleting marker rows before and after the test. A follow-up [[seaorm-postgres-test-url-env-fix]] removes the live test DSN from committed source and requires `VOL_AGENT_POSTGRES_TEST_URL` at runtime.

## Key Takeaways
- Runtime and task-store Postgres tests coordinate through the same temp-dir lock file: `vol-agent-postgres-task-store-test.lock`.
- The runtime Postgres test uses a unique subject marker containing process ID and UUID, then cleans rows by public `TaskStore::list`/`delete` before and after the persistence assertion.
- Holding the initial runtime store handle as `cleanup_store` lets cleanup run even if reconnect or assertions fail after task creation.
- `config.vol-agent.example.toml` now documents `postgres://USER:PASSWORD@HOST:5432/DATABASE` instead of a concrete local credential URL.
- Follow-up hardening moved the live test URL out of committed source; tests now read `VOL_AGENT_POSTGRES_TEST_URL` and fail clearly if it is unset.

## Detailed Summary
`vol-llm-task` Postgres tests previously serialized only within one crate process using a private Tokio mutex, while the runtime Postgres test could run in a separate process and be disrupted by task-store tests that issue `DELETE FROM tasks`. The fix replaces the private mutex with an OS-backed file lock in the system temp directory and has both runtime and task-store Postgres tests acquire the same lock before touching the shared Postgres table.

The runtime test also stopped relying on happy-path-only deletion by ID. It generates a unique subject marker, performs a marker cleanup before creating the task, clones the initial `runtime.task_store` as `cleanup_store`, and runs marker cleanup after the reconnect/assertion block before returning the result. This avoids leaked rows when later runtime rebuild, get, or assertion steps fail.

The config example change removes environment-specific Postgres credentials from `config.vol-agent.example.toml` while preserving a useful placeholder shape for users.

## Entities Mentioned
- [[vol-llm-runtime-crate]]: owns `AgentRuntime` and the runtime Postgres task-store persistence test.
- [[vol-llm-task-crate]]: owns `DatabaseTaskStore` and the shared Postgres database integration tests.

## Concepts Covered
- [[runtime-task-store-configuration]]: runtime database task-store behavior and test isolation expectations.

## Notes
The shared lock is intentionally minimal and duplicated in test modules rather than introduced as production code. If more crates add shared Postgres task-store tests, they should reuse the same lock filename or move it into a small test helper.