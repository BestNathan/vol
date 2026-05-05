---
type: source
source_type: plan
date: 2026-04-25
ingested: 2026-05-04
tags: [session, ssot, context-management]
---

# Session as SSOT — ReAct Agent Context Redesign

**Authors/Creators:** vol-monitor team
**Date:** 2026-04-25
**Link:** `docs/superpowers/plans/2026-04-25-session-ssot-redesign.md`

## TL;DR
Remove dual-write message synchronization from ReAct Agent. Session becomes the single source of truth for all conversation messages. `RunContext` no longer maintains its own `messages` vector; context is built on-demand from Session via `ContextBuilder`.

## Key Takeaways
- **Before**: RunContext maintained both `messages: Arc<RwLock<Vec<Message>>>` (runtime copy) and `session: Arc<Session>` (persistent copy) — dual-write requiring synchronization
- **After**: RunContext only holds `session: Arc<Session>`; `get_context()` builds context on-demand via ContextBuilder
- `init_messages()` and `get_messages()` deleted from RunContext; replaced by `get_context(user_input)`
- `ContextContributor` trait's `contribute()` method now returns `Result<Vec<ContextBlock>, ContextError>` — errors propagate, no partial context
- `SessionContributor` simplified: deleted `cached_blocks`, reads directly from Session
- `PluginContext.messages` field deleted — plugins work entirely through events
- `ReActAgent.resume()` method deleted — caller passes existing session into Config, calls `run()`
- `ContextBuilder.build()` now returns `Result<ContextOutput, ContextError>` instead of `ContextOutput`
- Resume flow simplified: `Session + SessionEntryStore` already persist checkpoint state, `get_context()` auto-includes all checkpointed messages

## Detailed Summary

This plan fundamentally changes how messages are managed in the agent. The previous dual-write architecture maintained a runtime copy of messages in RunContext alongside the persistent Session store, requiring synchronization logic. The new architecture eliminates this by making Session the only message store.

The `ContextContributor` trait becomes fallible, allowing contributors to return errors. A new `ContextError` enum covers contributor errors, budget exceeded, and session errors. The `ContextBuilder.build()` loop now propagates errors — if any contributor fails, the build aborts.

`SessionContributor` is dramatically simplified: no caching, no `cached_blocks` field. It reads messages from Session on every `contribute()` call, applies the `max_history` limit, and returns the blocks. The Session itself acts as the cache.

`RunContext.add_message()` now writes only to Session, with a single write path that tracks `last_message_id` for parent_id ordering. The `get_context()` method builds a fresh context on every call by creating a `ContextBuilderBuilder` with contributors from the config, a new `SessionContributor`, and a `UserInputContributor`.

The resume flow migration eliminates the special `resume()` method. Instead, callers store a Session reference between runs and pass it into the next agent config. When `run()` is called, `get_context()` → `SessionContributor.contribute()` → `session.get_messages()` automatically includes all checkpointed messages.

## Entities Mentioned
- [[vol-llm-agent-crate]]: Where RunContext and ReActAgent are modified
- [[vol-llm-agents-crate]]: CodingAgent/advice agents affected
- [[vol-session]]: Session crate providing message store

## Concepts Covered
- [[session-as-ssot]]: Session as the single source of truth for messages
- [[context-builder]]: On-demand context construction with error propagation
- [[session-contributor]]: Reads messages from Session as a context contributor
- [[run-context]]: Simplified RunContext without messages vector
- [[context-error]]: New error type for context building failures
- [[session-compression]]: Session compression mentioned in SessionContributor.compress()

## Notes
- This is a migration plan with 10 steps across 5 crates
- Error handling ensures no partial state on Session read/write failures
