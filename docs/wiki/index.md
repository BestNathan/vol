# Wiki Index

Last updated: 2026-05-07

## Entities

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[vol-llm-agent-crate]] | ReAct Agent orchestration crate | active | 2026-05-04 |
| [[vol-llm-agents-crate]] | High-level agent implementations (advice, coding, ppt, qa) | active | 2026-05-04 |
| [[vol-llm-core-crate]] | Core LLM interaction abstractions | stable | 2026-05-04 |
| [[vol-llm-tool-crate]] | Tool definition and registry framework | stable | 2026-05-04 |
| [[vol-llm-provider-crate]] | Anthropic and OpenAI provider implementations | stable | 2026-05-04 |
| [[vol-session]] | Session message store and entry persistence | active | 2026-05-04 |
| [[vol-llm-agent-channel-crate]] | Agent communication channel layer with multiple transports | active | 2026-05-05 |
| [[tdengine]] | Time-series database used for market data storage | active | 2026-05-04 |
| [[dashscope]] | DashScope API endpoint for Claude model access | active | 2026-05-04 |

## Concepts

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[react-pattern]] | Reason-Act-Observe agent execution loop | active | 2026-05-04 |
| [[agent-plugin-system]] | Event stream interception architecture for cross-cutting concerns | active | 2026-05-04 |
| [[plugin-actions]] | Plugin return types: Continue, ShortCircuit, Skip, Abort | active | 2026-05-04 |
| [[built-in-plugins]] | HITL, Observability, Caching, Retry, RateLimiter, LokiPlugin plugins | active | 2026-05-06 |
| [[agent-event-stream]] | StreamEvent types and lifecycle hooks | active | 2026-05-04 |
| [[agent-builder-pattern]] | Fluent builder for ReActAgent configuration | stable | 2026-05-04 |
| [[tool-registry]] | Tool registration and execution framework | stable | 2026-05-04 |
| [[tool-trait]] | Tool trait, ToolResult, ToolContext types | stable | 2026-05-04 |
| [[tool-context]] | Tool execution context with alert, messages, metadata | stable | 2026-05-04 |
| [[skill-system]] | Skills as native ReActAgent capability via SkillsConfig | active | 2026-05-04 |
| [[session-as-ssot]] | Session as single source of truth for messages | active | 2026-05-04 |
| [[run-context]] | Unified run state management replacing PluginContext, with model field | active | 2026-05-06 |
| [[context-builder]] | Pluggable prompt construction from contributors | active | 2026-05-04 |
| [[session-contributor]] | Session history as context contributor | active | 2026-05-04 |
| [[session-compression]] | Two-layer session message compression | active | 2026-05-04 |
| [[plugin-context-migration]] | Migration from PluginContext to RunContext | active | 2026-05-04 |
| [[context-error]] | Error type for context building failures | stable | 2026-05-04 |
| [[agent-observability]] | JSONL logging + OTel structured log routing | stable | 2026-05-06 |
| [[otel-log-routing]] | OTel Collector log routing via tracing::info! macros | active | 2026-05-06 |
| [[semantic-caching]] | Response caching with semantic similarity matching | stable | 2026-05-04 |
| [[human-in-the-loop]] | Human approval workflow for tool execution | stable | 2026-05-04 |
| [[retry-with-backoff]] | Automatic retry with exponential backoff on errors | stable | 2026-05-04 |
| [[rate-limiting]] | Concurrency control using semaphore-based rate limiting | stable | 2026-05-04 |
| [[http-transport]] | HTTP transport with blocking and SSE streaming modes | active | 2026-05-05 |
| [[connection-trait]] | Connection trait abstracting transport protocols | active | 2026-05-05 |
| [[connection-holder]] | ConnectionHolder plugin for forwarding agent events | active | 2026-05-05 |
| [[agent-dispatcher]] | FIFO request queueing for single-agent execution | active | 2026-05-05 |
| [[subagent-review-pattern]] | Independent subagent review of documents before user gate | active | 2026-05-06 |
| [[agent-router]] | Multi-agent routing with per-agent dispatchers | active | 2026-05-07 |
| [[connection-holder-clone-limitation]] | ConnectionHolder cannot be both plugin and transport reference | active | 2026-05-07 |
| [[clarifying-requirements-workflow]] | Structured dialogue for turning vague requests into requirements | active | 2026-05-06 |

## Sources

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[agent-channel-examples]] | WS + HTTP service examples using channel primitives | active | 2026-05-07 |
| [[react-agent-docs]] | ReAct Agent plugin system documentation and test report | active | 2026-05-04 |
| [[agent-tool-design]] | AI Agent tool design: Tool trait, registry, built-in tools, ReAct loop | active | 2026-05-04 |
| [[skills-as-react-native]] | Plan: move skill init from CodingAgent into ReActAgent as native capability | active | 2026-05-04 |
| [[session-ssot-redesign]] | Plan: Session as single source of truth, RunContext simplification | active | 2026-05-04 |
| [[http-transport-impl]] | HTTP transport implementation with blocking and SSE modes | active | 2026-05-05 |
| [[clarifying-requirements-subagent-review]] | Subagent review mechanism added to clarifying-requirements skill | active | 2026-05-06 |
| [[loki-plugin-otel-migration-tasks-3-4]] | LokiPlugin rewritten to use tracing::info! + RunContext model field added | active | 2026-05-06 |
| [[otel-029-log-init]] | OTel 0.29 API migration and init_otel_logs() implementation in vol-monitor | active | 2026-05-06 |

## Analyses

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
