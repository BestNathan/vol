---
type: concept
category: framework
tags: [session, ssot, message-storage]
created: 2026-05-04
updated: 2026-06-10
source_count: 3
---

# Session as SSOT

**Category:** Message management architecture
**Related:** [[run-context]], [[context-builder]], [[session-contributor]], [[session-compression]], [[agent-event-stream]]

## Definition

An architectural pattern where the Session is the single source of truth (SSOT) for all agent conversation messages. `RunContext` holds only a reference to Session and builds context on-demand — no duplicate message storage.

## Key Points
- RunContext no longer maintains its own `messages` vector [[session-ssot-redesign]]
- All message writes go only to Session — single write path [[session-ssot-redesign]]
- Context is built on every call via `get_context(user_input)` → `ContextBuilder` — no caching [[session-ssot-redesign]]
- `init_messages()` and `get_messages()` deleted from RunContext [[session-ssot-redesign]]
- Resume flow simplified: pass existing session into agent config, call `run()` — history auto-loaded [[session-ssot-redesign]]
- `PluginContext.messages` deleted — plugins work through events only [[session-ssot-redesign]]
- Session persistence can now be file-backed or database-backed without changing the SSOT model; `Session` still wraps `Arc<dyn SessionEntryStore>` [[session-database-store-implementation]]

## How It Works

### Before (dual-write)

```
RunContext
  ├── messages: Arc<RwLock<Vec<Message>>>    ← runtime copy
  ├── session: Arc<Session>                  ← persistent copy
  └── add_message() → writes BOTH            ← needs sync
```

### After (Session as SSOT)

```
RunContext
  └── session: Arc<Session>                  ← only message store
  └── get_context(input) → build on-demand   ← no cache
  └── add_message() → writes Session only    ← single write path
```

`RunContext.get_context()` creates a `ContextBuilder` with contributors from the config (system prompt, context files), a new `SessionContributor` (reads from Session), and a `UserInputContributor`. The builder assembles the full prompt on every call.

`RunContext.add_message()` writes only to Session, tracking `last_message_id` for parent_id ordering. No secondary copy is maintained.

Resume is handled by passing an existing Session (with its entry store containing checkpoint messages) into the agent config. When `run()` calls `get_context()`, the `SessionContributor` reads all history including checkpoints automatically.

The database session store preserves this model: only the entry store implementation changes. Runtime/channel code selects the store through `SessionManager`, then passes the resulting `Arc<dyn SessionEntryStore>` into `Session` just like the file and in-memory paths.

## Examples / Applications

- **Multi-turn conversations**: Each turn's messages stored in Session, context rebuilt from full history
- **Resuming sessions**: Pass the same Session to a new agent run — history included automatically
- **Session compression**: `SessionContributor.compress()` mutates Session in place; next `contribute()` sees compressed result [[session-compression]]

## Related Concepts
- [[run-context]]: Holds the Session reference
- [[context-builder]]: Builds context from Session on-demand
- [[session-contributor]]: Reads messages from Session as context blocks
- [[session-compression]]: Compresses Session messages to reduce token usage
- [[agent-builder-pattern]]: Configures Session and context builder
- [[agent-event-stream]]: Events emitted, no longer carries messages
- [[runtime-session-store-configuration]]: Selects file or database persistence for the same Session SSOT model
- [[session-database-store-implementation]]: Implements database-backed session entries
