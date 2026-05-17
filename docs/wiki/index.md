# Wiki Index

Last updated: 2026-05-17 (frontend-auto-reconnect)

## Entities

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[vol-llm-ui-crate]] | Shared UI state model and connection abstraction, with TUI and Web frontends including FileContentView file tabs, ConnectionStatePanel connection status dashboard, MCP state types, Skills panel with detail dialog, and auto-reconnect with session restoration | active | 2026-05-17 |
| [[vol-llm-agent-crate]] | ReAct Agent orchestration crate | active | 2026-05-04 |
| [[vol-llm-agents-crate]] | High-level agent implementations (advice, coding, ppt, qa) with runnable MCP examples | active | 2026-05-11 |
| [[vol-llm-core-crate]] | Core LLM interaction abstractions | stable | 2026-05-04 |
| [[vol-llm-tool-crate]] | Tool definition and registry framework | stable | 2026-05-04 |
| [[vol-llm-provider-crate]] | Anthropic and OpenAI provider implementations with LLMClient trait; protocol-abstracted SSE streaming parsers (AnthropicProtocol, OpenaiStreamParser); factory dispatch and TOML-based config | stable | 2026-05-15 |
| [[vol-session]] | Session message store and entry persistence | active | 2026-05-04 |
| [[vol-llm-agent-channel-crate]] | Agent communication channel layer with multiple transports and JSON-RPC Connection implementation, plus skill.list/skill.get RPC methods for skill discovery | active | 2026-05-16 |
| [[tdengine]] | Time-series database used for market data storage | active | 2026-05-04 |
| [[dashscope]] | DashScope API endpoint for Claude model access | active | 2026-05-04 |
| [[vol-mcp-servers-crate]] | MCP server collection with multi-transport support | active | 2026-05-10 |
| [[vol-llm-mcp-crate]] | MCP Client protocol layer for ReAct Agent — config parsing with multi-transport enum (Stdio/Http), McpManager lifecycle, tool/resource/prompt discovery | active | 2026-05-15 |

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
| [[mcp-transport-pattern]] | Multi-transport startup pattern for MCP servers (stdio, HTTP/SSE) — server-side (CLI flags) and client-side (type-field config parsing) | active | 2026-05-15 |
| [[mcp-manager-lifecycle]] | McpManager connection lifecycle: multi-transport dispatch (Stdio/Http), state tracking, auto-reconnect with backoff, full MCP protocol | active | 2026-05-15 |
| [[docs-rs-tools]] | Four MCP tools exposing docs.rs/crates.io documentation | active | 2026-05-10 |
| [[rmcp-sdk]] | Rust SDK for Model Context Protocol — macros, transports, service | active | 2026-05-10 |
| [[ratatui-tui-pattern]] | Layout and widget composition patterns for ratatui 0.30 TUI rendering | active | 2026-05-08 |
| [[ui-event-loop-pattern]] | crossterm EventStream + tokio::select! multiplexing for async TUI | active | 2026-05-08 |
| [[dioxus-signal-pattern]] | Signal-based state management with Signal<UiState> via Dioxus context | active | 2026-05-08 |
| [[dioxus-web-pattern]] | Dioxus 0.6 WASM component architecture and rendering patterns, 18+ components including auto-reconnect with exponential backoff and session restoration | active | 2026-05-17 |
| [[remote-agent-connection]] | AgentConnection and FileOperations traits with local/remote implementations | active | 2026-05-08 |
| [[json-rpc-websocket]] | JSON-RPC 2.0 over WebSocket protocol for remote agent communication, with auto-reconnect on web frontend | active | 2026-05-17 |
| [[jsonrpc-transport]] | JSON-RPC 2.0 over WebSocket implementing the Connection trait | active | 2026-05-09 |
| [[jsonrpc-server-handler]] | Historical JSON-RPC handler architecture — deleted, replaced by jsonrpc-transport | stale | 2026-05-09 |
| [[file-tab-pattern]] | Tabbed file viewer with non-component render function pattern for Dioxus | active | 2026-05-10 |
| [[workspace-tree-pattern]] | Recursive WorkspaceTreeNode tree with lazy-loaded directory children via JSON-RPC file.list | active | 2026-05-10 |
| [[event-bus-pattern]] | EventBus with UiEventKind routing, SubscriptionSet auto-cleanup, per-component local signals, connection status event handling (WsConnected/WsConnecting/WsDisconnected/WsReconnecting/WsReconnectFailed/WsReconnected) | active | 2026-05-17 |
| [[mcp-client-integration]] | Bridging MCP server tools into ExecutableTool trait — McpTool, McpSession, AgentConfigBuilder integration | active | 2026-05-11 |
| [[mcp-example-pattern]] | Pattern for runnable example files demonstrating MCP integration with ReActAgent | active | 2026-05-11 |
| [[sessions-ui-pattern]] | Tab-based session browsing with SessionsState signal, SessionsPanel component, checkpoint CSS | active | 2026-05-11 |
| [[connection-state-dashboard-pattern]] | ConnectionStatePanel component subscribing to WsConnected/WsConnecting/WsDisconnected via EventBus, rendering color-coded status in StatusBar | active | 2026-05-14 |
| [[tailwind-css-migration]] | Systematic migration from global CSS to Tailwind utility classes — ALL 16 components complete, GLOBAL_CSS deleted | complete | 2026-05-12 |
| [[agent-error-handling]] | Hierarchical error types with retryable vs non-retryable classification and exponential backoff | active | 2026-05-14 |
| [[loki-plugin-otel-migration-design]] | Design spec for migrating LokiPlugin from HTTP POST to OTel SDK via tracing::info! | active | 2026-05-14 |
| [[loki-raw-event-serialization-design]] | Design spec for flat JSON serialization format of agent events | active | 2026-05-14 |
| [[streaming-session]] | StreamProtocol trait and StreamingSession — protocol-abstracted SSE parsing for Anthropic and OpenAI formats | active | 2026-05-15 |
| [[otel-dependency-upgrade]] | Workspace dependency upgrade from OTel 0.21 to 0.29 with breaking API changes | active | 2026-05-14 |
| [[mcp-state-types]] | State types and wire structures for displaying MCP servers, tools, resources, and prompts in the Dioxus web frontend | active | 2026-05-14 |
| [[schema-form-pattern]] | Auto-generated form fields from JSON Schema — SchemaForm component with type-specific renderers (string, number, boolean, object, enum) | active | 2026-05-16 |
| [[skills-panel-json-rpc]] | Exposing skill discovery via JSON-RPC — skill.list/skill.get methods with graceful degradation, lazy detail loading | active | 2026-05-16 |
| [[frontend-auto-reconnect]] | WebSocket auto-reconnect with exponential backoff and session restoration on Dioxus web frontend — 10 max retries, countdown display, automatic conversation rebuild | active | 2026-05-17 |

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
| [[schemaform-toolcall-dialog]] | Task 3: SchemaForm integration into ToolCallDialog — replaced raw JSON textarea with auto-generated form from tool JSON Schema | active | 2026-05-16 |
| [[lazy-load-dir-tree]] | Lazy-loading directory tree: WorkspaceTreeNode replaces flat entries, on-demand fetch via file.list, refresh button | active | 2026-05-10 |
| [[split-signal-state]] | Split Signal state: centralized Signal<UiState> replaced with EventBus + per-component local signals | active | 2026-05-11 |
| [[react-agent-mcp-integration]] | ReAct Agent MCP client integration: vol-llm-mcp crate, McpTool, McpManager, with_mcp_from_config builder method | active | 2026-05-13 |
| [[mcp-manager-impl]] | Source: McpManager replaces McpSession — connection state, auto-reconnect, full MCP protocol (tools, resources, prompts) | active | 2026-05-13 |
| [[docs-rs-mcp-example]] | Runnable example: ReActAgent connecting to docs-rs MCP server via with_mcp_from_config() | active | 2026-05-11 |
| [[task-6-sessions-tab-wiring]] | Sessions tab wired into App: SessionsState signal, SessionsPanel, TabBar, CSS, checkpoint rendering | active | 2026-05-11 |
| [[conversation-tailwind-migration]] | conversation.rs migrated from semantic CSS classes to inline Tailwind utilities — all 9 message types updated | active | 2026-05-12 |
| [[connection-state-dashboard]] | ConnectionStatePanel component: EventBus-driven connection status indicator in StatusBar, color-coded for connected/connecting/disconnected states | active | 2026-05-14 |
| [[tailwind-css-full-migration]] | Full Tailwind CSS v4 migration — all 16 components, GLOBAL_CSS deleted, build pipeline verified | complete | 2026-05-12 |
| [[mcp-multi-transport-config]] | Design + implementation: McpTransport enum with required type field, serde tagged enum parsing, HTTP via StreamableHttpClientTransport | active | 2026-05-15 |
| [[openai-stream-parser-impl]] | OpenaiStreamParser implementation — StreamProtocol for OpenAI SSE format with [DONE] sentinel, content/tool deltas, usage, finish reasons | active | 2026-05-15 |
| [[openai-provider-impl]] | OpenaiProvider implementation — full LLMClient for OpenAI Chat Completions API, message/tool conversion, SSE streaming, factory dispatch | active | 2026-05-15 |
| [[tool-call-dialog-component]] | ToolCallDialog Dioxus component — modal dialog for invoking MCP tools with SchemaForm for structured parameter input, async RPC call, inline result/error display | active | 2026-05-16 |
| [[schemaform-toolcall-dialog]] | Task 3: SchemaForm integration into ToolCallDialog — replaced raw JSON textarea with auto-generated form from tool JSON Schema | active | 2026-05-16 |
| [[mcp-toolcall-input-schema]] | Task 1: Added input_schema field to McpToolCallState for SchemaForm support | active | 2026-05-16 |
| [[skills-panel-content]] | Backend JSON-RPC + web UI detail dialog for browsing discovered skills — skill.list/skill.get RPC, SkillLoader integration, SkillDetailDialog modal | active | 2026-05-16 |
| [[frontend-auto-reconnect]] | WebSocket auto-reconnect with exponential backoff and session restoration on Dioxus web frontend | active | 2026-05-17 |

## Analyses

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
