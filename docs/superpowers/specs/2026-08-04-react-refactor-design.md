# Design: vol-llm-ui React Refactor

## Summary

Rewrite `vol-llm-ui` from Rust/Dioxus/WASM to Vite + React 18 + TypeScript + shadcn/ui, maintaining 100% feature parity with the current frontend while keeping the `vol-agent-server` backend completely unchanged.

**Requirements:** [[2026-08-04-react-refactor-requirement]]

## Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                    React SPA (Vite)                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐               │
│  │  Jotai   │  │  React   │  │ shadcn/ui│               │
│  │  Stores  │  │Components│  │  (dark)  │               │
│  └────┬─────┘  └────┬─────┘  └──────────┘               │
│       │              │                                   │
│  ┌────┴──────────────┴─────┐                             │
│  │    lib/jsonrpc-client   │  ← JSON-RPC 2.0 over WS    │
│  │    lib/dp-pool          │                             │
│  │    lib/reconnect        │                             │
│  └──────────┬──────────────┘                             │
└─────────────┼────────────────────────────────────────────┘
              │  ws://host/ws (same-origin)
    ┌─────────┴──────────┐
    │  vol-agent-server   │  ← Zero changes
    │  (CP / DP modes)    │
    └────────────────────┘
```

## Project Structure

```
frontend/                          # repo-root, sibling to crates/
├── package.json                   # Vite + React 18 + TypeScript
├── vite.config.ts
├── tailwind.config.ts             # shadcn/ui preset + dark theme tokens
├── tsconfig.json
├── components.json                # shadcn/ui configuration
├── index.html
├── public/
├── tests/
│   ├── unit/                      # Vitest unit tests
│   └── e2e/                       # Playwright (adapted from current)
└── src/
    ├── main.tsx                   # ReactDOM.createRoot, global Provider tree
    ├── App.tsx                    # Root layout: StatusBar + FileTree + TabBar + TabContent
    ├── components/
    │   ├── ui/                    # shadcn/ui primitives (Button, Dialog, Input, Badge, etc.)
    │   ├── layout/                # App shell, StatusBar, TabBar, TabContent
    │   ├── panels/                # Feature panels (one per tab / sub-tab)
    │   │   ├── AgentsPanel.tsx
    │   │   ├── ConversationView.tsx
    │   │   ├── FileTree.tsx
    │   │   ├── FileContentView.tsx
    │   │   ├── ToolsTab.tsx
    │   │   ├── SkillsPanel.tsx
    │   │   ├── McpPanel.tsx
    │   │   ├── TasksPanel.tsx
    │   │   ├── LogViewer.tsx
    │   │   ├── SessionsPanel.tsx
    │   │   ├── ContextPanel.tsx
    │   │   ├── NodesPanel.tsx
    │   │   └── NodeDetailPanel.tsx
    │   ├── dialogs/               # Modal/overlay components
    │   │   ├── ApprovalDialog.tsx
    │   │   ├── ToolCallDialog.tsx
    │   │   ├── ResourceViewer.tsx
    │   │   ├── PromptViewer.tsx
    │   │   ├── SkillDetailDialog.tsx
    │   │   ├── SessionDetailOverlay.tsx
    │   │   ├── ContextDialog.tsx
    │   │   ├── DebugPanel.tsx
    │   │   ├── ToolDetailModal.tsx
    │   │   └── TaskDepGraph.tsx
    │   ├── inputs/                # Input-area components
    │   │   ├── InputArea.tsx
    │   │   ├── SchemaForm.tsx
    │   │   ├── CapabilityBar.tsx
    │   │   └── CapabilityDrawer.tsx
    │   └── shared/                # Reusable sub-components
    │       ├── ConnectionIndicator.tsx
    │       ├── NodesDropdown.tsx
    │       └── Markdown.tsx
    ├── hooks/                     # Shared React hooks
    │   ├── useJsonRpc.ts
    │   ├── useEventStream.ts
    │   ├── useAutoScroll.ts
    │   └── useThrottledValue.ts
    ├── stores/                    # Jotai atoms (one file per domain)
    │   ├── connection.ts          # wsConnected, serverMode, reconnectState
    │   ├── agents.ts              # agents[], selectedAgentId, agentStatus
    │   ├── conversation.ts        # atomFamily: conversationByAgent[agentId]
    │   ├── tools.ts               # toolCalls[], systemTools[]
    │   ├── workspace.ts           # workspaceTree, openFiles[]
    │   ├── skills.ts              # skills[]
    │   ├── mcp.ts                 # mcpState, activeSubtab
    │   ├── tasks.ts               # tasks[], statusFilter
    │   ├── logs.ts                # logRuns[], logEntries[]
    │   ├── sessions.ts            # sessions[]
    │   ├── capability.ts          # capOverlay, drawerOpen, savingStates
    │   ├── dialogs.ts             # approval, mcp dialogs, skill dialog, debug
    │   ├── context.ts             # contributors, contextDialogState
    │   ├── cache.ts               # nodeDataCache: atomFamily([nodeId, key])
    │   └── ui.ts                  # activeTab, globalRunState, viewingNodeDetail
    ├── lib/
    │   ├── jsonrpc-client.ts      # WebSocket JSON-RPC client
    │   ├── dp-pool.ts             # Per-node data-plane connection pool
    │   ├── reconnect.ts           # Exponential-backoff pure function
    │   └── protocol.ts            # All wire types: UiEvent, method signatures, etc.
    └── types/
        └── index.ts               # Shared TypeScript interfaces
```

## State Management: Jotai

### Why Jotai

- Closest mental model to Dioxus Signals (independent atoms, fine-grained subscription)
- `atomFamily` maps directly to per-agent conversations (`HashMap<agent_id, AgentConversation>`)
- `atomFamily([nodeId, key])` maps directly to `NodeDataCache`
- No boilerplate — an atom is `const countAtom = atom(0)`, reads as `useAtomValue(countAtom)`, writes as `useSetAtom(countAtom)`

### Store Topology

```
stores/
├── connection.ts    # wsConnected, serverMode, wsUrl, reconnectState
├── agents.ts        # agents[], selectedAgentId, agentsLoading, agentStatus
├── conversation.ts  # atomFamily: conversationByAgent[agentId], activeAgentId
├── tools.ts         # toolCalls[], systemTools[], toolsLoading
├── workspace.ts     # workspaceTree, openFiles[], selectedFileTab
├── skills.ts        # skills[], skillsLoading
├── mcp.ts           # mcpState (server/tools/resources/prompts), activeSubtab
├── tasks.ts         # tasks[], statusFilter, selectedTaskId
├── logs.ts          # logRuns[], selectedRun, logEntries[]
├── sessions.ts      # sessions[], sessionsLoading
├── capability.ts    # capOverlay, drawerOpen, searchText, savingStates
├── dialogs.ts       # approvalDialog, mcpDialogState, skillDialog, debugPanel
├── context.ts       # contextContributors, contextDialogState
├── cache.ts         # nodeDataCache: atomFamily([nodeId, key])
└── ui.ts            # activeTab, globalRunState, fileTreeDrawerOpen, viewingNodeDetail
```

### Event Flow (replaces EventBus)

```
WebSocket push agent.event → JsonRpcClient.eventStream()
  → event loop (useEventStream hook, runs once at App level)
    → match on UiEvent variant name (AgentStart, ContentDelta, ToolCallComplete, ...)
      → write Jotai atom:
        • ContentDelta → conversationByAgent(agentId) atom (append delta to last entry)
        • ToolCallComplete → tools atom (update call status) + conversation atom (append ToolResult)
        • AgentComplete → global atoms (run_elapsed, is_running=false)
        • WsConnected → connection atom
```

No more pub/sub bus — Jotai's reactive dependency graph handles cross-component updates naturally.

## WebSocket / JSON-RPC Layer

### JsonRpcClient (`lib/jsonrpc-client.ts`)

```typescript
class JsonRpcClient {
  constructor(url: string, opts?: { autoSubscribe?: boolean })

  // Request/response (callback → Promise via .then)
  call<T>(method: string, params?: unknown): Promise<T>

  // Push event stream (agent.event notifications)
  eventStream(): AsyncIterable<AgentEvent>

  // Connection lifecycle
  onStateChange(cb: (state: ConnectionState) => void): void
  reconnect(): void
  sendRaw(message: string): void  // debug only
}

type AgentEvent = { run_id: string; event: Record<string, unknown> }
type ConnectionState = 'connecting' | 'connected' | 'disconnected'
```

**Implementation:** Native `WebSocket` API. `call()` assigns incrementing JSON-RPC `id`, stores callback in `Map<id, resolve/reject>`. Response handler matches `id` → invokes callback. Notifications (`method` field, no `id`) → routed to `eventStream` consumer.

### DpConnectionPool (`lib/dp-pool.ts`)

```typescript
class DpConnectionPool {
  getOrCreate(nodeId: string, wsUrl: string): JsonRpcClient
  get(nodeId: string): JsonRpcClient | undefined
  connections(): [string, JsonRpcClient][]
}
```

Lazy creation on first `getOrCreate` per node. Each DP client auto-subscribes on open. Pool iterable for event forwarding loop.

### Reconnect (`lib/reconnect.ts`)

Pure function, no React dependency:

```typescript
async function attemptReconnect(
  client: JsonRpcClient,
  onAttempt: (attempt: number, delaySecs: number) => void
): Promise<boolean>
```

Exponential backoff: 10 attempts, delay = min(3 × 2^(attempt-1), 30)s. Returns `true` if reconnected, `false` if exhausted.

### Wire Types (`lib/protocol.ts`)

Manually maintained TypeScript mirrors of Rust types:
- `UiEvent` (discriminated union, keyed by `type` field)
- `AgentEvent` / `AgentStreamEvent` (externally tagged)
- RPC method signatures: `agent.submit`, `agent.approve`, `agent.cancel`, `agent.list`, `agent.status`, `agent.get_capabilities`, `agent.update_capabilities`, `agent.context_config`, `agent.context_snapshot`, `session.list`, `session.entries`, `session.resume`, `file.list`, `file.read`, `tool.list`, `tool.call`, `skill.list`, `skill.get`, `skill.refresh`, `mcp.list_servers`, `mcp.list_tools`, `mcp.list_resources`, `mcp.list_resource_templates`, `mcp.list_prompts`, `mcp.read_resource`, `mcp.call_tool`, `mcp.reconnect`, `mcp.get_prompt`, `task.list`, `task.get`, `log.list`, `log.read`, `system.connected`, `control.node_list`, `control.node_get`, `control.capability_list`

## Component Architecture

### Layout Tree

```
<div data-app-root>  (dark theme, #1a1a2e bg)
├── <StatusBar>
│   ├── <ConnectionIndicator />   (connected/reconnecting/disconnected/error)
│   ├── <NodesDropdown />         (ControlPlane mode only)
│   ├── Run stats (session, run#, iter, tools, time)
│   └── Debug toggle
├── <div.flex>
│   ├── <FileTree />              (left sidebar / mobile drawer)
│   └── <main>
│       ├── <TabBar />            (Tasks|Agents|Tools|Workspace|Skills|MCP|Logs)
│       └── <TabContent />        (switch on activeTab)
├── <ApprovalDialog />            (z-100 overlay)
├── <ToolCallDialog />            (MCP dialog)
├── <ResourceViewer />
├── <PromptViewer />
├── <SkillDetailDialog />
├── <DebugPanel />
└── <CapabilityDrawer />          (right-side fixed panel)
```

### Data Flow (single-direction)

```
WebSocket event
  → eventStream loop (App level)
    → write Jotai atoms
      → React re-renders only affected components
```

Components never read from each other. Components call `client.call()` for mutations.

### Key Interaction Flows

1. **User submits message:** InputArea → `client.call('agent.submit', {input, target})` → run_id → WS pushes AgentStart/ThinkingDelta/ContentDelta/ToolCall*/AgentComplete events → conversation + tool atoms update → UI re-renders
2. **Streaming content:** ContentDelta → append to last ContentStreaming entry → Markdown component re-renders via `useThrottledValue(50ms)`
3. **Capability toggle:** CapabilityDrawer → optimistic local update → `client.call('agent.update_capabilities')` → success: update effective atoms / error: rollback + show warning
4. **Agent switch:** AgentsPanel → set `selectedAgentId` atom → conversation atom family switches active → UI shows that agent's entries (from nodeDataCache)
5. **Node select (CP mode):** NodesDropdown → `pool.getOrCreate(nodeId, wsUrl)` → `activeNodeId` atom → AgentsPanel reloads agents from DP
6. **Reconnect:** WS onClose → `attemptReconnect(client, cb)` → StatusBar shows countdown → on success: `client.call('session.list')` → restore latest session → rebuild conversation

## Component Inventory

### Layout (4 components)
| Component | Purpose |
|---|---|
| App | Root layout, provider tree, event stream loop |
| StatusBar | Connection indicator, nodes dropdown, run stats, debug toggle |
| TabBar | 7 tab buttons: Tasks, Agents, Tools, Workspace, Skills, MCP, Logs |
| TabContent | Router: switch on activeTab → render panel |

### Panels (11 components)
| Component | Purpose |
|---|---|
| AgentsPanel | Agent card grid + sub-tabs (Conversation/Sessions/Context/Tasks) |
| ConversationView | Streaming timeline with markdown, tool detail modals |
| FileTree | Collapsible directory tree, lazy loading, mobile drawer |
| FileContentView | Open file tabs with content display |
| ToolsTab | System tools list + call history |
| SkillsPanel | Skills table/cards, search |
| McpPanel | Sub-tabs: Servers/Tools/Resources/Prompts |
| TasksPanel | Status filter chips, expandable rows, dep graph trigger |
| LogViewer | Run list + per-run entry drilldown |
| SessionsPanel | Session list, view overlay, resume |
| ContextPanel | Contributor list + snapshot dialog |
| NodesPanel | Node list + node detail drilldown |

### Dialogs (10 components)
| Component | Purpose | Gap Fix |
|---|---|---|
| ApprovalDialog | HITL modal: tool name, reason, approve/reject | Must call `agent.approve` RPC |
| ToolCallDialog | MCP tool execution with schema form | — |
| ResourceViewer | MCP resource reader | — |
| PromptViewer | MCP prompt execution | Must call `mcp.get_prompt` RPC |
| SkillDetailDialog | Skill detail + file preview | — |
| SessionDetailOverlay | Session entries rendered as conversation | — |
| ContextDialog | Contributor messages viewer | — |
| DebugPanel | WS message inspector | — |
| ToolDetailModal | Tool call arguments/result in conversation | — |
| TaskDepGraph | SVG dependency graph | — |

### Inputs (4 components)
| Component | Purpose | Gap Fix |
|---|---|---|
| InputArea | Text input, submit, new session | Add Cancel button → `agent.cancel` RPC |
| SchemaForm | JSON Schema → form renderer | — |
| CapabilityBar | Summary: N tools · N skills · N MCPs | — |
| CapabilityDrawer | Right panel: search, toggles, instant-apply | — |

### Shared (3 components)
| Component | Purpose |
|---|---|
| ConnectionIndicator | Connection status with reconnecting countdown |
| NodesDropdown | Node selector in status bar |
| Markdown | Streaming-safe markdown renderer with throttle |

## Markdown Rendering

- **Library:** `react-markdown` + `remark-gfm` + `rehype-sanitize` + `rehype-highlight`
- **Streaming throttle:** `useThrottledValue(content, 80)` — limits to ~12 renders/sec, matching current Playwright assertion
- **Sanitization:** `rehype-sanitize` with custom schema — strip `img`, `video`, `script`, `iframe`, `object`, `embed` (mirrors current DOMPurify `FORBID_TAGS`)
- **Highlight theme:** `highlight.js` `atom-one-dark` (via `rehype-highlight`)
- **No CDN dependencies** — everything bundled via npm

## Phased Implementation

| Phase | Scope | Verification |
|---|---|---|
| **P1: Shell + Connection** | Vite scaffold, shadcn/ui config, tailwind dark theme, `JsonRpcClient`, `DpPool`, reconnect logic, Jotai stores (connection + ui), App Shell (StatusBar + TabBar + TabContent placeholders), FileTree (static) | WS connects, StatusBar shows status, tabs switch, FileTree renders |
| **P2: Agents + Conversation** | AgentsPanel (list + select), ConversationView (streaming + Markdown + all entry types), InputArea (submit + Cancel button), CapabilityBar + CapabilityDrawer | Core flow works: select agent → chat → see streaming → tool calls → results |
| **P3: Tools + MCP + Skills** | ToolsTab (system tools + call history), SchemaForm, McpPanel (4 sub-tabs + all dialogs), SkillsPanel + SkillDetailDialog, ApprovalDialog (with `agent.approve` RPC) | System tools callable, MCP tools/resources callable, skills viewable, approval works |
| **P4: Tasks + Sessions + Context** | TasksPanel (filters + expand), TaskDepGraph (SVG), SessionsPanel (list + view + resume), ContextPanel + ContextDialog | Task filtering works, session resume restores conversation, context contributors visible |
| **P5: Workspace + Logs + Debug** | FileContentView (file tabs), LogViewer (runs + entries), NodesPanel + NodeDetailPanel, DebugPanel (WS inspector), NodesDropdown | All tabs functional, node switching works, debug panel captures |
| **P6: Polish + Tests** | Mobile responsive audit, iOS zoom prevention, Playwright test adaptation, Markdown perf tuning, dark theme consistency pass, accessibility basics | Existing Playwright tests pass, 480px usable, streaming performance acceptable |

## Technical Decisions

| Decision | Choice | Rationale |
|---|---|---|
| State management | Jotai | Closest to Dioxus Signals; `atomFamily` for per-agent/per-node data |
| Project location | `frontend/` (repo root) | Clean separation from Rust workspace, standard npm tooling |
| Component organization | Type-based (ui/layout/panels/dialogs/inputs/shared) | Chosen by user; clear role boundaries |
| WS layer | Custom hooks + lib | Protocol is simple enough; 1970 lines Rust → ~400 lines TS |
| Markdown | react-markdown + throttle hook | React-native, replaceable, matching current behavior |
| Types | Manual, Playwright safety net | Avoids CI complexity of Rust→TS codegen |
| Dioxus fate | Keep during dev, delete after | Reference during implementation, clean removal post-launch |
| No URL router | Tab state = Jotai atom | Matching current behavior; deep links not required |

## Dependencies

| Package | Purpose |
|---|---|
| react, react-dom ^18 | UI framework |
| vite | Build tool |
| @vitejs/plugin-react | Vite React integration |
| typescript | Type checking |
| jotai | State management |
| tailwindcss, @tailwindcss/vite ^4 | CSS framework |
| shadcn/ui (lucide-react, class-variance-authority, clsx, tailwind-merge) | Component primitives |
| react-markdown, remark-gfm, rehype-sanitize, rehype-highlight, highlight.js | Markdown rendering |
| vitest, @testing-library/react, @testing-library/jest-dom | Unit/component tests |
| @playwright/test | E2E tests |

## Migration Path

1. Build React frontend in `frontend/` alongside existing Dioxus code
2. `make web-backend` starts `vol-agent-server` (unchanged)
3. `npm run dev` in `frontend/` starts Vite dev server on :5173, proxying WS to backend
4. When Phase 2 completes, dev workflow switches to React; Dioxus is reference-only
5. After all 6 phases + E2E pass, delete `crates/vol-llm-ui/src/web/`, update `CLAUDE.md` and `Makefile`
6. `make web-*` targets updated: `web-css` removed (Tailwind via Vite), `web-dev` → `npm run dev --prefix frontend`, `web-backend` unchanged
