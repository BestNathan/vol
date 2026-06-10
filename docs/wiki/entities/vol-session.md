---
type: entity
category: product
tags: [crate, session, persistence]
created: 2026-05-04
updated: 2026-06-10
source_count: 3
---

# vol-session Crate

**Category:** Rust crate — Session message store and entry persistence
**Related:** [[session-as-ssot]], [[session-contributor]], [[run-context]], [[vol-llm-agent-crate]]

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

## Timeline
- **2026-04**: Session used as message store alongside RunContext.messages (dual-write)
- **2026-04-25**: Session becomes SSOT — RunContext.messages removed [[session-ssot-redesign]]
- **2026-06-09**: `FileSessionManager` hardened against path traversal in `agent_id` values with validation, `StoreError::InvalidInput`, and encoded quarantine paths for infallible store creation [[file-session-agent-id-validation]]
- **2026-06-10**: SeaORM-backed `DatabaseSessionEntryStore` and `DatabaseSessionManager` added with SQLite/Postgres support, compiled migrations, scoped access validation, and `SessionManager` integration [[session-database-store-implementation]]

## Related Concepts
- [[session-as-ssot]]: Session is the single source of truth
- [[session-contributor]]: Reads messages from Session as context
- [[session-compression]]: Compresses messages in Session
- [[run-context]]: Holds Session reference
- [[vol-llm-agent-crate]]: SessionRecorderPlugin lives here, uses vol-session types
- [[file-session-agent-id-validation]]: documents the agent-id path traversal hardening
- [[runtime-session-store-configuration]]: describes file/database runtime session backend selection
- [[session-database-store-implementation]]: documents the SeaORM session database store implementation
