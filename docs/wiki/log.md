# Change Log

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
