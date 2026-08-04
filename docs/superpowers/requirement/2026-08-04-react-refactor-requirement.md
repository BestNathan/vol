# Requirements: vol-llm-ui React Refactor

## Background

`vol-llm-ui` is currently built with Rust/Dioxus/WASM. Development velocity is severely hampered by:
- Dioxus reactive primitives (`write_unchecked`, thread-locals for event routing, manual Signal cloning) add excessive friction
- WASM compile times (minutes per change) kill iteration speed
- No component library — every UI element is hand-built with raw Tailwind classes
- Skill market for Dioxus developers is essentially zero; React developers are abundant
- The `make web-*` three-terminal workflow (Tailwind watch + dx serve + cargo watch) is fragile

The decision has been made to **rewrite the frontend in React** while keeping the backend (`vol-agent-server`) completely unchanged.

## Goals

1. **100% feature parity** — every page, component, dialog, sub-tab, and interaction in the current Dioxus UI must exist in the React version
2. **Fix known gaps** — the rewrite is the opportunity to complete features that are currently stubbed or broken:
   - Approval dialog must actually call `agent.approve` RPC with `req_id`, `approved`, `reason` params (currently only clears local state)
   - MCP Prompt Viewer must call `mcp.get_prompt` RPC (currently hardcoded "not implemented" error)
   - Input area must expose a Cancel button when an agent is running (calling `agent.cancel` RPC)
3. **Backend zero-change** — `vol-agent-server` must not require any modifications. The React frontend speaks the exact same JSON-RPC/WebSocket protocol
4. **Modern dev experience** — HMR, fast builds, standard React tooling, single `npm run dev`
5. **Shadcn/ui design system** — all UI components use shadcn/ui primitives, replacing hand-rolled Tailwind classes

## Non-Goals

- **Backend changes** — the JSON-RPC/WebSocket protocol, route paths, and server behavior remain exactly as-is
- **Feature additions beyond gap fixes** — no new tabs, no new RPC endpoints, no new capabilities beyond the 3 gap fixes listed above
- **Light mode** — dark theme only (matching current)
- **i18n** — no internationalization; English UI only (matching current)
- **URL routing** — no client-side router (matching current tab-switching pattern, no deep links)
- **SSR/SSG** — pure client-side SPA (matching current WASM model)
- **Storybook/component library** — out of scope for v1

## Scope

### Included (complete component inventory)

| Current Component | React Equivalent | Notes |
|---|---|---|
| App (app.rs) | `<App />` | Root layout, signal wiring → context/hooks |
| StatusBar | `<StatusBar />` | Connection indicator, session info, run stats, nodes dropdown, CapabilityBar |
| FileTree | `<FileTree />` | Left sidebar, collapsible dirs, emoji icons, mobile drawer |
| TabBar + TabContent | `<TabBar />` + `<TabContent />` | 7 tabs: Tasks, Agents, Tools, Workspace, Skills, MCP, Logs |
| AgentsPanel | `<AgentsPanel />` | Agent card grid, select/deselect, sub-tabs |
| ConversationView | `<ConversationView />` | Streaming timeline, markdown, tool detail modals |
| InputArea | `<InputArea />` | Text input, submit, cancel button (new), new session |
| TasksPanel | `<TasksPanel />` | Status filter chips, expandable rows |
| TaskDepGraph | `<TaskDepGraph />` | SVG dependency graph modal |
| ToolsTabContent | `<ToolsTab />` | System tools + call history |
| SchemaForm | `<SchemaForm />` | JSON Schema → form renderer |
| SkillsPanel | `<SkillsPanel />` | Table/cards, search |
| McpPanel | `<McpPanel />` | Sub-tabs: Servers/Tools/Resources/Prompts |
| Workspace (FileContentView) | `<FileContentView />` | Open file tabs |
| LogViewer | `<LogViewer />` | Run list + entry drilldown |
| CapabilityBar | `<CapabilityBar />` | Summary bar: N tools · N skills · N MCPs |
| CapabilityDrawer | `<CapabilityDrawer />` | Right panel, search, toggles, instant-apply |
| NodesPanel + NodeDetailPanel | `<NodesPanel />` + `<NodeDetailPanel />` | Node list + detail drilldown |
| NodesDropdown | `<NodesDropdown />` | Status bar node selector |
| SessionsPanel | `<SessionsPanel />` | Session list, view/resume |
| ApprovalDialog | `<ApprovalDialog />` | HITL modal — **must call RPC** (gap fix) |
| ToolCallDialog | `<ToolCallDialog />` | MCP tool execution |
| ResourceViewer | `<ResourceViewer />` | MCP resource reading |
| PromptViewer | `<PromptViewer />` | MCP prompt viewing — **must call RPC** (gap fix) |
| SkillDetailDialog | `<SkillDetailDialog />` | Skill detail modal |
| SessionDetailOverlay | `<SessionDetailOverlay />` | Rendered conversation preview |
| ContextPanel + ContextDialog | `<ContextPanel />` + `<ContextDialog />` | Contributor list + message viewer |
| DebugPanel | `<DebugPanel />` | WS message inspector |
| ToolDetailModal | `<ToolDetailModal />` | Tool call arguments/result viewer |

### Excluded (dead code — do NOT port)

- `WorkspacePanel` (legacy flat view, not routed)
- `SessionDialog` (legacy, not used)
- `ToolsPanel` (legacy, superseded by ToolsTabContent)
- `src/hooks/mod.rs` (empty placeholder)

## Constraints

1. **Protocol**: JSON-RPC 2.0 over WebSocket — exactly matching current `client.rs` wire format
2. **Dual connection**: Control Plane WS + per-node Data Plane WS pool (current `DpConnectionPool` pattern)
3. **Streaming**: Agent events arrive as a push stream via `agent.event` JSON-RPC notifications — must handle incremental rendering (thinking deltas, content deltas)
4. **Tech stack**: Vite + React 18+ + TypeScript + shadcn/ui + Tailwind CSS v4
5. **State management**: React Context + a lightweight state library (Zustand or Jotai — to be decided in brainstorming phase)
6. **Markdown**: `react-markdown` + `rehype-sanitize` + `rehype-highlight` (replacing marked/DOMPurify/hljs CDN pipeline)
7. **Dark theme only**: Must use shadcn's dark theme CSS variables, matching current color palette
8. **Mobile responsive**: Must preserve current mobile patterns (drawer-based file tree, card vs table layouts at sm:480px)
9. **iOS Safari**: Must prevent zoom on input focus (16px font-size on inputs, viewport meta)

## Success Criteria

1. **Feature completeness**: All 28 components listed in Scope are implemented and functional
2. **Protocol compatibility**: All JSON-RPC methods in `client.rs` (35+ method call sites as of this writing, plus `agent.approve` sent via the connection layer) are callable and produce correct request/response behavior
3. **Streaming**: Conversation view renders streaming agent output in real-time (thinking → content → tools → results)
4. **Reconnection**: WebSocket disconnect triggers visible reconnection UI with countdown, auto-reconnects, restores session
5. **Dual connection**: In ControlPlane mode, node selection creates DP connections; agent events from DP nodes appear in conversation
6. **Gap fixes**: Approval dialog sends RPC; Prompt viewer sends RPC; Cancel button stops running agent
7. **Playwright parity**: Existing Playwright tests (`capability_drawer.spec.js`, `markdown.spec.js`) pass against React version (adapted for new DOM structure)
8. **Mobile**: UI is usable at 480px width — file tree collapses to drawer, tables switch to cards, font sizes prevent iOS zoom
9. **Dev experience**: `npm run dev` starts Vite HMR dev server; `npm run build` produces production bundle
10. **No backend changes**: `vol-agent-server` starts with `make web-backend` and works against React frontend with zero modifications

## Edge Cases

| Scenario | Expected Behavior |
|---|---|
| WebSocket disconnected mid-stream | Show reconnecting UI with countdown; buffer or discard in-flight events; restore session on reconnect |
| All reconnect attempts exhausted | Show "Connection lost. Please refresh." message; disable input |
| User switches agents mid-run | Each agent maintains independent conversation; events are routed to correct agent via run_map |
| User rapidly toggles capability switches | Instant-apply with race-condition guard — only last response applied; rollback on error with warning icon |
| Empty search in CapabilityDrawer | Show all toggles (matching current) |
| Agent running — user clicks submit | Input is disabled while agent is running (matching current) |
| Node with no ws_url | Cannot be selected; show "offline" state |
| MCP server reconnect failure | Show per-server "Reconnecting..." pulse; retry on manual button click |
| Session with 0 entries | Show empty state, not error |
| Very long tool result (>50KB) | Truncate display to 200 chars in preview; full content in modal |
| XSS in LLM output | `rehype-sanitize` strips dangerous tags; markdown rendered as sanitized HTML |
| User scrolls up during streaming | Auto-scroll pauses; resumes when user scrolls back to bottom (2px threshold) |
| Browser tab hidden during streaming | Render throttling — max ~12 renders/sec (matching current Playwright assertion) |
| Double Esc in input | Clear input text |
| File tree directory with 1000+ files | Virtual scrolling or pagination not required — current behavior (render all) is acceptable |
| Task dependency cycle | Graph layout handles cycles gracefully (dashed "unknown" nodes) |
| Server returns ControlPlane type | Auto-fetch node list; auto-select first online node with ws_url; create DP connection |
| Server returns DataPlane type | Direct connection; no node dropdown needed |
| Page refresh | New WebSocket connection; server identifies as CP/DP; fresh state (no session persistence in browser) |

## Open Questions

1. **State management library**: Zustand vs Jotai vs Redux Toolkit — to be decided in brainstorming phase based on:
   - WebSocket subscription management ergonomics
   - Per-agent conversation isolation
   - DevTools and debugging experience

2. **Monorepo placement**: Should the React frontend live in:
   - `crates/vol-llm-ui/` (replacing Dioxus code)
   - A new top-level `frontend/` directory
   - A new `web/` or `app/` directory
   (To be decided in brainstorming)

3. **Dioxus code fate**: Should the existing `crates/vol-llm-ui/src/web/` be:
   - Deleted immediately after React version is ready
   - Kept behind a feature flag during transition
   - Archived as reference

4. **Shared types**: The `UiEvent` enum and data types (`ConversationEntry`, `ToolCallEntry`, etc.) are defined in Rust. Should we:
   - Manually rewrite TypeScript types (risk of drift)
   - Generate TypeScript types from Rust via `ts-rs` or similar
   - Keep manual with integration tests as the safety net

5. **Testing strategy**: Beyond porting existing Playwright tests, should we add:
   - Component unit tests (Vitest + React Testing Library)
   - WebSocket mock testing (MSW or custom mock server)
   - Visual regression tests
