# Wiki Index

Last updated: 2026-06-16 (argocd-gitops-deployment)

## Entities

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[vol-llm-runtime-crate]] | AgentRuntime owner of shared agent resources, runtime task/session store config types, and data-plane capability source | active | 2026-06-10 |
| [[vol-llm-task-crate]] | Task models and persistence stores, including SeaORM database store for SQLite and Postgres with compiled migrations | active | 2026-06-09 |
| [[vol-agent-server-crate]] | Standalone server crate that composes DataPlaneServerCore/ControlPlaneServerCore routes and is deployed by the self-contained ArgoCD GitOps tree as `agent-server` | active | 2026-06-16 |
| [[vol-llm-ui-crate]] | Shared UI state model and connection abstraction, with Dioxus as the sole active web frontend | active | 2026-05-29 |
| [[vol-llm-agent-crate]] | ReAct Agent orchestration crate with structured `AgentInput` multimodal run API | active | 2026-05-21 |
| [[vol-llm-agents-crate]] | High-level agent implementations (advice, coding, ppt, qa) with runnable MCP examples | active | 2026-05-11 |
| [[vol-llm-core-crate]] | Core LLM interaction abstractions, including provider-neutral multipart message content | stable | 2026-05-21 |
| [[vol-llm-tool-crate]] | Tool definition and registry framework with MCP tool proxying through McpManager | stable | 2026-05-21 |
| [[vol-llm-provider-crate]] | Anthropic and OpenAI provider implementations with Anthropic multipart text/image conversion | stable | 2026-05-21 |
| [[vol-session]] | Session message store and entry persistence, including file and SeaORM database-backed session managers | active | 2026-06-10 |
| [[vol-llm-agent-protocol-crate]] | Protocol, JSON-RPC transport, connection, handler, registry, and generic service abstraction layer | active | 2026-06-10 |
| [[tdengine]] | Time-series database used for market data storage | active | 2026-05-04 |
| [[dashscope]] | DashScope API endpoint for Claude model access | active | 2026-05-04 |
| [[vol-mcp-servers-crate]] | MCP server collection with multi-transport support; `docs-rs-mcp` is GitOps-managed and built by the MCP image workflow | active | 2026-06-16 |
| [[vol-repository]] | Rust workspace with crates, docs, legacy `k8s/` manifests, and self-contained `deploy/argocd/` GitOps deployment tree | active | 2026-06-16 |
| [[vol-llm-mcp-crate]] | MCP Client protocol layer for ReAct Agent — config parsing, McpManager lifecycle, tool/resource/prompt discovery | active | 2026-05-13 |

## Concepts

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[argocd-app-of-apps-gitops]] | Self-contained ArgoCD App-of-Apps deployment pattern under `deploy/argocd/`, with CI-built MCP images updating GitOps manifests | active | 2026-06-16 |
| [[agent-server-control-data-plane]] | Single server crate with DataPlaneServerCore/ControlPlaneServerCore, channel-owned JSON-RPC protocol, route composition, data-plane snapshot facade, command/run semantics, control-plane router MVP, role-mode verification tests, and dependency boundary checks | draft | 2026-06-10 |
| [[runtime-session-store-configuration]] | Shared `[runtime.session_store]` TOML contract and runtime `SessionManager` behavior for file/database session persistence | active | 2026-06-10 |
| [[runtime-task-store-configuration]] | Shared `[runtime.task_store]` TOML contract and single global runtime store behavior for file/database task persistence | active | 2026-06-09 |
| [[rich-text-conversation]] | Markdown rendering for chat (Dioxus handoff to marked.js + DOMPurify + highlight.js) | active | 2026-06-04 |
| [[dependency-graph-visualization]] | Layered SVG node-link graph of task dependencies: pure layout fn + Dioxus component | active | 2026-06-04 |
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
| [[http-transport]] | Historical HTTP transport with blocking and SSE streaming modes; deleted from active channel API after Task 4 | stale | 2026-06-10 |
| [[connection-trait]] | Connection trait abstracting transport protocols | active | 2026-05-05 |
| [[connection-holder]] | ConnectionHolder plugin for forwarding agent events | active | 2026-05-05 |
| [[agent-dispatcher]] | FIFO request queueing for single-agent execution | active | 2026-05-05 |
| [[subagent-review-pattern]] | Independent subagent review of documents before user gate | active | 2026-05-06 |
| [[agent-router]] | Node-local multi-agent routing with per-agent dispatchers; distributed routing sits above it | active | 2026-06-10 |
| [[connection-holder-clone-limitation]] | ConnectionHolder cannot be both plugin and transport reference | active | 2026-05-07 |
| [[clarifying-requirements-workflow]] | Structured dialogue for turning vague requests into requirements | active | 2026-05-06 |
| [[mcp-transport-pattern]] | Multi-transport startup pattern for MCP servers (stdio, HTTP/SSE) | active | 2026-05-10 |
| [[mcp-manager-lifecycle]] | McpManager connection lifecycle: state tracking, auto-reconnect with backoff, full MCP protocol | active | 2026-05-13 |
| [[docs-rs-tools]] | Four MCP tools exposing docs.rs/crates.io documentation | active | 2026-05-10 |
| [[rmcp-sdk]] | Rust SDK for Model Context Protocol — macros, transports, service | active | 2026-05-10 |
| [[ratatui-tui-pattern]] | Layout and widget composition patterns for ratatui 0.30 TUI rendering | active | 2026-05-08 |
| [[ui-event-loop-pattern]] | crossterm EventStream + tokio::select! multiplexing for async TUI | active | 2026-05-08 |
| [[dioxus-signal-pattern]] | Signal-based state management with Signal<UiState> via Dioxus context | active | 2026-05-08 |
| [[dioxus-web-pattern]] | Dioxus 0.6 WASM component architecture and rendering patterns | active | 2026-05-08 |
| [[remote-agent-connection]] | AgentConnection and FileOperations traits with local/remote implementations | active | 2026-05-08 |
| [[json-rpc-websocket]] | JSON-RPC 2.0 over WebSocket protocol for remote agent communication | active | 2026-05-08 |
| [[jsonrpc-transport]] | JSON-RPC 2.0 over WebSocket with `Connection`, generic `JsonRpcMessageService`, and configured server mount path | active | 2026-06-10 |
| [[jsonrpc-server-handler]] | Historical JSON-RPC handler architecture — deleted, replaced by jsonrpc-transport | stale | 2026-05-09 |
| [[file-tab-pattern]] | Tabbed file viewer with non-component render function pattern for Dioxus | active | 2026-05-10 |
| [[workspace-tree-pattern]] | Recursive WorkspaceTreeNode tree with lazy-loaded directory children via JSON-RPC file.list | active | 2026-05-10 |
| [[event-bus-pattern]] | EventBus with UiEventKind routing, SubscriptionSet auto-cleanup, per-component local signals | active | 2026-05-11 |
| [[mcp-client-integration]] | Bridging MCP server tools into ExecutableTool trait — McpTool, McpSession, AgentConfigBuilder integration | active | 2026-05-11 |
| [[mcp-example-pattern]] | Pattern for runnable example files demonstrating MCP integration with ReActAgent | active | 2026-05-11 |
| [[sessions-ui-pattern]] | Tab-based session browsing with SessionsState signal, SessionsPanel component, checkpoint CSS | active | 2026-05-11 |
| [[tailwind-css-migration]] | Systematic migration from global CSS to Tailwind utility classes — ALL 16 components complete, GLOBAL_CSS deleted | complete | 2026-05-12 |
| [[agentinput-multimodal-run]] | Structured ReActAgent run input envelope for text/image parts, run_id, metadata, and protocol compatibility | active | 2026-05-21 |

## Sources

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[argocd-gitops-deployment]] | Self-contained ArgoCD GitOps implementation: App-of-Apps, vol-agent-system manifests, MCP Dockerfile, and MCP image workflow | active | 2026-06-16 |
| [[control-plane-behavior-completion-plan]] | Follow-up plan to complete JSON-RPC notifications, endpoint roles, client handlers, control.command, run status, and combined-mode registration | draft | 2026-06-10 |
| [[agent-server-boundary-mode-verification]] | Task 10 boundary and role-mode verification: cargo-tree dependency guard plus `/ws` ownership and disabled-role config tests | active | 2026-06-10 |
| [[agent-server-control-router-mvp]] | Task 9 control router MVP: routes targeted or untargeted agents to online nodes using capability snapshots | active | 2026-06-10 |
| [[agent-server-data-plane-snapshot-command]] | Task 8 data-plane primitives: runtime capability snapshot facade, static source, fake-source test, and control command acceptance skeleton | active | 2026-06-10 |
| [[agent-server-health-route-collision-validation]] | Task 7 quality fix rejecting active WebSocket paths that collide with `/health` before Axum can panic on duplicate routes | active | 2026-06-10 |
| [[agent-server-role-route-composition]] | Task 7 role route composition: pure `/ws` ownership tests, role-specific core construction, configured control/data WebSocket mounting, and main startup delegation | active | 2026-06-10 |
| [[agent-server-control-plane-core-handlers]] | Task 6 control-plane core and handlers: register/heartbeat/snapshot/event, node list/get, capability list, and JsonRpcMessageService loop | active | 2026-06-10 |
| [[task-4-quality-issues-cleanup]] | Task 4 follow-up cleanup for channel dependency scopes, generic JSON-RPC docs, active backend ownership, and moved-routing comments | active | 2026-06-10 |
| [[agent-server-data-plane-core-move]] | Task 4 migration moving concrete data-plane core/router/dispatcher/handlers from channel into vol-agent-server::data_plane | active | 2026-06-10 |
| [[agent-server-role-config-route-skeleton]] | Task 3 server role config and base Axum route skeleton for future control/data-plane composition | active | 2026-06-10 |
| [[control-payload-flat-jsonrpc-encoding-fix]] | Task 2 code-quality fix aligning `ControlPayload` serialization with flat JSON-RPC `control.*` params/results and codec tests | active | 2026-06-10 |
| [[agent-server-control-data-plane-implementation-plan]] | Staged implementation plan for generic channel JSON-RPC service, control protocol, data-plane core move, control-plane core, routing, and tests | draft | 2026-06-10 |
| [[agent-server-control-data-plane-addendum]] | Addendum detailing endpoint allowlists, command/run semantics, capability revisions, node sessions, lifecycle, and migration tests | draft | 2026-06-10 |
| [[agent-server-control-data-plane-architecture]] | Architecture for channel-owned JSON-RPC protocol and agent-server-owned data/control server cores | draft | 2026-06-10 |
| [[session-database-store-implementation]] | End-to-end file/database session-store implementation: SessionManager, SeaORM SQLite/Postgres store, runtime/server config, channel JSON-RPC integration | active | 2026-06-10 |
| [[file-session-agent-id-validation]] | FileSessionManager agent-id path traversal hardening with validation, InvalidInput errors, and encoded quarantine stores | active | 2026-06-09 |
| [[seaorm-task-database-store-implementation]] | End-to-end replacement of SQLx task store with SeaORM + SeaORM Migration for SQLite and Postgres | active | 2026-06-09 |
| [[seaorm-postgres-test-url-env-fix]] | SeaORM Postgres task-store test URL hardening: mandatory `VOL_AGENT_POSTGRES_TEST_URL`, clear unset failure, and placeholder docs DSN | active | 2026-06-09 |
| [[seaorm-postgres-test-isolation-fix]] | SeaORM Postgres task-store test isolation: shared temp-dir file lock, UUID marker cleanup, and placeholder config DSN | active | 2026-06-09 |
| [[seaorm-sqlite-url-normalization-fix]] | SeaORM SQLite URL normalization fix: exact `mode` query-key detection so `journal_mode=wal` still appends `mode=rwc` | active | 2026-06-09 |
| [[task-database-store-implementation]] | End-to-end implementation of global SQLx SQLite database-backed task store | active | 2026-06-09 |
| [[runtime-database-task-store-construction]] | AgentRuntime database task-store construction and persistence test hardening | active | 2026-06-09 |
| [[task-store-sqlite-embedded-migrations]] | SQLite task-store migrations embedded into the `vol-llm-task` binary via SQLx macros | active | 2026-06-09 |
| [[task-store-config-parsing]] | Runtime task store config parsing and validation for `[runtime.task_store]` | active | 2026-06-09 |
| [[rich-text-conversation-design]] | Design spec for markdown rendering in chat (Dioxus + marked.js) | active | 2026-06-04 |
| [[task-dependency-graph-view]] | Tasks tab "⇄ deps" button + SVG dependency-graph modal (read-only, frontend-only) | active | 2026-06-04 |
| [[agent-channel-examples]] | Historical WS + HTTP channel examples; source files deleted after Task 4 cleanup | stale | 2026-06-10 |
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
| [[react-agent-mcp-integration]] | ReAct Agent MCP client integration: vol-llm-mcp crate, McpTool, McpManager, with_mcp_from_config builder method | active | 2026-05-13 |
| [[mcp-manager-impl]] | Source: McpManager replaces McpSession — connection state, auto-reconnect, full MCP protocol (tools, resources, prompts) | active | 2026-05-13 |
| [[docs-rs-mcp-example]] | Runnable example: ReActAgent connecting to docs-rs MCP server via with_mcp_from_config() | active | 2026-05-11 |
| [[task-6-sessions-tab-wiring]] | Sessions tab wired into App: SessionsState signal, SessionsPanel, TabBar, CSS, checkpoint rendering | active | 2026-05-11 |
| [[conversation-tailwind-migration]] | conversation.rs migrated from semantic CSS classes to inline Tailwind utilities — all 9 message types updated | active | 2026-05-12 |
| [[tailwind-css-full-migration]] | Full Tailwind CSS v4 migration — all 16 components, GLOBAL_CSS deleted, build pipeline verified | complete | 2026-05-12 |
| [[agentinput-multimodal-run-implementation]] | AgentInput multimodal run implementation: run_input, Anthropic multipart conversion, channel compatibility | active | 2026-05-21 |
| [[agentinput-channel-unification]] | Channel crate unified to use AgentInput directly: Submit payload, AgentRequest, dispatcher all switched from String | active | 2026-05-22 |
| [[jsonrpc-transport-consolidation]] | JSON-RPC transport consolidated: jsonrpc/ and gateway/ moved into transport/jsonrpc/ | active | 2026-05-22 |
| [[tool-protocol-operations]] | Tool protocol: tool.list/tool.call JSON-RPC methods with ToolHandler backed by ToolRegistry | active | 2026-05-22 |
| [[agent-directory-discovery]] | Agent directory discovery: discover_agents() from .md files, agent.list metadata, frontend agent selector | active | 2026-05-23 |
| [[agent-centric-ui]] | Agent-centric UI: agents tab first, conversation/sessions as sub-tabs, agent status cards, agent_id session filtering | active | 2026-05-23 |
| [[per-agent-conversation]] | Per-agent conversation state: HashMap keyed by agent_id, independent entries per agent, active_agent routing | active | 2026-05-23 |
| [[web-dev-environment-claudemd]] | CLAUDE.md and project skill web tooling update for Dioxus, Tailwind watch mode, cargo-watch, and startup troubleshooting | active | 2026-05-28 |
| [[remove-vol-agent-manager]] | Cleanup removing obsolete vol-agent-manager crate, legacy frontend, and manager-only deployment artifacts | active | 2026-05-29 |

## Analyses

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
