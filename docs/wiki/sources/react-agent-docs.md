---
type: source
source_type: report
date: 2026-04-06
ingested: 2026-05-04
tags: [react-agent, plugin-system, vol-llm-agent]
---

# ReAct Agent Plugin System

**Authors/Creators:** vol-monitor team
**Date:** 2026-04-06
**Link:** `docs/ai-agent/react-plugin-system.md`

## TL;DR
A plugin system for ReAct Agent that injects cross-cutting concerns into the agent execution flow through event stream interception.

## Key Takeaways
- Plugins intercept events in the agent event stream with 5 action types (Continue, ShortCircuit, Skip, Abort, Continue(None))
- Priority-based execution: lower number = higher priority = executed first
- 5 built-in plugins: RateLimiter (5), Observability (10), Caching (20), HITL (25), Retry (30)
- PluginContext provides shared state across plugins via key-value storage
- HTTP approval channel for remote approval via callbacks
- Full test coverage: 10/10 tests passing across code agent simulation, mock, and integration tests

## Detailed Summary

The ReAct Agent plugin system allows injecting cross-cutting concerns without modifying core agent logic. The `AgentPlugin` trait defines 4 hook points: `on_start`, `intercept`, `on_complete`, and `on_error`. Plugins are executed in priority order, with rate limiter executing first and retry executing last.

The architecture uses a `PluginStream` that sits between the Agent Core and the output, intercepting all events. Each event flows through all plugins in priority order, with each plugin potentially transforming, dropping, short-circuiting, or aborting the event.

The system is async-first, thread-safe (Send + Sync), and composable. It supports feature-gated HTTP functionality for remote approval workflows.

## Entities Mentioned
- [[vol-llm-agent-crate]]: Core implementation of ReActAgent and plugin system
- [[vol-llm-agents-crate]]: High-level agent implementations using the plugin system
- [[vol-session]]: Session message store, SSOT for conversation messages
- [[tdengine]]: Time-series database queried by agent tools
- [[dashscope]]: API endpoint used for LLM access in tests
- [[vol-llm-agent-channel-crate]]: Channel layer with dispatcher, router, and transports

## Concepts Covered
- [[react-pattern]]: The core Reason-Act-Observe cycle that plugins intercept
- [[agent-plugin-system]]: The event interception architecture
- [[plugin-actions]]: The return types that control agent behavior
- [[built-in-plugins]]: The 5 built-in plugins and their configurations
- [[agent-event-stream]]: The event types and lifecycle that plugins hook into
- [[agent-observability]]: The observability plugin for tracing and audit logging
- [[human-in-the-loop]]: HITL plugin for requiring human approval
- [[semantic-caching]]: Caching plugin with semantic similarity and TTL
- [[retry-with-backoff]]: Retry plugin with exponential backoff
- [[rate-limiting]]: Rate limiter plugin with semaphore-based concurrency control
- [[agent-builder-pattern]]: Fluent builder for registering plugins with agent
- [[tool-registry]]: Tools that the agent calls, which plugins can intercept
- [[run-context]]: Replaced PluginContext as the context type for plugins
- [[session-as-ssot]]: Session stores messages, context built on-demand
- [[context-builder]]: Constructs prompt from SessionContributor and other contributors
- [[skill-system]]: Skills as native ReActAgent capability

## Notes
- All tests pass (10/10) but integration tests use DashScope coding endpoint which returns 405 for non-coding requests
- The test report documents detailed API request/response formats for Anthropic-compatible endpoints
- Agent supports max_iterations=5 default, configurable via builder
