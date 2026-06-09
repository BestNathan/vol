---
type: source
source_type: code
date: 2026-06-10
ingested: 2026-06-10
tags: [session, database, seaorm, runtime, json-rpc]
---

# Session Database Store Implementation

**Authors/Creators:** Nathan + Claude Code
**Date:** 2026-06-10
**Link:** `crates/vol-session`, `crates/vol-llm-runtime`, `crates/vol-llm-agent-channel`, `crates/vol-agent-server`, `config.vol-agent.example.toml`

## TL;DR
The session persistence path now supports a configurable database backend. `vol-session` owns a SeaORM-backed `DatabaseSessionEntryStore` plus `SessionManager` abstractions; `vol-llm-runtime` constructs a shared file or database session manager from `[runtime.session_store]`; `vol-llm-agent-channel` uses that manager for registered agent sessions and JSON-RPC `session.*` methods; `vol-agent-server` parses and passes the configuration through.

## Key Takeaways
- `DatabaseSessionEntryStore` stores session metadata in `sessions` and entries in `session_entries` using SeaORM and compiled migrations.
- `SessionManager` gives runtime/channel code a backend-neutral API for scoped stores, session listing, existence checks, and session-to-agent resolution.
- `FileSessionManager` preserves existing JSONL behavior while validating agent IDs and preventing path traversal.
- `[runtime.session_store]` mirrors task-store configuration: `type = "file"` or `type = "database"` with SQLite/Postgres URL support.
- `AgentRuntime` is the owner of the shared session manager, matching the AgentRuntime single-source-of-truth rule for shared resources.
- `AgentServerCore::register_agent` now uses `runtime.session_manager.entry_store_for_agent`, so live registered agents write to the configured backend.
- JSON-RPC session domain methods list/resume/read entries through `SessionManager` and preserve client-visible response fields.
- JSON-RPC WebSocket error sending now preserves request IDs and structured `ErrorPayload` application codes such as `session_not_found`.

## Detailed Summary

### vol-session
`vol-session` gained the persistent database implementation and manager abstraction:

- `manager.rs` defines `SessionInfo`, `SessionManager`, and `FileSessionManager`.
- `database_store/mod.rs` defines `DatabaseSessionManager` and `DatabaseSessionEntryStore`.
- `database_store/entity.rs` defines SeaORM entities for `sessions` and `session_entries`.
- `database_store/mapping.rs` maps `SessionEntry` to/from database rows.
- `database_store/migration.rs` embeds the SeaORM migrator into the crate.

The database schema uses two tables:

| Table | Role |
| --- | --- |
| `sessions` | Session metadata: `id`, `agent_id`, `created_at`, `updated_at`, `entry_count`, `metadata`. |
| `session_entries` | Entry stream: `id`, `session_id`, `created_at`, `parent_id`, `entry_type`, `data`. |

`save()` uses transactional get-or-create session metadata, validates agent scope, inserts the entry, and atomically increments `entry_count`. Reads, counts, checkpoint lookup, and deletes validate the scoped `agent_id` before touching entries. `get_after` is inclusive (`created_at >= after`) to match existing file/in-memory store behavior.

### Runtime and server configuration
`vol-llm-runtime` now defines `SessionStoreType` and `SessionStoreConfig`, validates database URL schemes, and builds `Arc<dyn SessionManager>` in `AgentRuntimeBuilder::build()`. Default behavior remains file-backed sessions.

`vol-agent-server` parses optional `[runtime.session_store]`, validates it during config load, logs whether the configured or default session store is active, and passes it through `AgentServerCoreBuilder::with_session_store_config`.

### Channel and JSON-RPC integration
`vol-llm-agent-channel` no longer scans file directories inside `SessionHandler`. It uses `Arc<dyn SessionManager>` for:

- `session.list`
- `session.resume`
- `session.entries`

`AgentServerCoreBuilder` forwards session-store config into runtime construction, extracts `runtime.session_manager`, and registers `SessionHandler` with it. `AgentServerCore::register_agent` also uses the same manager for the agent's active session store, preventing database-configured cores from accidentally writing live sessions to files.

Error mapping now distinguishes missing sessions, invalid agent IDs, scope conflicts, ambiguous file-backed session IDs, and database/store failures. The JSON-RPC WebSocket send path routes error messages through the normal codec so request IDs and structured application error payloads are preserved.

### Tests and verification notes
Added tests cover:

- File manager listing, scoped resolution, ambiguity, and invalid agent-id handling.
- SQLite database save/list/read/count/delete/scope-conflict behavior.
- Concurrent database saves preserving `entry_count`.
- Inclusive `get_after` behavior.
- Deterministic latest checkpoint tie-breaking.
- Postgres coverage when `VOL_AGENT_POSTGRES_TEST_URL` is configured, with test-row cleanup scoped to a session-id prefix.
- Runtime SQLite session manager construction.
- Channel JSON-RPC session listing through SQLite manager.
- Registered agent sessions writing through the configured SQLite manager.
- JSON-RPC WebSocket error payload preservation.

Final local verification found targeted crate tests passing, while workspace-level commands still have unrelated blockers: the mandatory runtime Postgres task-store test needs `VOL_AGENT_POSTGRES_TEST_URL`, `cargo check --workspace` fails in `vol-llm-ui`, and `cargo fmt --all --check` reports broad pre-existing formatting drift.

## Entities Mentioned
- [[vol-session]]: owns `SessionManager`, file manager, and SeaORM database session store.
- [[vol-llm-runtime-crate]]: constructs shared `session_manager` from `[runtime.session_store]`.
- [[vol-llm-agent-channel-crate]]: routes registered agent sessions and JSON-RPC session methods through the runtime manager.
- [[vol-agent-server-crate]]: parses, validates, logs, and forwards session-store config.

## Concepts Covered
- [[runtime-session-store-configuration]]: backend-neutral file/database session-store configuration.
- [[session-as-ssot]]: sessions remain the single message-history source; only the persistence backend changes.
- [[jsonrpc-transport]]: WebSocket error responses preserve request IDs and structured error payloads.
- [[runtime-task-store-configuration]]: task-store pattern mirrored by the new session-store config.

## Notes
- Existing JSONL sessions are not migrated automatically into the database.
- Postgres tests are explicit but locally skip if `VOL_AGENT_POSTGRES_TEST_URL` is unset.
- MySQL URL schemes are recognized at config validation time but database session store construction reports unsupported backend, matching the task-store style.
