---
type: entity
category: product
tags: [crate, session, persistence]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
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

## Timeline
- **2026-04**: Session used as message store alongside RunContext.messages (dual-write)
- **2026-04-25**: Session becomes SSOT — RunContext.messages removed [[session-ssot-redesign]]

## Related Concepts
- [[session-as-ssot]]: Session is the single source of truth
- [[session-contributor]]: Reads messages from Session as context
- [[session-compression]]: Compresses messages in Session
- [[run-context]]: Holds Session reference
- [[vol-llm-agent-crate]]: SessionRecorderPlugin lives here, uses vol-session types
