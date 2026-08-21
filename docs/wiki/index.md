# Wiki Index

Last updated: 2026-08-21 (README restructure: pure agent six-section layout; pipeline mentions removed)

## Entities

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[vol-llm-runtime-crate]] | AgentRuntime owner of shared agent resources, runtime task/session store config types, and data-plane capability source; registers the CLI-style `fs` and `task` tools in `AgentRuntimeBuilder::build()`, plus the builtin `agent` sub-agent dispatch tool, shared `agent_loader`, and AgentInjector wiring | active | 2026-08-20 |
| [[vol-llm-agent-tool-crate]] | High-level composition crate: AgentTool (dispatch sub-agents by `AgentDef.id` with depth guard + name-keyed session persistence) and AgentInjector (context contributor listing available agents); coverage 89.32% | active | 2026-08-20 |
| [[vol-llm-fs-crate]] | Unified CLI-style `fs` tool (read/write/edit/grep/glob/scheme, `--json`) delegating to the five builtin file-op tools; modeled on the `task` CLI | active | 2026-08-16 |
| [[vol-llm-task-crate]] | Task models and persistence stores, including SeaORM database store for SQLite and Postgres with compiled migrations | active | 2026-06-09 |
| [[vol-agent-server-crate]] | Standalone server crate that composes DataPlaneServerCore/ControlPlaneServerCore routes and is deployed by the self-contained ArgoCD GitOps tree as `agent-server`; supports remote control-plane registration with heartbeat/reconnect | active | 2026-06-17 |
| [[vol-llm-ui-crate]] | Shared UI state model. Web (Dioxus) DEPRECATED 2026-08 — React frontend/ is the active web UI. TUI + state maintained. | deprecated | 2026-08-06 |
| [[vol-llm-sandbox-crate]] | Sandbox abstraction (Local/Tmp/SSH/Firecracker/Wasm), SandboxRegistry with pure-config loading, TmpSandbox with bind_metadata lifecycle; LocalSandbox timeout kill reworked to positive-pid kills (group kills kill the caller tree in sandboxes) | active | 2026-08-19 |
| [[vol-llm-agent-crate]] | ReAct Agent orchestration crate with structured `AgentInput` multimodal run API and `[image]` display-text markers; AgentTool moved out to vol-llm-agent-tool, AgentLoader gained `get_by_id` | active | 2026-08-20 |
| [[vol-llm-agents-crate]] | High-level agent implementations (advice, coding, ppt, qa) with runnable MCP examples | active | 2026-05-11 |
| [[vol-llm-core-crate]] | Core LLM interaction abstractions, including provider-neutral multipart message content and `[image]` display markers; coverage gate PASS 95.62% | stable | 2026-08-17 |
| [[vol-llm-tool-crate]] | Tool definition and registry framework with MCP tool proxying through McpManager; web module with ProxyConfig (three-tier resolution) and RetryConfig (exponential backoff) | active | 2026-08-12 |
| [[vol-llm-provider-crate]] | Anthropic and OpenAI provider implementations; four 2026-08-17 bugfixes (raw tool-call args, request.system forwarding, symmetric Secret JSON, streamed ToolCallComplete); coverage gate PASS 95.41% (120 tests) | stable | 2026-08-17 |
| [[vol-llm-context-crate]] | Pluggable prompt construction: ContextBuilder, ContextContributor, builtin simple/file/user_input contributors, token-budgeted compression, snapshot APIs; coverage gate PASS 88.94% regions / 90.08% lines | active | 2026-08-17 |
| [[vol-session]] | Session message store and entry persistence, including file and SeaORM database-backed session managers; images kept through compression | active | 2026-08-17 |
| [[vol-llm-agent-protocol-crate]] | Protocol, JSON-RPC transport, connection, handler, registry, and generic service abstraction layer | active | 2026-06-10 |
| [[tdengine]] | Time-series database used for market data storage | active | 2026-05-04 |
| [[dashscope]] | DashScope API endpoint for Claude model access | active | 2026-05-04 |
| [[vol-mcp-servers-crate]] | MCP server collection with multi-transport support; `docs-rs-mcp` is GitOps-managed and built by the MCP image workflow | active | 2026-06-16 |
| [[vol-repository]] | Rust workspace: agent-only six-section README, just recipes as command entry point, React `frontend/` (active web UI; vol-llm-ui deprecated), ArgoCD GitOps primary with `k8s/` legacy deprecated | active | 2026-08-21 |
| [[vol-observability-crate]] | Consolidated observability library (LoggingPlugin, MetricsPlugin, /metrics endpoint, OTel init); agent file logs rotate hourly into `logs/` instead of the process CWD (2026-08-19) | active | 2026-08-19 |
| [[vol-llm-mcp-crate]] | MCP Client protocol layer for ReAct Agent — config parsing, McpManager lifecycle, tool/resource/prompt discovery | active | 2026-05-13 |
| [[playwright-mcp-service]] | Standalone in-cluster MCP service exposing Playwright browser automation (24 browser_* tools) on port 8931, referenced via http URL in mcp-config; hardened (ro rootfs, non-root, dropped caps) | active | 2026-08-13 |

## Concepts

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[sandbox-lifecycle]] | Sandbox lifecycle: define→construct→register→acquire→bind→start→use→cleanup. Pure registry design with TmpSandbox default, bind_metadata for sub_dir | active | 2026-08-11 |
| [[test-tiers]] | Three-tier test split (unit `--lib` / integration `-E 'kind(test)'` / e2e `--ignored`) mapped to scenarios: pre-push runs changed-crate unit tests only, CI runs unit+integration+coverage; e2e tier landed — `#[ignore = "e2e: ..."]` marker convention, in-test env guards (clean skips), manual e2e.yml workflow + frontend Playwright in the PR gate; broken/ignored tests fixed, never `#[ignore]`d | active | 2026-08-19 |
| [[cli-style-tool-pattern]] | Single `ExecutableTool` taking a CLI command string (`tool <subcommand> --flag value`): tokenizer + clap parser → typed command enum → delegation to underlying tools; `task` CLI and `fs` tool are the two implementations | active | 2026-08-16 |
| [[agenttool-subagent-dispatch]] | Builtin `agent` tool dispatch semantics: by unique `AgentDef.id`, depth guard via caller `tool_config.agent.max_depth` (default 1 = one layer), parent/depth recorded on AgentDef and carried through ToolContext, sub-agent session persisted by name | active | 2026-08-20 |
| [[arc-new-cyclic-registration]] | Rust pattern: `Arc::new_cyclic` for a tool holding a `Weak` to the registry it is being registered into — `get_mut` (requires weak_count==1) and `try_unwrap` (dangles prior Weaks) are both wrong | active | 2026-08-20 |
| [[argocd-app-of-apps-gitops]] | Self-contained ArgoCD App-of-Apps deployment pattern split into `runtime-config` (namespace + shared agents/providers/skills ConfigMaps) and `workloads` (application deployments), with `agent-server` mounting `/app/.agents` and CI-built MCP images updating GitOps manifests | active | 2026-06-16 |
| [[agent-server-control-data-plane]] | Single server crate with DataPlaneServerCore/ControlPlaneServerCore, channel-owned JSON-RPC protocol, route composition, data-plane snapshot facade, command/run semantics, control-plane router MVP, role-mode verification tests, dependency boundary checks, and remote data-plane registration with heartbeat/reconnect | active | 2026-06-17 |
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
| [[context-builder]] | Pluggable prompt construction from contributors, with multipart-aware `estimate_tokens` (per-image budget 1600) | active | 2026-08-17 |
| [[session-contributor]] | Session history as context contributor, with image-aware compression (summary `[image]` markers, sampling exemption) | active | 2026-08-17 |
| [[session-compression]] | Two-layer session message compression that keeps images (`[image]` summary markers, sampling exemption) | active | 2026-08-17 |
| [[plugin-context-migration]] | Migration from PluginContext to RunContext | active | 2026-05-04 |
| [[context-error]] | Error type for context building failures | stable | 2026-05-04 |
| [[agent-observability]] | JSONL logging + OTel structured log routing | stable | 2026-05-06 |
| [[otel-log-routing]] | OTel Collector log routing via tracing::info! macros | active | 2026-05-06 |
| [[semantic-caching]] | Response caching with semantic similarity matching | stable | 2026-05-04 |
| [[human-in-the-loop]] | Human approval workflow for tool execution | stable | 2026-05-04 |
| [[retry-with-backoff]] | Two implementations: agent plugin retry and web-tool-level `retry_async()` with transient-error detection | active | 2026-08-12 |
| [[proxy-config-resolution]] | Three-tier proxy resolution chain: tool parameter > agent config > environment variable (`HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`) | active | 2026-08-12 |
| [[rate-limiting]] | Concurrency control using semaphore-based rate limiting | stable | 2026-05-04 |
| [[pull-based-metrics]] | Prometheus pull metrics via shared registry + /metrics endpoint | active | 2026-07-24 |
| [[http-transport]] | Historical HTTP transport with blocking and SSE streaming modes; deleted from active channel API after Task 4 | stale | 2026-06-10 |
| [[connection-trait]] | Connection trait abstracting transport protocols | active | 2026-05-05 |
| [[connection-holder]] | ConnectionHolder plugin for forwarding agent events | active | 2026-05-05 |
| [[agent-dispatcher]] | FIFO request queueing for single-agent execution | active | 2026-05-05 |
| [[subagent-review-pattern]] | Independent subagent review of documents before user gate | active | 2026-05-06 |
| [[agent-router]] | Node-local multi-agent routing with per-agent dispatchers; distributed routing sits above it | active | 2026-06-10 |
| [[connection-holder-clone-limitation]] | ConnectionHolder cannot be both plugin and transport reference | active | 2026-05-07 |
| [[clarifying-requirements-workflow]] | Structured dialogue for turning vague requests into requirements | active | 2026-05-06 |
| [[mcp-transport-pattern]] | Multi-transport startup pattern for MCP servers (stdio, HTTP/SSE); in-cluster pattern = standalone Deployment + ClusterIP Service + http URL (docs-rs-mcp / cli-tools-mcp / playwright-mcp); stdio requires the runtime image to contain the command | active | 2026-08-13 |
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
| [[agentinput-multimodal-run]] | Structured ReActAgent run input envelope for text/image parts, run_id, metadata, and protocol compatibility; `[image]` display markers | active | 2026-08-17 |
| [[streaming-session]] | StreamProtocol/StreamingSession SSE parsing; OpenAI stream-end ToolCallComplete flush via ContentBlockStop (no per-block stop marker); known limitation — multi-tool-call streams complete only the last-started call | active | 2026-08-17 |

## Sources

| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[readme-restructure]] | README restructured to pure agent six-section layout (agent system / architecture / project structure / install & deploy / AI workflow / tools & commands); volatility pipeline not mentioned per follow-up decision; stale content fixed (make→just, vol-llm-ui deprecated, crate table updated, ArgoCD primary); lean overview style linking to wiki concepts | active | 2026-08-21 |
| [[provider-bugfixes]] | Four vol-llm-provider production bugfixes (TDD, one commit each): raw string tool-call args, request.system as first system message, symmetric Secret JSON round-trip, streamed ToolCallComplete via ContentBlockStop flush; gate re-verified 95.41%, 120 tests / 0 failed | active | 2026-08-17 |
| [[agenttool-builtin-impl]] | AgentTool builtin implementation: new vol-llm-agent-tool crate, id-based dispatch with depth guard, name-keyed session persistence, AgentInjector, runtime wiring; subagent-driven execution with Arc::new_cyclic fix round | active | 2026-08-20 |
| [[test-tiering-hooks]] | Three-tier test split: pre-commit fmt/lint/type, pre-push changed-crate unit tests only (coverage removed — was the slow part), CI unit+integration+coverage; justfile umbrella recipes deleted, hooks rewritten as thin just-calling shells, `test-integration` fixed to `-E 'kind(test)'` filter, 6 superseded check scripts deleted; e2e dedicated workflow deferred | active | 2026-08-18 |
| [[test-tiering-e2e-completion]] | E2E tier landed: `e2e:` ignore-marker convention + env guards in all e2e tests, manual e2e.yml + Playwright in PR gate, `test-e2e-crate`/`fe-e2e` recipes; fixed wasmtime memory-export test, brittle mock (MockLlmClient event queue), runtime inline ignore, 2 bash-timeout tests; LocalSandbox kill reworked (positive pids only — group kills kill the caller tree in sandboxes) | active | 2026-08-19 |
| [[frontend-test-tiering]] | Frontend vitest split into unit (node) + integration (jsdom + testing-library) projects; 4 new component tests (InputArea/TabBar/StatusBar/CapabilityBar) render real components with jotai store + mocked panel client; `fe-test-unit`/`fe-test-integration` recipes, CI runs tiers as separate steps; Playwright e2e unchanged (standalone-package proposal dropped) | active | 2026-08-19 |
| [[ci-workflow-restructure]] | CI workflows restructured: quality.yml drops all e2e (Playwright → e2e.yml only), unit+integration are the PR gate while coverage jobs are report-only (artifact upload, no threshold), and every workflow step calls a `just` recipe (`test-e2e-ci`, `cover-ci`, `fe-install`, `fe-pw-install`, `boundaries`) with script logic in scripts/ci-coverage-report.sh | active | 2026-08-19 |
| [[coverage-gate-work]] | Test-only coverage raises to ≥80%: vol-llm-context 88.94% regions / 90.08% lines, vol-llm-core 95.62%, vol-llm-provider 85.79% pre-bugfix; the provider suite surfaced the four production bugs | active | 2026-08-17 |
| [[fs-cli-tool]] | vol-llm-fs crate implementation: CLI-style `fs` tool (read/write/edit/grep/glob/scheme, `--json` envelope) over the five builtin file-op tools; registered from AgentRuntimeBuilder::build() next to the task tool; 89.81% line coverage | active | 2026-08-16 |
| [[multimodal-image-input]] | Multimodal image input feature: `[image]` display markers, per-image token budget (1600), images kept through session compression, OpenAI vision conversion, frontend attach/paste/render UI, WS frame-size verification (no explicit limit; defaults 64MiB/16MiB), live-stack e2e verification | active | 2026-08-17 |
| [[frontend-image-session-lightbox]] | Frontend image UX follow-ups: session detail overlay renders image parts (was text-only), shared ImageGallery lightbox (click-to-enlarge, prev/next) in ConversationView + SessionDetailOverlay, Attach trigger moved from InputArea bottom row to CapabilityBar next to ✎ via shared imageAttachmentsAtom + useImageAttachments hook; 173 frontend tests | active | 2026-08-19 |
| [[web-tools-proxy-retry]] | Three-tier proxy configuration (tool param > agent config > env var) and exponential-backoff retry for web_fetch/web_search tools | active | 2026-08-12 |
| [[playwright-mcp-k8s-deployment]] | Playwright MCP replaced stdio/npx (unrunnable in Rust-only agent-server image) with standalone Deployment + ClusterIP Service + http URL in mcp-config; in-field fixes runAsUser 1000 and --allowed-hosts *; verified in-cluster; egress test pending | active | 2026-08-13 |
| [[observability-pull-metrics-refactor]] | Consolidated observability crate, Prometheus pull /metrics, LLMCall event emission, run-level metrics, MetricsPlugin concurrency fix | active | 2026-07-24 |
| [[otel-agent-log-dir-fix]] | Agent log file location fix: rolling appender writes `logs/agent.*.log` instead of the process CWD (was littering the repo root); `build_agent_file_appender()` extracted, behavior test writes a marker and asserts logs/ placement with no CWD leak; 58/58 unit tests, coverage 88.24% | active | 2026-08-19 |
| [[argocd-gitops-deployment]] | Self-contained ArgoCD GitOps implementation: App-of-Apps split into runtime-config/workloads, shared .agents ConfigMaps mounted at /app/.agents, agent-provider-secrets, vol-agent-system manifests, MCP Dockerfile, and MCP image workflow | active | 2026-06-16 |
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
| [[data-plane-registration-sandbox-tolerance]] | Sandbox fault-tolerant loading and remote data-plane WebSocket registration with heartbeat/reconnect | active | 2026-06-17 |
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
