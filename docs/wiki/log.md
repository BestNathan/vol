# Change Log

## [2026-05-29] ingest | Remove vol-agent-manager and Legacy Frontend
- Created sources: [[remove-vol-agent-manager]]
- Updated entities: [[vol-llm-ui-crate]], [[vol-llm-agent-channel-crate]]
- Updated index: new source entry, updated date
- Cross-references added: 4
- Changes: Removed obsolete `vol-agent-manager` crate, manager-only Docker/Kubernetes artifacts, and legacy React `frontend/`; active web backend remains `vol-llm-agent-channel` JSON-RPC service via `make web-backend`.

## [2026-05-28] ingest | CLAUDE.md Web Development Environment
- Created sources: [[web-dev-environment-claudemd]]
- Updated entities: [[vol-llm-ui-crate]] (web development prerequisites, persistent Makefile CSS watcher, project web-dev skill, startup services, troubleshooting)
- Updated index: new source entry, vol-llm-ui summary/date, updated date
- Cross-references added: 7
- Changes: CLAUDE.md now documents web-only prerequisites: Dioxus CLI 0.6.x, cargo-watch, Node/npm, wasm32 target, vol-llm-ui npm dependencies, Tailwind --watch=always, and dx --platform web fallback for Dioxus 404. `make web-css` now runs persistent Tailwind watch mode, and `.claude/skills/vol-web-dev/SKILL.md` is tracked as the project-specific web startup/debug guide.

## [2026-05-23] ingest | Per-Agent Conversation State
- Created sources: [[per-agent-conversation]]
- Updated entities: [[vol-llm-ui-crate]] (per-agent ConversationState, source_count)
- Updated index: new source entry, updated date
- Cross-references added: 1
- Changes: ConversationState rewritten as HashMap<String, AgentConversation>; events route to active agent; agent switch restores per-agent entries; resume stores under correct agent key.

## [2026-05-23] ingest | Agent-Centric UI + Protocol
- Created sources: [[agent-centric-ui]]
- Updated entities: [[vol-llm-agent-channel-crate]] (session.list agent_id, agent status tracking)
- Updated index: new source entry, updated date
- Cross-references added: 1
- Changes: Tab bar reorganized (Agents first, no Conversation/Sessions tabs). Conversation/Sessions are sub-tabs inside Agents panel scoped to selected agent. Agent cards show status/current task. session.list accepts agent_id filter. agent.list returns status/current_input.

## [2026-05-23] ingest | Agent Directory Discovery
- Created sources: [[agent-directory-discovery]]
- Updated entities: [[vol-llm-agent-channel-crate]] (agent_defs, discover_agents, agent.list metadata)
- Updated index: new source entry, updated date
- Cross-references added: 1
- Changes: Created 3 agent definition files (general-purpose, explore, review); example uses discover_agents(); agent.list returns type/description/scope; frontend adds agent selector dropdown with target param.

## [2026-05-22] ingest | Tool Protocol Operations
- Created sources: [[tool-protocol-operations]]
- Updated entities: [[vol-llm-agent-channel-crate]] (timeline, source_count)
- Updated index: new source entry, updated date
- Cross-references added: 1
- Changes: Added ToolOperation/ToolPayload to protocol; created ToolHandler with tool.list/tool.call; frontend client and tools panel updated with system tool listing and direct invocation.

## [2026-05-22] ingest | JSON-RPC Transport Consolidation
- Created sources: [[jsonrpc-transport-consolidation]]
- Updated entities: [[vol-llm-agent-channel-crate]] (module structure, key facts, timeline)
- Updated index: new source entry, updated date
- Cross-references added: 1
- Changes: Moved jsonrpc/{server,connection,serde_helpers}.rs and gateway/jsonrpc_ws.rs (as codec.rs) into transport/jsonrpc/; deleted old jsonrpc/ and gateway/ directories; updated internal imports and test paths; no public API breakage.

## [2026-05-22] ingest | AgentInput Channel Unification
- Created sources: [[agentinput-channel-unification]]
- Updated concepts: [[agent-dispatcher]] (run_input instead of run_with_id), [[agentinput-multimodal-run]] (channel uses AgentInput directly)
- Updated entities: [[vol-llm-agent-channel-crate]] (new key facts, timeline entry)
- Updated index: new source entry, updated date
- Cross-references added: 3
- Changes: Unified AgentPayload::Submit, AgentRequest, and dispatcher to use AgentInput directly. Dropped redundant run_id/metadata fields. Switched dispatcher from run_with_id to run_input.

## [2026-05-21] ingest | AgentInput Multimodal Run Implementation
- Created sources: [[agentinput-multimodal-run-implementation]]
- Created concepts: [[agentinput-multimodal-run]]
- Updated entities: [[vol-llm-agent-crate]] (AgentInput/InputPart, run_input, run_id and metadata support), [[vol-llm-core-crate]] (multipart message content testability), [[vol-llm-provider-crate]] (Anthropic multipart text/image conversion), [[vol-llm-agent-channel-crate]] (legacy string and structured AgentInput compatibility), [[vol-llm-tool-crate]] (McpTool aligned with McpManager)
- Updated index: refreshed entity summaries, new concept and source entries
- Cross-references added: 14
- Changes: ReActAgent now supports structured multimodal AgentInput while preserving run(&str); first modalities are text and image URL/data URL; Anthropic provider emits native multipart blocks; agent-channel transports deserialize both old string input and new structured input

## [2026-05-13] ingest | McpManager Implementation — Connection Lifecycle Manager
- Created sources: [[mcp-manager-impl]]
- Created concepts: [[mcp-manager-lifecycle]]
- Updated concepts: [[mcp-client-integration]] (McpSession → McpManager, async list_all_tools, auto-reconnect)
- Updated entities: [[vol-llm-mcp-crate]] (added McpManager, ServerStatus, ServerState modules)
- Updated sources: [[react-agent-mcp-integration]] (added McpManager migration section)
- Updated index: vol-llm-mcp-crate summary, new concept/source entries
- Cross-references added: 6
- Changes: Replaced McpSession with McpManager across vol-llm-mcp, vol-llm-tool, vol-llm-agent crates; added connection state tracking (Connected/Disconnected/Connecting/Error), auto-reconnect with exponential backoff (1s-30s, 5 max retries), full MCP protocol (resources, prompts); 14 tests pass in vol-llm-mcp, 6 in vol-llm-tool, 142 in vol-llm-agent

## [2026-05-12] ingest | Full Tailwind CSS Migration — All 16 Components
- Created sources: [[tailwind-css-full-migration]]
- Updated concepts: [[tailwind-css-migration]] (marked complete, added infrastructure section, responsive breakpoints)
- Updated concepts: [[dioxus-web-pattern]] (GLOBAL_CSS deleted, Tailwind v4 active)
- Updated entities: [[vol-llm-ui-crate]] (timeline updated with full migration completion)
- Updated index: tailwind-css-migration status → complete, new source entry
- Cross-references added: 4
- Changes: All 16 web component files migrated from GLOBAL_CSS (~215 lines, ~100 classes) to Tailwind v4 utility classes; input.css with custom breakpoints/animations; rebuild-web.sh integrates @tailwindcss/cli; 0 old CSS class references remain; Rust wasm32 build passes; full rebuild produces dist with index.html, tailwind.css (59KB), wasm/; responsive breakpoints for sidebar and tab bar

## [2026-05-12] ingest | Conversation.rs Tailwind CSS Migration
- Created sources: [[conversation-tailwind-migration]]
- Created concepts: [[tailwind-css-migration]]
- Updated concepts: [[dioxus-web-pattern]]
- Updated entities: [[vol-llm-ui-crate]]
- Updated index: new entries for tailwind-css-migration and conversation-tailwind-migration
- Cross-references added: 4
- Changes: conversation.rs migrated from semantic CSS classes (conversation, conversation-empty, msg-*) to inline Tailwind utility classes; container uses flex-1 overflow-y-auto p-2.5; empty state uses flexbox centering; all 9 message types (user input, thinking, streaming, tool call, tool result, agent answer, run summary, error, checkpoint) use Tailwind classes with preserved color palette via arbitrary values; helper functions unchanged

## [2026-05-11] ingest | Task 6: Wire Sessions Tab into App Component
- Created sources: [[task-6-sessions-tab-wiring]]
- Created concepts: [[sessions-ui-pattern]]
- Updated entities: [[vol-llm-ui-crate]]
- Updated concepts: [[dioxus-web-pattern]]
- Updated index: new entries for sessions-ui-pattern and task-6-sessions-tab-wiring
- Cross-references added: 5
- Changes: Sessions tab wired into App component — SessionsState signal created and provided via context, SessionsPanel replaces SessionDialog in web UI, Sessions tab button added to TabBar, placeholder replaced with SessionsPanel in TabContent, 9 CSS classes added for sessions panel and checkpoint rendering, msg-checkpoint CSS added for EntryCheckpoint display in conversation view
- Created sources: [[docs-rs-mcp-example]]
- Created concepts: [[mcp-example-pattern]]
- Updated entities: [[vol-llm-agents-crate]]
- Updated concepts: [[mcp-client-integration]], [[agent-builder-pattern]]
- Updated sources: [[react-agent-mcp-integration]]
- Updated index: new entries for docs-rs-mcp-example and mcp-example-pattern
- Cross-references added: 6
- Changes: Runnable example added to vol-llm-agents demonstrating full MCP integration flow — temp dir with .mcp.json, AgentConfig builder with with_mcp_from_config(), MCP tool discovery inspection, agent execution with docs-rs search, result printing; compiles cleanly with cargo check

## [2026-05-11] ingest | Split Signal State — EventBus Architecture
- Created sources: [[split-signal-state]]
- Created concepts: [[event-bus-pattern]]
- Updated concepts: [[dioxus-signal-pattern]], [[dioxus-web-pattern]]
- Updated entities: [[vol-llm-ui-crate]]
- Updated index: new entries for event-bus-pattern and split-signal-state
- Cross-references added: 6
- Changes: Centralized Signal<UiState> replaced with EventBus + UiEventKind routing + per-component local signals; SubscriptionSet with Drop impl for auto-cleanup; shared GlobalState/ApprovalUiState signals via use_context_provider; AppState simplified to EventBus + JsonRpcClient + Signal<ActiveTab>; EventHandler changed from Fn+Send+Sync to Fn+'static; ConversationEntry gained PartialEq; 43 tests passing; web + TUI builds both green

## [2026-05-11] ingest | ReAct Agent MCP Integration — vol-llm-mcp Crate
- Created sources: [[react-agent-mcp-integration]]
- Created entities: [[vol-llm-mcp-crate]]
- Created concepts: [[mcp-client-integration]]
- Updated entities: [[vol-llm-agent-crate]], [[vol-llm-tool-crate]], [[vol-mcp-servers-crate]]
- Updated concepts: [[tool-registry]], [[agent-builder-pattern]]
- Updated index: new entries for vol-llm-mcp-crate, mcp-client-integration, react-agent-mcp-integration
- Cross-references added: 8
- Changes: New vol-llm-mcp crate (config parsing, McpSession, McpToolInfo); McpTool implements ExecutableTool with name format mcp__{server}_{tool}; ToolRegistry gains register_from_mcp(); AgentConfigBuilder gains with_mcp_from_config(); AgentConfig gains mcp_session field with disconnect in run() cleanup; 142+ tests passing; workspace compiles cleanly

## [2026-05-10] update | vol-mcp-servers Docker Packaging — Alpine Multi-Stage
- Created sources: [[vol-mcp-servers-dockerfile]]
- Created entity: [[vol-mcp-servers-crate]] (Docker section)
- Updated concepts: [[docs-rs-mcp-impl]]
- Changes: Multi-stage Alpine 3.21 Dockerfile — builder stage with Rust toolchain + rsproxy mirror (.cargo/config.toml), runtime stage ~30MB; apk uses mirrors.aliyun.com; ENV BIN_NAME + ENTRYPOINT shell pattern enables ARG-based binary selection; ACR registry target

## [2026-05-10] ingest | docs-rs MCP Server Implementation
- Created sources: [[docs-rs-mcp-impl]]
- Created entities: [[vol-mcp-servers-crate]]
- Created concepts: [[mcp-transport-pattern]], [[docs-rs-tools]], [[rmcp-sdk]]
- Updated entity: [[vol-llm-agent-channel-crate]]
- Cross-references added: 5+
- Changes: vol-mcp-servers crate created with docs-rs-mcp binary; 4 MCP tools (search_crates, readme, get_item, search_in_crate) ported from TypeScript reference using rmcp 1.6.0; stdio (default) and HTTP/SSE transports via --http flag; StreamableHttpService with LocalSessionManager for session mgmt; HTML parsing via scraper + html2md; both transports verified working

## [2026-05-10] ingest | Lazy-Loading Directory Tree
- Created sources: [[lazy-load-dir-tree]]
- Created concepts: [[workspace-tree-pattern]]
- Updated entities: [[vol-llm-ui-crate]]
- Updated concepts: [[dioxus-signal-pattern]], [[dioxus-web-pattern]]
- Updated index: new entries for workspace-tree-pattern and lazy-load-dir-tree
- Cross-references added: 5
- Changes: WorkspaceTree/WorkspaceEntry replaced with WorkspaceTreeNode tree; directories fetch children on-demand via JSON-RPC file.list on expand; every expand re-fetches fresh data; refresh button on each directory; TreeNode is a reactive Dioxus #[component] (not plain function); Signal::with_mut() for tree mutations; borrow checker pattern: return value from with_mut before making async callback; TUI rendering updated with flatten_tree_for_tui helper; 42 tests passing

## [2026-05-10] ingest | Task 5: FileContentView Component
- Created sources: [[task-5-file-content-view]]
- Created concepts: [[file-tab-pattern]]
- Updated concepts: [[dioxus-web-pattern]], [[dioxus-signal-pattern]]
- Updated entities: [[vol-llm-ui-crate]]
- Cross-references added: 5
- Changes: `FileContentView` component with file tab bar showing open files with icons, names, close buttons; content area displays loaded content (`<pre>`), error, or loading state; `render_tab` uses plain function (not `#[component]`) to avoid `PartialEq` derive on `OpenFileTab` props; `bump_version()` helper extracted; `file_icon` made `pub(crate)`; WASM build compiles with only pre-existing `ActiveTab::Tools` error (Task 6)

## [2026-05-09] ingest | JSON-RPC Transport Refactoring
- Created sources: [[jsonrpc-transport-refactoring]]
- Created concepts: [[jsonrpc-transport]]
- Updated concepts: [[jsonrpc-server-handler]] (marked deleted), [[connection-holder]], [[connection-trait]], [[agent-plugin-system]]
- Updated entities: [[vol-llm-agent-channel-crate]]
- Updated index: new entries, updated summaries
- Cross-references added: 6
- Changes: EventBridgePlugin deleted, JsonRpcHandler/JsonRpcContext replaced by JsonRpcConnection implementing Connection trait; JsonRpcServer with Vec<AgentRegistration> multi-agent support; agent.submit gains optional target param; 49 integration tests; wire format preserved

## [2026-05-09] ingest | Task 5: JSON-RPC Integration Tests
- Created sources: [[task-5-jsonrpc-integration-tests]]
- Updated entities: [[vol-llm-agent-channel-crate]]
- Updated concepts: [[json-rpc-websocket]], [[jsonrpc-server-handler]]
- Cross-references added: 3
- Changes: 44 integration tests in `tests/jsonrpc_integration.rs` covering all 22 AgentStreamEvent variant serializations, all 13 JSON-RPC request method parsings, event format structure verification, parse-and-respond roundtrip, and 6 error handling tests for malformed input

## [2026-05-08] ingest | Task 10: Final Verification
- Created sources: [[task-10-final-verification]]
- Cross-references added: 4
- Changes: All 10 tasks completed; 55 tests pass (39 vol-llm-ui + 16 vol-llm-agent-channel); vol-llm-ui compiles with all features (default/tui, web, both); vol-llm-agent-channel compiles with all targets; architecture: shared core (state/events/connections) + TUI (ratatui 0.30, 11 render functions, 12 input tests) + Web (Dioxus 0.6 WASM, 10 components) + JSON-RPC server (jsonrpsee 0.26, 9 methods); both LocalConnection (in-process) and RemoteConnection (JSON-RPC WS with auto-reconnect) implemented and verified

## [2026-05-08] ingest | Task 9: JSON-RPC Server
- Created sources: [[task-9-jsonrpc-server]]
- Created concepts: [[jsonrpc-server-handler]]
- Updated concepts: [[json-rpc-websocket]], [[connection-holder]], [[agent-dispatcher]], [[remote-agent-connection]]
- Updated entity: [[vol-llm-agent-channel-crate]]
- Cross-references added: 5
- Changes: jsonrpc module in vol-llm-agent-channel with JsonRpcHandler and JsonRpcContext; 9 JSON-RPC methods (agent.submit/cancel/approve, file.list/read, log.list/read, session.list/resume); JsonRpcContext wraps AgentDispatcher with working_dir and store_dir paths; example binary uses jsonrpsee 0.26 ServerBuilder with RpcModule::from_arc; list and read operations use std::fs; log and session listing scan store_dir/logs/*.jsonl and store_dir/sessions/*.json; stub implementations for log.read and session.resume return empty results; compiles with cargo check -p vol-llm-agent-channel --all-targets; all 16 existing tests pass

## [2026-05-08] ingest | Task 8: Dioxus Web Frontend
- Created sources: [[task-8-dioxus-web-frontend]]
- Created concepts: [[dioxus-signal-pattern]], [[dioxus-web-pattern]]
- Updated concepts: [[human-in-the-loop]], [[vol-llm-ui-crate]]
- Cross-references added: 8
- Changes: Dioxus 0.6 web frontend with WASM compilation; Signal<UiState> via use_context_provider with write_silent() for interior mutability; 10 components (App, StatusBar, ConversationView, ToolsPanel, InputArea, WorkspacePanel, SkillsPanel, LogViewer, SessionDialog, ApprovalDialog); global CSS dark theme; feature gated with #[cfg(feature = "web")]; compiles with cargo check -p vol-llm-ui --features web --bin vol-llm-ui-web; TabBar/TabContent routing pattern; modal dialogs rendered at root level

## [2026-05-08] ingest | TUI Frontend (ratatui)
- Created sources: [[tui-frontend-ratatui]]
- Created concepts: [[ratatui-tui-pattern]], [[ui-event-loop-pattern]]
- Updated entities: [[vol-llm-ui-crate]]
- Updated concepts: [[human-in-the-loop]], [[connection-trait]], [[agent-event-stream]], [[remote-agent-connection]]
- Cross-references added: 6
- Changes: 9 render functions migrated from vol-llm-tui/ui/ to UiState using ratatui 0.30; crossterm EventStream + tokio::select! event loop at 30fps; approval key handling (A/R/S); session dialog; workspace tree, log viewer, skills panel; futures + uuid added as optional tui deps; LocalConnection::clone_for_run() made public; tui modules exported via lib.rs behind #[cfg(feature = "tui")]

## [2026-05-08] ingest | RemoteConnection for vol-llm-ui
- Created sources: [[remote-connection-impl]]
- Created entities: [[vol-llm-ui-crate]]
- Created concepts: [[json-rpc-websocket]], [[remote-agent-connection]]
- Updated concepts: [[connection-holder]], [[connection-trait]]
- Cross-references added: 4
- Changes: RemoteConnection implements AgentConnection and FileOperations via JSON-RPC 2.0 over WebSocket (jsonrpsee 0.26); uses ObjectParams for named parameters, ClientT trait for request method; auto-reconnect with exponential backoff (max 5 retries, 1s-30s); methods: agent.submit, agent.approve, agent.cancel, file.list, file.read, log.list, session.list; 3 unit tests all passing

## [2026-05-07] ingest | Agent Channel WS + HTTP Examples
- Created sources: [[agent-channel-examples]]
- Created concepts: [[agent-router]], [[connection-holder-clone-limitation]]
- Updated entity: [[vol-llm-agent-channel-crate]]
- Updated concepts: [[connection-holder]], [[agent-dispatcher]], [[http-transport]], [[agent-plugin-system]]
- Cross-references added: 5
- Changes: Added single_agent.rs (dual WS+HTTP transport on port 3000) and multi_agent.rs (agent router with 3 agents on port 3001); documented ConnectionHolder Clone limitation; code quality review completed

## [2026-05-06] ingest | OTel 0.29 Migration and Log Initialization in vol-monitor
- Created sources: [[otel-029-log-init]]
- Updated concepts: [[otel-log-routing]], [[agent-observability]]
- Cross-references added: 2
- Changes: tracing_setup.rs migrated from OTel 0.21 to 0.29 APIs — Resource::builder pattern, SdkTracerProvider flattened builder, SpanExporter/LogExporter builder, removed runtime param from batch exporter, global::shutdown replaced with direct provider.shutdown(); added init_otel_logs() function with OpenTelemetryTracingBridge layer; opentelemetry-appender-tracing dependency added

## [2026-05-06] ingest | LokiPlugin OTel Migration Tasks 3+4
- Created sources: [[loki-plugin-otel-migration-tasks-3-4]]
- Created concepts: [[otel-log-routing]]
- Updated concepts: [[agent-observability]], [[run-context]], [[built-in-plugins]]
- Cross-references added: 5
- Changes: LokiPlugin stateless, uses tracing::info! instead of HTTP POST; RunContext gains model field; 12+ test call sites updated; tempfile dev-dependency added

## [2026-05-06] ingest | Clarifying Requirements Subagent Review
- Created sources: [[clarifying-requirements-subagent-review]]
- Created concepts: [[subagent-review-pattern]], [[clarifying-requirements-workflow]]
- Updated concepts: [[skill-system]], [[human-in-the-loop]]
- Cross-references added: 4

## [2026-05-05] update | HTTP Transport improvements and tests
- Updated concepts: [[http-transport]], [[connection-trait]], [[connection-holder]]
- Updated entity: [[vol-llm-agent-channel-crate]]
- Changes: SSE stream termination (drop event_tx vs 100ms sleep), holder detach on stream end, 409 for concurrent SSE requests, simplified HttpEventConnection, 5 tests added

## [2026-05-05] ingest | HTTP Transport Implementation
- Created sources: [[http-transport-impl]]
- Created concepts: [[http-transport]], [[connection-trait]], [[connection-holder]], [[agent-dispatcher]]
- Created entity: [[vol-llm-agent-channel-crate]]
- Cross-references added: 6+

## [2026-05-04] ingest | Agent Component Documentation (tools, skills, session, context)
- Created sources: [[agent-tool-design]], [[skills-as-react-native]], [[session-ssot-redesign]]
- Created concepts: [[skill-system]], [[session-as-ssot]], [[run-context]], [[context-builder]], [[session-contributor]], [[session-compression]], [[plugin-context-migration]], [[context-error]], [[tool-trait]], [[tool-context]]
- Created entity: [[vol-session]]
- Updated concepts: [[tool-registry]], [[agent-plugin-system]], [[react-pattern]], [[agent-builder-pattern]], [[agent-event-stream]], [[vol-llm-agent-crate]]
- Cross-references added: 15+

## [2026-05-04] ingest | ReAct Agent Documentation
- Created: [[react-agent-docs]]
- Created concepts: [[react-pattern]], [[agent-plugin-system]], [[plugin-actions]], [[built-in-plugins]], [[agent-event-stream]], [[agent-builder-pattern]], [[tool-registry]], [[agent-observability]], [[semantic-caching]], [[human-in-the-loop]], [[retry-with-backoff]], [[rate-limiting]]
- Created entities: [[vol-llm-agent-crate]], [[vol-llm-agents-crate]], [[vol-llm-core-crate]], [[vol-llm-tool-crate]], [[vol-llm-provider-crate]], [[tdengine]], [[dashscope]]
- Cross-references added: 12
