# Change Log

## [2026-06-16] ingest | ArgoCD GitOps Deployment
- Created sources: [[argocd-gitops-deployment]]
- Created concepts: [[argocd-app-of-apps-gitops]]
- Updated entities: [[vol-agent-server-crate]] (agent-server GitOps workload), [[vol-mcp-servers-crate]] (docs-rs-mcp GitOps workload and MCP image workflow), [[vol-repository]] (`deploy/argocd/` GitOps tree)
- Updated index: new source/concept entries and refreshed entity summaries
- Cross-references added: 18
- Changes: Documented the self-contained `deploy/argocd/` App-of-Apps implementation for `agent-server` and `docs-rs-mcp`, the `vol-agent-system` namespace, ACR pull secret usage, `dockers/vol-mcp-servers.Dockerfile`, `build-mcp-images` workflow, and validation results including the Docker Hub timeout caveat.

## [2026-06-11] refactor | Rename crate: vol-llm-agent-channel → vol-llm-agent-protocol
- Renamed crate directory, Cargo.toml, all workspace dependency references, all Rust source imports, scripts, Makefile, CLAUDE.md, wiki entity page

## [2026-06-10] ingest | Control Plane Behavior Completion Plan
- Created sources: [[control-plane-behavior-completion-plan]]
- Updated concepts: [[agent-server-control-data-plane]] (follow-up plan for notification decode, endpoint roles, client handlers, data-plane command handling, capability revision sync, run status, and combined-mode registration)
- Updated index: new follow-up plan source entry
- Cross-references added: 8
- Changes: Captured the follow-up implementation plan after final review found behavior gaps in the initial control/data-plane implementation. Feishu/Lark upload: https://my.feishu.cn/docx/JRjcd9jnkoKxVyxoQ7zc1aHenue

## [2026-06-10] ingest | Agent Server Boundary and Role-Mode Verification
- Created sources: [[agent-server-boundary-mode-verification]]
- Updated entities: [[vol-agent-server-crate]] (Task 10 integration tests and dependency boundary script, source_count 15->16)
- Updated concepts: [[agent-server-control-data-plane]] (Task 10 verification status and boundary invariants, source_count 11->12)
- Updated index: new Task 10 source entry and refreshed server/control-data summaries
- Cross-references added: 10
- Changes: Documented boundary and mode verification for the control/data-plane split: executable cargo-tree guard script, `/ws` ownership integration tests for standalone and control-plane modes, disabled-role TOML validation, and verification commands.

## [2026-06-10] ingest | Agent Server Control Router MVP
- Created sources: [[agent-server-control-router-mvp]]
- Updated entities: [[vol-agent-server-crate]] (`ControlRouter<'a>`, `route_agent`, source_count 14->15)
- Updated concepts: [[agent-server-control-data-plane]] (Task 9 implementation status and routing semantics, source_count 10->11)
- Updated index: new Task 9 source entry and refreshed server/control-data summaries
- Cross-references added: 9
- Changes: Documented Task 9 control-plane routing MVP: online-node filtering through `NodeRegistry`, capability snapshot iteration through `CapabilityIndex`, target matching by `agent_id` or `name`, untargeted first-agent routing, and `capability_not_found` on miss.

## [2026-06-10] ingest | Agent Server Data-Plane Snapshot/Command Skeletons
- Created sources: [[agent-server-data-plane-snapshot-command]]
- Updated entities: [[vol-agent-server-crate]] (`RuntimeCapabilitySource`, `StaticCapabilitySource`, `accept_control_command`, source_count 13->14)
- Updated concepts: [[agent-server-control-data-plane]] (Task 8 implementation status and data-plane reporting primitives, source_count 9->10)
- Updated index: new Task 8 source entry and refreshed server/control-data summaries
- Cross-references added: 9
- Changes: Documented Task 8 data-plane primitives: snapshot/load facade, static empty capability source, fake-source test, accepted `CommandAck` skeleton with synthetic `run_{command_id}` for `SubmitAgent`, and verification commands.

## [2026-06-10] ingest | Agent Server Health Route Collision Validation
- Created sources: [[agent-server-health-route-collision-validation]]
- Updated entities: [[vol-agent-server-crate]] (`ServerConfig::validate` rejects active WebSocket paths equal to `/health`, source_count 12->13)
- Updated concepts: [[agent-server-control-data-plane]] (route edge case for `/health` collision validation, source_count 8->9)
- Updated index: new source entry plus refreshed server/control-data summaries
- Cross-references added: 8
- Changes: Documented the Task 7 quality fix preventing Axum duplicate-route startup panics by rejecting `/health` collisions for enabled control-plane client/node paths and data-plane-only client paths; noted regression test and verification commands.

## [2026-06-10] ingest | Agent Server Role Route Composition
- Created sources: [[agent-server-role-route-composition]]
- Updated entities: [[vol-agent-server-crate]] (`ws_owner`, role-specific `app::run`, configured control/data WebSocket mounting, main startup delegation, source_count 11->12)
- Updated concepts: [[agent-server-control-data-plane]] (Task 7 implementation status and route composition semantics, source_count 7->8)
- Updated index: new Task 7 source entry and refreshed server/control-data summaries
- Cross-references added: 10
- Changes: Documented Task 7 role composition for `vol-agent-server`: pure `/ws` ownership tests, control-plane priority for `/ws`, data-plane standalone fallback, configured `/control/v1/ws` node path, path expansion, data-plane agent discovery, and startup delegation to `app::run`.

## [2026-06-10] ingest | Agent Server Control-Plane Core and Handlers
- Created sources: [[agent-server-control-plane-core-handlers]]
- Updated entities: [[vol-agent-server-crate]] (`ControlPlaneServerCore`, control handlers, source_count 10->11)
- Updated concepts: [[agent-server-control-data-plane]] (Task 6 implementation status, source_count 6->7)
- Updated index: new Task 6 source entry and updated last-modified marker
- Cross-references added: 9
- Changes: Documented the Task 6 control-plane core and handler implementation: TDD `control_register_creates_node`, `control.register` RegisterAck behavior, heartbeat/snapshot/event handling, node list/get, capability list node filtering, and `JsonRpcMessageService` serve loop verification.

## [2026-06-10] ingest | Task 4 Quality Issues Cleanup
- Created sources: [[task-4-quality-issues-cleanup]]
- Updated concepts: [[jsonrpc-transport]] (generic `JsonRpcServer<S>`/`JsonRpcMessageService` path ownership and current `vol_llm_agent_channel::transport::jsonrpc::*` module path)
- Updated entities: [[vol-llm-agent-channel-crate]] (dependency scope cleanup and moved-router/dispatcher comment cleanup), [[vol-agent-server-crate]] (active backend ownership and `config.control_plane.client_ws_path` default `/ws` startup path)
- Updated sources: [[remove-vol-agent-manager]] (active backend claim points to `vol-agent-server` instead of deleted channel example)
- Updated index: new source entry and refreshed server/JSON-RPC summaries
- Cross-references added: 7
- Changes: Documented the follow-up Task 4 quality cleanup: removed unused `uuid`/`tempfile`, moved `tokio-tungstenite` and `vol-llm-core` to test-only dev-dependencies, removed stale active registration-list/deleted-example documentation claims, and verified checks/tests/clippy/fmt.

## [2026-06-10] update | Task 4 Code Quality Cleanup
- Updated concepts: [[http-transport]] marked historical/deleted from active channel API
- Updated sources: [[agent-channel-examples]] marked historical/deleted
- Updated entities: [[vol-llm-agent-channel-crate]] current transport/API notes, dependency cleanup, and configured JSON-RPC path ownership
- Updated index: HTTP transport and examples statuses set to stale
- Changes: Cleaned stale Task 4 wiki references after HTTP transport/examples deletion and channel/data-plane boundary cleanup.

## [2026-06-10] ingest | Agent Server Data-Plane Core Move
- Created sources: [[agent-server-data-plane-core-move]]
- Updated entities: [[vol-agent-server-crate]] (`DataPlaneServerCore`, data-plane module tree, configured standalone WebSocket mounting with `/ws` default, source_count 8->9), [[vol-llm-agent-channel-crate]] (protocol/connection/service/generic transport boundary, concrete module removal, source_count 18->19)
- Updated concepts: [[agent-server-control-data-plane]] (source_count 5->6, Task 4 implementation status)
- Updated index: new source entry and refreshed server summary
- Cross-references added: 18
- Changes: Documented Task 4 migration of concrete data-plane execution behavior into `vol-agent-server::data_plane`, with channel reduced to protocol/transport abstractions and verification passing for channel/server checks, tests, and formatting.

## [2026-06-10] ingest | Agent Server Role Config and Route Skeleton
- Created sources: [[agent-server-role-config-route-skeleton]]
- Updated sources: [[agent-server-control-data-plane-implementation-plan]] (Task 3 completion status and verification commands)
- Updated entities: [[vol-agent-server-crate]] (`ServerRoles`, `[control_plane]`, `[data_plane]`, `/health` route skeleton, source_count 7->8)
- Updated concepts: [[agent-server-control-data-plane]] (source_count 4->5, Task 3 role-config skeleton status)
- Updated index: new source entry for agent-server-role-config-route-skeleton
- Cross-references added: 10
- Changes: Documented Task 3 implementation of role-aware config parsing/validation and minimal Axum app/routes/health skeleton in `vol-agent-server`; focused tests, crate check, and formatting passed.

## [2026-06-10] ingest | ControlPayload Flat JSON-RPC Encoding Fix
- Created sources: [[control-payload-flat-jsonrpc-encoding-fix]]
- Updated entities: [[vol-llm-agent-channel-crate]] (key fact for `ControlPayload` flat encoding, timeline entry, source_count 17->18)
- Updated concepts: [[agent-server-control-data-plane]] (source_count 3->4, new source link, flat JSON-RPC payload rule)
- Updated index: new source entry for control-payload-flat-jsonrpc-encoding-fix
- Cross-references added: 5
- Changes: Documented the Task 2 code-quality fix removing internal serde tagging from `ControlPayload` so `Payload::data_json()` produces flat `control.*` JSON-RPC params/results. Two regression tests (`encode_control_register_command_uses_flat_params`, `encode_control_register_ack_result_uses_flat_result`) verify absence of `type`/`data` wrappers. `cargo test -p vol-llm-agent-channel encode_control` (2/2), `decode_control_register` (1/1), `decode_control_heartbeat_notification` (1/1) all pass. `cargo fmt --check` passes.

## [2026-06-10] ingest | Agent Server Control/Data Plane Task 1 Implementation
- Updated sources: [[agent-server-control-data-plane-implementation-plan]] (Task 1 completion status and verification commands)
- Updated entities: [[vol-llm-agent-channel-crate]] (`JsonRpcMessageService`, generic `JsonRpcServer<S>`, explicit route path, `AgentServerCore::serve_dyn` bridge), [[vol-agent-server-crate]] (startup now passes `/ws` explicitly)
- Updated index: no new entries; existing summaries already cover generic service abstraction
- Cross-references added: 6
- Changes: Documented the completed Task 1 implementation that decouples JSON-RPC transport from concrete `AgentServerCore` while preserving current data-plane behavior and tests.

## [2026-06-10] ingest | Agent Server Control/Data Plane Implementation Plan
- Created sources: [[agent-server-control-data-plane-implementation-plan]]
- Updated concepts: [[agent-server-control-data-plane]] (staged implementation sequence for channel service abstraction, control protocol, data-plane core migration, control-plane core, route composition, and boundary tests)
- Updated entities: [[vol-llm-agent-channel-crate]] (implementation starts with `JsonRpcMessageService` and `control.*` protocol), [[vol-agent-server-crate]] (implementation owns moved data-plane core and new control-plane core), [[vol-llm-runtime-crate]] (source count for runtime capability source role)
- Updated index: new implementation-plan source entry
- Cross-references added: 17
- Changes: Captured the implementation plan for the final agent-server control/data-plane architecture. Feishu/Lark upload updated to revision 18: https://my.feishu.cn/docx/TnKWd2VUeoKHnjxX8FgcIKzEnQ5

## [2026-06-10] ingest | Agent Server Control/Data Plane Addendum
- Created sources: [[agent-server-control-data-plane-addendum]]
- Updated concepts: [[agent-server-control-data-plane]] (endpoint role allowlists, command/run semantics, capability revision consistency, node record/session separation, combined-mode lifecycle, runtime capability facade, subscriptions, error code ownership, migration constraints, and boundary tests)
- Updated entities: [[vol-agent-server-crate]] (addendum-owned implementation details for roles/lifecycle/stores), [[vol-llm-agent-channel-crate]] (error vocabulary and allowlist protocol semantics), [[vol-llm-runtime-crate]] (source count for capability-source reference)
- Updated index: new source entry and refreshed concept summary
- Cross-references added: 22
- Changes: Captured the brainstormed addendum for implementation-critical details that should guide the future plan. Feishu/Lark upload: https://my.feishu.cn/docx/Rk11ddyFJoC6q2x8HOjcrwuQn4c

## [2026-06-10] ingest | Agent Server Control Plane / Data Plane Architecture
- Created sources: [[agent-server-control-data-plane-architecture]]
- Created concepts: [[agent-server-control-data-plane]]
- Updated entities: [[vol-agent-server-crate]] (final owner of concrete `DataPlaneServerCore`, `ControlPlaneServerCore`, and config-driven role composition), [[vol-llm-agent-channel-crate]] (final owner of protocol definitions, JSON-RPC transport, connection/handler/registry/service abstractions), [[vol-llm-runtime-crate]] (runtime remains data-plane capability source)
- Updated concepts: [[agent-router]] (clarified node-local router vs distributed `ControlRouter`)
- Removed obsolete entities: `vol-agent-control-plane` (final design does not add a separate crate)
- Updated index: source/concept entries and refreshed server/channel summaries
- Cross-references added: 34
- Changes: Documented the final architecture design: no new control-plane crate; `vol-llm-agent-channel` owns all JSON-RPC protocol/transport abstractions; `vol-agent-server` owns concrete data/control-plane cores and role composition; `vol-llm-runtime` remains execution resource owner. Feishu/Lark upload updated to revision 20: https://my.feishu.cn/docx/K0mGdhW5UoKL9IxVBwHcQmsxn9c

## [2026-06-10] ingest | Session Database Store Implementation
- Created sources: [[session-database-store-implementation]]
- Created concepts: [[runtime-session-store-configuration]]
- Updated entities: [[vol-session]] (SessionManager, DatabaseSessionEntryStore, DatabaseSessionManager, SeaORM sessions/session_entries schema), [[vol-llm-runtime-crate]] (runtime-owned `session_manager` and `[runtime.session_store]` config), [[vol-llm-agent-channel-crate]] (SessionHandler/register_agent use runtime session manager; JSON-RPC error payload preservation), [[vol-agent-server-crate]] (server parses, validates, logs, and forwards session store config)
- Updated concepts: [[session-as-ssot]] (file/database backend selection preserves Session SSOT model)
- Updated index: new source and concept entries, refreshed session/runtime/server/channel summaries
- Cross-references added: 28
- Changes: Documented the completed database-backed session store implementation: SeaORM SQLite/Postgres persistence with compiled migrations, scoped session manager APIs, runtime/server config, channel JSON-RPC integration, Postgres test isolation, and final verification caveats for unrelated workspace checks.

## [2026-06-09] ingest | File Session Agent ID Validation
- Created sources: [[file-session-agent-id-validation]]
- Updated entities: [[vol-session]] (`FileSessionManager` validates agent IDs, `StoreError::InvalidInput` added, invalid infallible stores use encoded quarantine paths)
- Updated index: new source entry and refreshed `vol-session` summary/date
- Cross-references added: 8
- Changes: Documented the Task 1 code-quality fix for filesystem path traversal risk in file-backed session manager agent IDs. Fallible APIs reject invalid IDs, while `entry_store_for_agent` safely roots invalid IDs under `agents_root/.invalid-agent-id/<hex>/sessions`; `cargo test -p vol-session` passed with 66 tests.

## [2026-06-09] ingest | SeaORM Task Database Store Implementation
- Created sources: [[seaorm-task-database-store-implementation]]
- Updated entities: [[vol-llm-task-crate]] (SeaORM entity/migration/mapping replaces SQLx, SQLite + Postgres implemented, crate-root export), [[vol-llm-runtime-crate]] (SeaORM runtime database store construction and Postgres builder test with env-var DSN), [[vol-agent-server-crate]] (server config pass-through and startup logging; no changes needed)
- Updated concepts: [[runtime-task-store-configuration]] (completed SeaORM file/database runtime behavior, credential hygiene, non-goals)
- Updated index: new SeaORM task database store implementation source entry, refreshed task crate summary
- Cross-references added: 20
- Changes: Documented the completed SeaORM replacement of the SQLx database task store: `DatabaseTaskStore` uses SeaORM entity (`tasks`), SeaORM Rust migrator compiled into the binary, mapping helpers, SQLite/Postgres CRUD, ready-task behavior, cross-process test lock, mandatory `VOL_AGENT_POSTGRES_TEST_URL`, single global `runtime.task_store` semantics, and explicit exclusion of `.agents/task-providers` or per-agent stores.

## [2026-06-09] ingest | SeaORM Postgres Test URL Env Var Fix
- Created sources: [[seaorm-postgres-test-url-env-fix]]
- Updated entities: [[vol-llm-task-crate]] (mandatory Postgres tests read `VOL_AGENT_POSTGRES_TEST_URL`), [[vol-llm-runtime-crate]] (runtime Postgres builder test uses the same env var)
- Updated concepts: [[runtime-task-store-configuration]] (credential hygiene and mandatory env-var behavior for Postgres tests)
- Updated sources: [[seaorm-postgres-test-isolation-fix]] (removed outdated claim that the fixed live URL remains in test code)
- Updated index: new SeaORM Postgres test URL env-var source entry
- Cross-references added: 10
- Changes: Documented the review fix that removed the live Postgres DSN from committed test source and docs. Mandatory Postgres tests now fail clearly if `VOL_AGENT_POSTGRES_TEST_URL` is unset, and docs use `postgres://USER:PASSWORD@HOST:5432/DATABASE` as the placeholder DSN.

## [2026-06-09] ingest | SeaORM Postgres Test Isolation Fix
- Created sources: [[seaorm-postgres-test-isolation-fix]]
- Updated entities: [[vol-llm-runtime-crate]] (runtime Postgres task-store test marker cleanup and cross-process lock), [[vol-llm-task-crate]] (Postgres database tests share the same temp-dir lock)
- Updated concepts: [[runtime-task-store-configuration]] (Postgres test isolation and cleanup expectations)
- Updated index: new SeaORM Postgres test isolation source entry
- Cross-references added: 9
- Changes: Documented the SeaORM Task 6 review fix: runtime and task-store Postgres tests coordinate through a shared OS file lock, runtime test rows use a UUID subject marker with before/after cleanup through `TaskStore`, and the example Postgres DSN is now a placeholder.

## [2026-06-09] ingest | SeaORM SQLite URL Normalization Fix
- Created sources: [[seaorm-sqlite-url-normalization-fix]]
- Updated entities: [[vol-llm-task-crate]] (SeaORM SQLite URL normalization exact `mode` query-key behavior)
- Updated concepts: [[runtime-task-store-configuration]] (SQLite create-mode normalization edge case)
- Updated index: new SeaORM SQLite URL normalization source entry
- Cross-references added: 6
- Changes: Documented the SeaORM Task 1 review fix where `normalize_sqlite_url` parses query parameters and checks for an exact `mode` key, ensuring `journal_mode=wal` still appends `mode=rwc` while explicit `mode=rwc` remains unchanged.

## [2026-06-09] ingest | Task Database Store Implementation
- Created sources: [[task-database-store-implementation]]
- Updated entities: [[vol-llm-task-crate]] (DatabaseTaskStore CRUD, ready-task behavior, crate-root export), [[vol-llm-runtime-crate]] (single global runtime task store construction), [[vol-agent-server-crate]] (server config pass-through and startup logging), [[vol-llm-agent-channel-crate]] (builder pass-through and shared TaskHandler store semantics)
- Updated concepts: [[runtime-task-store-configuration]] (completed file/database runtime behavior and non-goals)
- Updated index: new end-to-end implementation source entry, refreshed task-store concept and agent-channel summaries
- Cross-references added: 18
- Changes: Documented the completed implementation of `[runtime.task_store] type = "database"`: SQLx SQLite `DatabaseTaskStore` with embedded migrations, config validation, runtime construction, task tool/RPC sharing of one global `runtime.task_store`, final verification commands, and explicit exclusion of `.agents/task-providers` or per-agent stores.

## [2026-06-09] ingest | Runtime Database Task Store Construction
- Created sources: [[runtime-database-task-store-construction]]
- Updated entities: [[vol-llm-runtime-crate]] (runtime database task-store construction and persistence test coverage)
- Updated concepts: [[runtime-task-store-configuration]] (database config now maps to real runtime construction)
- Updated index: new runtime database construction source entry
- Cross-references added: 8
- Changes: Documented Task 6 runtime database store wiring and the review fix that made the builder test require successful runtime construction plus task create/get persistence across SQLite-backed runtime rebuilds.

## [2026-06-09] ingest | Task Store SQLite Embedded Migrations
- Created sources: [[task-store-sqlite-embedded-migrations]]
- Created entities: [[vol-llm-task-crate]]
- Updated concepts: [[runtime-task-store-configuration]] (linked SQLite database-store initialization to embedded migrations)
- Updated index: new task crate entity and embedded migrations source entries
- Cross-references added: 5
- Changes: Documented the Task 4 review fix: `DatabaseTaskStore` now uses a compile-time SQLx migrator for SQLite migrations, and the workspace `sqlx` dependency enables macros so release binaries/containers do not need source-tree migration files at runtime.

## [2026-06-09] ingest | Runtime Task Store Config Parsing
- Created sources: [[task-store-config-parsing]]
- Created entities: [[vol-llm-runtime-crate]], [[vol-agent-server-crate]]
- Created concepts: [[runtime-task-store-configuration]]
- Updated index: new runtime/server entities, task store configuration concept, and source entry
- Cross-references added: 9
- Changes: Added wiki coverage for Task 1 of the database task store plan: runtime-owned `TaskStoreType`/`TaskStoreConfig`, SQL-independent database URL scheme validation, server `[runtime.task_store]` parsing, load-time validation, and config tests.

## [2026-06-04] ingest | Rich Text Conversation Rendering
- Created sources: [[rich-text-conversation-design]]
- Created concepts: [[rich-text-conversation]]
- Updated entities: [[vol-llm-ui-crate]] (conversation.rs: html_escape, markdown_container, 4 render sites; app.rs: include_str embed; index.html: CDN scripts; new assets: markdown.js, markdown.css)
- Updated index: new concept + source entries, updated date
- Cross-references added: 6
- Changes: Added markdown rendering pipeline for agent answers and tool results. Dioxus emits <div data-md="1"><pre data-md-raw>...</pre></div>; embedded markdown.js (MutationObserver + 100ms debounce) renders via marked + DOMPurify + highlight.js. CDN scripts loaded synchronously in index.html. 10 pre-registered languages for syntax highlighting. 2px scroll threshold for stick-to-bottom escape.

## [2026-06-04] ingest | Task Dependency Graph View
- Created sources: [[task-dependency-graph-view]]
- Created concepts: [[dependency-graph-visualization]]
- Updated entities: [[vol-llm-ui-crate]] (TasksPanel/TaskDepGraph components, "⇄ deps" button, TaskEntry PartialEq, pub(crate) status_color, timeline entry, source_count 23→24)
- Updated index: new concept + source entries, updated date
- Cross-references added: 8
- Changes: Added a per-row "⇄ deps" button to the Tasks tab that opens an SVG node-link dependency-graph modal (`TaskDepGraph`) centered on the task. Pure `build_graph_layout` uses longest-path (Sugiyama-style) layering of the full transitive closure (upstream `dependencies` above, downstream `blocks` below), is cycle-safe, marks not-loaded nodes, and skips self-loops; 7 unit tests. Read-only, frontend-only — no backend changes (data already on the wire). Panel-local `graph_target` signal; modal reuses the approval_dialog shell.

## [2026-05-29] ingest | Remove vol-agent-manager and Legacy Frontend
- Created sources: [[remove-vol-agent-manager]]
- Updated entities: [[vol-llm-ui-crate]], [[vol-llm-agent-channel-crate]]
- Updated index: new source entry, updated date
- Cross-references added: 4
- Changes: Removed obsolete `vol-agent-manager` crate, manager-only Docker/Kubernetes artifacts, and legacy React `frontend/`; the active web backend is now documented as `vol-agent-server` via `make web-backend`, using channel-owned JSON-RPC protocol/transport abstractions.

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
- Changes: EventBridgePlugin deleted, JsonRpcHandler/JsonRpcContext replaced by JsonRpcConnection implementing Connection trait; the historical JsonRpcServer gained registration-list multi-agent support; agent.submit gained optional target param; 49 integration tests; wire format preserved

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
