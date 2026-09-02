---
type: entity
category: product
tags: [crate, session, persistence, metadata, binding]
created: 2026-05-04
updated: 2026-09-02
source_count: 5
---

# vol-session Crate

**Category:** Rust crate — Session message store, entry persistence, and session-level metadata
**Related:** [[session-as-ssot]], [[session-contributor]], [[run-context]], [[vol-llm-agent-crate]], [[session-task-binding]]

## Overview

The session crate providing `Session`, `SessionMessage`, and `SessionEntryStore` types for persistent conversation message storage. Session is the single source of truth for agent messages.

## Key Facts
- `Session` wraps an `Arc<dyn SessionEntryStore>` for pluggable persistence [[session-ssot-redesign]]
- `InMemoryEntryStore` provides in-memory storage for testing [[session-ssot-redesign]]
- `SessionMessage` wraps `Message` with session_id, id, parent_id, and metadata [[session-ssot-redesign]]
- `SessionEntry` stores messages with metadata (including `RUN_ID_KEY`) [[session-ssot-redesign]]
- `SessionRecorderPlugin` (in `vol-llm-agent`) records agent events to session [[plugin-context-migration]]
- Session no longer contains plugin code — `SessionRecorderPlugin` was moved to `vol-llm-agent` [[plugin-context-migration]]
- `FileSessionManager` validates scoped `agent_id` values as a single normal path component before constructing filesystem stores [[file-session-agent-id-validation]]
- Invalid IDs in `entry_store_for_agent` are quarantined below `agents_root/.invalid-agent-id/<hex>/sessions` because the trait method cannot return `Result` [[file-session-agent-id-validation]]
- `SessionManager` abstracts backend-neutral session listing, scoped store creation, existence checks, and session-to-agent resolution [[session-database-store-implementation]]
- `DatabaseSessionEntryStore` persists entries through SeaORM with `sessions` and `session_entries` tables for SQLite/Postgres backends [[session-database-store-implementation]]
- `SessionEntryStore` carries a generic session-level metadata map accessed via `get_session_metadata` (returns empty for unknown sessions) and `merge_session_metadata` (shallow merge with upsert) [[taskid-unification-session-task-binding]]
- `SessionEntryStore::append_session_metadata_values(session_id, key, values: &[String])` performs an atomic union into an array at `key`, inside each backend's own lock or transaction — the only safe way to accumulate array values [[session-task-binding]]
- The three backends implement atomic append differently: in-memory uses one `RwLock`, database uses one transaction with `OnConflict::do_nothing` + ownership guard, file uses a per-instance `Mutex` plus atomic rename (the file backend's cross-instance limitation is documented) [[session-task-binding]]
- `Session::bind_task_ids` is the canonical way to record task association on a session; it delegates to the atomic `append_session_metadata_values` with `key = "task_ids"` [[session-task-binding]]
- `SessionInfo.metadata: Map<String, Value>` exposes metadata on the read surface; `session_model_to_info` stops dropping the `sessions.metadata` column; the file backend `list_sessions` populates metadata via N+1 (permitted for dev/local) [[taskid-unification-session-task-binding]]

## Timeline
- **2026-04**: Session used as message store alongside RunContext.messages (dual-write)
- **2026-04-25**: Session becomes SSOT — RunContext.messages removed [[session-ssot-redesign]]
- **2026-06-09**: `FileSessionManager` hardened against path traversal in `agent_id` values with validation, `StoreError::InvalidInput`, and encoded quarantine paths for infallible store creation [[file-session-agent-id-validation]]
- **2026-06-10**: SeaORM-backed `DatabaseSessionEntryStore` and `DatabaseSessionManager` added with SQLite/Postgres support, compiled migrations, scoped access validation, and `SessionManager` integration [[session-database-store-implementation]]
- **2026-08-17**: Compression preserves images — summary messages carry `[image]` markers and position sampling exempts image-bearing messages; images persist as wire `ContentPart` shape and are re-sent from resumed sessions [[multimodal-image-input]]
- **2026-09-02**: Generic session-level metadata added — `get_session_metadata`/`merge_session_metadata`/`append_session_metadata_values` on all three backends (in-memory, database reusing the dead `sessions.metadata` column with zero migration, file sidecar). `Session::bind_task_ids` and `SessionInfo.metadata` added. Atomicity lives at the store, not the caller. File backend's cross-instance limitation documented [[session-task-binding]], [[taskid-unification-session-task-binding]]

## Related Concepts
- [[session-as-ssot]]: Session is the single source of truth
- [[session-contributor]]: Reads messages from Session as context
- [[session-compression]]: Compresses messages in Session
- [[run-context]]: Holds Session reference
- [[session-task-binding]]: Session ↔ task id association via the metadata layer
- [[vol-llm-agent-crate]]: SessionRecorderPlugin lives here, uses vol-session types
- [[file-session-agent-id-validation]]: documents the agent-id path traversal hardening
- [[runtime-session-store-configuration]]: describes file/database runtime session backend selection
- [[session-database-store-implementation]]: documents the SeaORM session database store implementation
