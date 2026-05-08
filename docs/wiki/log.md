# Change Log

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
