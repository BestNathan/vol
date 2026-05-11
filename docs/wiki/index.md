# Wiki Index

Last updated: 2026-05-11 (split-signal-state)

## Entities

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[vol-llm-ui-crate]] | Shared UI state model and connection abstraction, with TUI and Web frontends including FileContentView file tabs | active | 2026-05-10 |
| [[vol-llm-agent-crate]] | ReAct Agent orchestration crate | active | 2026-05-04 |
| [[vol-llm-agents-crate]] | High-level agent implementations (advice, coding, ppt, qa) | active | 2026-05-04 |
| [[vol-llm-core-crate]] | Core LLM interaction abstractions | stable | 2026-05-04 |
| [[vol-llm-tool-crate]] | Tool definition and registry framework | stable | 2026-05-04 |
| [[vol-llm-provider-crate]] | Anthropic and OpenAI provider implementations | stable | 2026-05-04 |
| [[vol-session]] | Session message store and entry persistence | active | 2026-05-04 |
| [[vol-llm-agent-channel-crate]] | Agent communication channel layer with multiple transports and JSON-RPC Connection implementation | active | 2026-05-09 |
| [[tdengine]] | Time-series database used for market data storage | active | 2026-05-04 |
| [[dashscope]] | DashScope API endpoint for Claude model access | active | 2026-05-04 |
| [[vol-mcp-servers-crate]] | MCP server collection with multi-transport support | active | 2026-05-10 |

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
| [[mcp-transport-pattern]] | Multi-transport startup pattern for MCP servers (stdio, HTTP/SSE) | active | 2026-05-10 |
| [[docs-rs-tools]] | Four MCP tools exposing docs.rs/crates.io documentation | active | 2026-05-10 |
| [[rmcp-sdk]] | Rust SDK for Model Context Protocol — macros, transports, service | active | 2026-05-10 |
| [[ratatui-tui-pattern]] | Layout and widget composition patterns for ratatui 0.30 TUI rendering | active | 2026-05-08 |
| [[ui-event-loop-pattern]] | crossterm EventStream + tokio::select! multiplexing for async TUI | active | 2026-05-08 |
| [[dioxus-signal-pattern]] | Signal-based state management with Signal<UiState> via Dioxus context | active | 2026-05-08 |
| [[dioxus-web-pattern]] | Dioxus 0.6 WASM component architecture and rendering patterns | active | 2026-05-08 |
| [[remote-agent-connection]] | AgentConnection and FileOperations traits with local/remote implementations | active | 2026-05-08 |
| [[json-rpc-websocket]] | JSON-RPC 2.0 over WebSocket protocol for remote agent communication | active | 2026-05-08 |
| [[jsonrpc-transport]] | JSON-RPC 2.0 over WebSocket implementing the Connection trait | active | 2026-05-09 |
| [[jsonrpc-server-handler]] | Historical JSON-RPC handler architecture — deleted, replaced by jsonrpc-transport | stale | 2026-05-09 |
| [[file-tab-pattern]] | Tabbed file viewer with non-component render function pattern for Dioxus | active | 2026-05-10 |
| [[workspace-tree-pattern]] | Recursive WorkspaceTreeNode tree with lazy-loaded directory children via JSON-RPC file.list | active | 2026-05-10 |
| [[event-bus-pattern]] | EventBus with UiEventKind routing, SubscriptionSet auto-cleanup, per-component local signals | active | 2026-05-11 |

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
| [[docs-rs-mcp-impl]] | vol-mcp-servers crate with docs-rs-mcp binary, 4 tools, stdio+HTTP/SSE | active | 2026-05-10 |
| [[vol-mcp-servers-dockerfile]] | Single-stage Ubuntu Docker packaging with ARG-based binary selection | active | 2026-05-10 |
| [[tui-frontend-ratatui]] | TUI frontend with ratatui rendering, crossterm event loop, 9 render functions migrated | active | 2026-05-08 |
| [[remote-connection-impl]] | RemoteConnection with JSON-RPC 2.0 WebSocket for vol-llm-ui | active | 2026-05-08 |
| [[task-8-dioxus-web-frontend]] | Web frontend with Dioxus 0.6 WASM, signal-based state, 10 components | active | 2026-05-08 |
| [[task-9-jsonrpc-server]] | JSON-RPC server with 9 methods, JsonRpcHandler/JsonRpcContext, jsonrpsee 0.26 | active | 2026-05-08 |
| [[task-10-final-verification]] | Final verification: 10 tasks complete, 55 tests passing, all feature builds verified | complete | 2026-05-08 |
| [[jsonrpc-transport-refactoring]] | Refactoring: EventBridgePlugin deleted, JsonRpcConnection implements Connection trait | active | 2026-05-09 |
| [[task-5-jsonrpc-integration-tests]] | 44 integration tests for JSON-RPC serialization, parsing, and error handling | active | 2026-05-09 |
| [[task-5-file-content-view]] | FileContentView component: file tab bar with content preview, error/loading states, non-component tab rendering | active | 2026-05-10 |
| [[lazy-load-dir-tree]] | Lazy-loading directory tree: WorkspaceTreeNode replaces flat entries, on-demand fetch via file.list, refresh button | active | 2026-05-10 |
| [[split-signal-state]] | Split Signal state: centralized Signal<UiState> replaced with EventBus + per-component local signals | active | 2026-05-11 |

## Analyses

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
