---
type: source
source_type: code
date: 2026-05-19
ingested: 2026-05-19
tags: [react-agent, plugin-system, shutdown, tracing, mcp]
---

# ReAct Plugin Event Shutdown

**Authors/Creators:** Claude Code + project maintainers
**Date:** 2026-05-19
**Link:** `crates/vol-llm-agent/src/react/`, `crates/vol-llm-tool/src/mcp_tool.rs`

## TL;DR

The ReAct plugin event path now preserves `TracedEvent` context through plugin emit requests and uses explicit sender ownership to make plugin interceptor/listener shutdown channel-driven instead of timeout-driven. `RunContext` stores event and plugin request senders as optional shared handles, infrastructure contexts drop the senders they must not keep alive, and listener tasks drain in-flight `plugin.listen()` work before returning.

## Key Takeaways

- `PluginRequest::Emit` continues carrying `TracedEvent<AgentStreamEvent>` and forwards through `RunContext::emit_traced()` without rewrapping or changing the trace id.
- `run_interceptor_loop` no longer receives a separate `event_tx`; it receives `plugin_rx`, plugins, and a `RunContext`.
- `RunContext` uses optional `Arc` sender handles so normal clones share channel ownership, while helper contexts can remove `plugin_event_tx` or both event senders.
- Agent shutdown now awaits the interceptor, drops the final event sender, then awaits the listener without normal 5-second timeout/abort behavior.
- `spawn_listener_task` tracks per-event `plugin.listen()` work with a `JoinSet` and drains it before returning.
- `McpTool` was aligned with the current `McpManager`-based registry path as a compile unblock for the current branch.

## Detailed Summary

The ReAct plugin interceptor previously accepted a direct `event_tx` broadcast sender in addition to `RunContext`. This made the interceptor own event emission separately from context APIs and complicated shutdown reasoning. The new flow keeps event emission behind `RunContext`: local events use `emit()`, which wraps values with `TracedEvent::without_span`, while already traced plugin events use `emit_traced()`.

Channel ownership is now explicit. `RunContext` stores `event_tx` and `plugin_event_tx` as optional `Arc` sender handles. `without_plugin_event_tx()` creates an interceptor-capable context that can still emit traced events but cannot keep `plugin_rx` open. `without_event_senders()` creates listener callback contexts that hold neither sender, allowing the broadcast channel to close after the agent and interceptor are done.

Shutdown is drop-driven. The agent awaits the interceptor after the agent task completes; once all plugin request senders are gone, `plugin_rx.recv().await` returns `None`. The agent then drops the final event sender and awaits the listener. Listener completion now includes draining spawned `plugin.listen()` tasks, which keeps observability or logging side effects from outliving `ReActAgent::run()`.

Verification included `cargo test -p vol-llm-agent test_run_interceptor_loop -- --nocapture` and `cargo check -p vol-llm-agent`. A full `cargo test -p vol-llm-agent -- --nocapture` progressed through the related unit tests but was stopped when an external docs MCP child process emitted EPIPE/hung in an unrelated agent tool test.

## Entities Mentioned

- [[vol-llm-agent-crate]]: owns ReAct plugin event emission and shutdown logic.
- [[vol-llm-tool-crate]]: owns `McpTool`, updated to match `McpManager` registry usage.

## Concepts Covered

- [[agent-plugin-system]]: plugin interception/listen event path and lifecycle.
- [[run-context]]: shared run state and channel ownership model.
- [[mcp-manager-lifecycle]]: related compile unblock aligns tool proxy with `McpManager`.

## Notes

The sender-removal helpers intentionally make plugin infrastructure contexts unable to recursively submit plugin requests. `emit()`/`emit_traced()` still assume a context with an event sender; listener callback contexts should not call them.
