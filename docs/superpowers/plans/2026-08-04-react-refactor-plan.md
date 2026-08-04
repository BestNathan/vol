# vol-llm-ui React Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite vol-llm-ui from Rust/Dioxus/WASM to Vite + React 18 + TypeScript + shadcn/ui with 100% feature parity and zero backend changes.

**Architecture:** Jotai atoms for state (1:1 mapping to Dioxus Signals), custom JsonRpcClient class for the JSON-RPC 2.0 WebSocket protocol, type-based component organization (ui/layout/panels/dialogs/inputs/shared), react-markdown for rendered output. Single-direction data flow: WS push → event loop → write atoms → React re-render.

**Tech Stack:** React 18, TypeScript, Vite, Jotai, shadcn/ui, Tailwind CSS v4, react-markdown, Vitest, Playwright

## Global Constraints

- Backend `vol-agent-server` must require zero changes — the JSON-RPC/WebSocket protocol is frozen
- Dark theme only — `#1a1a2e` background, `#e0e0e0` text, `#80a0ff` accent
- Mobile responsive at `sm: 480px` breakpoint — file tree uses drawer, tables switch to cards
- iOS Safari must not zoom on input focus — 16px font-size on text inputs, viewport meta `maximum-scale=1.0`
- No URL router — tab state is a Jotai atom (`activeTab`)
- No dead code from Dioxus: skip WorkspacePanel, SessionDialog, ToolsPanel (legacy variants)
- Three gap fixes are required: ApprovalDialog calls `agent.approve`, PromptViewer calls `mcp.get_prompt`, InputArea has Cancel button calling `agent.cancel`

---

## Phase 1: Shell + Connection

### Task 1.1: Scaffold Vite + React + TypeScript project

**Files:**
- Create: `frontend/package.json`
- Create: `frontend/vite.config.ts`
- Create: `frontend/tsconfig.json`
- Create: `frontend/tsconfig.app.json`
- Create: `frontend/tsconfig.node.json`
- Create: `frontend/index.html`
- Create: `frontend/src/main.tsx`
- Create: `frontend/src/vite-env.d.ts`

**Interfaces:**
- Produces: `npm run dev` starts Vite HMR server on port 5173

- [ ] **Step 1: Create frontend directory and init package.json**

```bash
mkdir -p frontend/src
cd frontend
```

```json
// frontend/package.json
{
  "name": "vol-llm-ui-react",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "test": "vitest",
    "test:run": "vitest run"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "jotai": "^2.10.0"
  },
  "devDependencies": {
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.4",
    "typescript": "~5.6.0",
    "vite": "^6.0.0",
    "vitest": "^2.1.0"
  }
}
```

- [ ] **Step 2: Install dependencies**

```bash
cd frontend && npm install
```

- [ ] **Step 3: Create vite.config.ts**

```typescript
// frontend/vite.config.ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/ws': {
        target: 'ws://localhost:3001',
        ws: true,
      },
    },
  },
})
```

- [ ] **Step 4: Create tsconfig files**

```json
// frontend/tsconfig.json
{
  "files": [],
  "references": [
    { "path": "./tsconfig.app.json" },
    { "path": "./tsconfig.node.json" }
  ]
}
```

```json
// frontend/tsconfig.app.json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true,
    "paths": {
      "@/*": ["./src/*"]
    },
    "baseUrl": "."
  },
  "include": ["src"]
}
```

```json
// frontend/tsconfig.node.json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2023"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true
  },
  "include": ["vite.config.ts"]
}
```

- [ ] **Step 5: Create index.html and main.tsx**

```html
<!-- frontend/index.html -->
<!doctype html>
<html lang="en" class="dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
    <title>vol-llm-ui</title>
  </head>
  <body class="bg-[#1a1a2e] text-[#e0e0e0]">
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

```typescript
// frontend/src/main.tsx
import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from './App'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
```

- [ ] **Step 6: Verify `npm run dev` starts and renders blank page**

Run: `cd frontend && npm run dev`
Expected: Vite starts, browser shows dark background, no errors in console.

- [ ] **Step 7: Commit**

```bash
git add frontend/
git commit -m "feat(frontend): scaffold Vite + React 18 + TypeScript project"
```

### Task 1.2: Setup Tailwind CSS v4 + shadcn/ui

**Files:**
- Create: `frontend/src/index.css`
- Create: `frontend/components.json`
- Create: `frontend/src/lib/utils.ts`
- Create: `frontend/src/components/ui/button.tsx`
- Create: `frontend/src/components/ui/badge.tsx`
- Create: `frontend/src/components/ui/dialog.tsx`
- Create: `frontend/src/components/ui/input.tsx`
- Create: `frontend/src/components/ui/scroll-area.tsx`
- Create: `frontend/src/components/ui/tabs.tsx`

**Interfaces:**
- Produces: shadcn/ui primitives available at `@/components/ui/*`

- [ ] **Step 1: Install Tailwind CSS v4 and shadcn dependencies**

```bash
cd frontend
npm install -D tailwindcss @tailwindcss/vite
npm install lucide-react class-variance-authority clsx tailwind-merge
npm install @radix-ui/react-dialog @radix-ui/react-scroll-area @radix-ui/react-tabs @radix-ui/react-slot
```

- [ ] **Step 2: Create index.css with dark theme tokens**

```css
/* frontend/src/index.css */
@import "tailwindcss";

@theme {
  --color-bg-primary: #1a1a2e;
  --color-bg-secondary: #252540;
  --color-bg-tertiary: #2a2a44;
  --color-bg-status: #2d2d44;
  --color-border: #333355;
  --color-text-primary: #e0e0e0;
  --color-text-secondary: #888;
  --color-text-muted: #666;
  --color-accent: #80a0ff;
  --color-success: #40c040;
  --color-error: #c04040;
  --color-warning: #f0c040;

  --breakpoint-sm: 480px;
  --breakpoint-md: 768px;
  --breakpoint-lg: 1024px;
}

/* shadcn dark theme CSS variables */
:root {
  --background: 240 10% 11%;       /* #1a1a2e */
  --foreground: 0 0% 88%;          /* #e0e0e0 */
  --card: 240 10% 15%;             /* #252540 */
  --card-foreground: 0 0% 88%;
  --popover: 240 10% 15%;
  --popover-foreground: 0 0% 88%;
  --primary: 220 100% 75%;         /* #80a0ff */
  --primary-foreground: 240 10% 11%;
  --secondary: 240 10% 17%;        /* #2a2a44 */
  --secondary-foreground: 0 0% 88%;
  --muted: 240 10% 17%;
  --muted-foreground: 0 0% 53%;    /* #888 */
  --accent: 240 10% 17%;
  --accent-foreground: 0 0% 88%;
  --destructive: 0 62% 50%;        /* #c04040 */
  --destructive-foreground: 0 0% 88%;
  --border: 240 10% 27%;
  --input: 240 10% 27%;
  --ring: 220 100% 75%;
  --radius: 0.375rem;
}

body {
  font-family: system-ui, -apple-system, sans-serif;
  font-size: 14px;
}
```

- [ ] **Step 3: Create components.json for shadcn/ui**

```json
// frontend/components.json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "default",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "src/index.css",
    "baseColor": "neutral",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  }
}
```

- [ ] **Step 4: Create lib/utils.ts**

```typescript
// frontend/src/lib/utils.ts
import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
```

- [ ] **Step 5: Create shadcn/ui primitives**

Create these files with standard shadcn implementations:
- `frontend/src/components/ui/button.tsx` (variants: default, destructive, outline, secondary, ghost, link; sizes: default, sm, lg, icon)
- `frontend/src/components/ui/badge.tsx` (variants: default, secondary, destructive, outline)
- `frontend/src/components/ui/dialog.tsx` (Dialog, DialogTrigger, DialogContent, DialogHeader, DialogTitle, DialogDescription)
- `frontend/src/components/ui/input.tsx`
- `frontend/src/components/ui/scroll-area.tsx` (ScrollArea, ScrollBar)
- `frontend/src/components/ui/tabs.tsx` (Tabs, TabsList, TabsTrigger, TabsContent)

Use `npx shadcn@latest add button badge dialog input scroll-area tabs` to generate, or write manually following shadcn conventions (Radix primitives + cn utility + forwardRef + displayName).

- [ ] **Step 6: Verify dark theme renders**

Add a test button in `main.tsx`:

```tsx
// temporary, remove after verify
import { Button } from '@/components/ui/button'
// render: <Button variant="default">Test</Button>
```

Expected: Dark-themed button renders correctly in browser.

- [ ] **Step 7: Commit**

```bash
git add frontend/
git commit -m "feat(frontend): add Tailwind CSS v4 + shadcn/ui with dark theme"
```

### Task 1.3: Wire protocol types

**Files:**
- Create: `frontend/src/types/index.ts`
- Create: `frontend/src/lib/protocol.ts`

**Interfaces:**
- Produces: All wire types matching `crates/vol-llm-ui/src/state/mod.rs` and `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Create types/index.ts with core enums and display types**

```typescript
// frontend/src/types/index.ts

// Tab routing
export type ActiveTab = 'tasks' | 'agents' | 'tools' | 'workspace' | 'skills' | 'mcp' | 'logs'
export type AgentSubTab = 'conversation' | 'sessions' | 'context' | 'tasks'
export type McpSubtab = 'servers' | 'tools' | 'resources' | 'prompts'
export type ConnectionState = 'connecting' | 'connected' | 'disconnected'
export type ServerType = 'ControlPlane' | 'DataPlane' | 'Unknown'

// Tool call status
export type ToolCallStatus = 'Running' | 'Success' | 'Error' | 'Skipped'

export interface ToolCallEntry {
  sequence: number
  toolName: string
  argPreview: string
  status: ToolCallStatus
  durationMs: number | null
}

// Conversation entries
export type ConversationEntry =
  | { type: 'UserInput'; text: string }
  | { type: 'Thinking'; content: string }
  | { type: 'ContentStreaming'; content: string }
  | { type: 'ToolCall'; toolName: string; argPreview: string; fullArguments: string }
  | { type: 'ToolResult'; toolName: string; preview: string; fullResult: string; success: boolean }
  | { type: 'AgentAnswer'; text: string }
  | { type: 'RunSummary'; iterations: number; toolCalls: number; elapsedMs: number }
  | { type: 'EntryCheckpoint'; reason: string; note: string | null; createdAt: number }
  | { type: 'Error'; message: string }
  | { type: 'RunningBanner'; runId: string }

export interface AgentConversation {
  entries: ConversationEntry[]
  autoScroll: boolean
}

// Agent list
export interface AgentListEntry {
  id: string
  name: string
  type: string
  description: string
  scope: string
  status?: string
  node_id?: string
  ws_url?: string
}

// Node types
export interface NodeLoad { running: number; queued: number }
export interface NodeListEntry {
  node_id: string
  name: string
  version: string
  status: string
  last_seen_at_ms?: number
  capability_revision: number
  load: NodeLoad
  agent_count?: number
  ws_url?: string
}

// RPC response types
export interface ConnectedInfo { server_type: ServerType; version: string; capabilities: string[] }
export interface SkillDetail {
  name: string; version: string; scope: string; description: string
  triggers: string[]; content: string; file_listing: string[]; directory: string
}
export interface SkillListEntry {
  id: string; name: string; version: string; scope: string; description: string; triggers: string[]
}
export interface McpServerInfo { name: string; status: string }
export interface McpToolInfo { server: string; name: string; description?: string; input_schema?: unknown }
export interface McpResourceInfo { server: string; name: string; uri: string; mime_type?: string; description?: string }
export interface McpResourceTemplateInfo { server: string; name: string; uri_template: string; description?: string }
export interface McpPromptInfo { server: string; name: string; description?: string; arguments?: McpPromptArgInfo[] }
export interface McpPromptArgInfo { name: string; description?: string; required: boolean }
export interface TaskEntry {
  id: number; status: string; kind: string; publisher: string; assignee: string
  subject: string; description: string; active_form: string
  dependencies: number[]; blocks: number[]
  created_at: string; started_at?: string; completed_at?: string
}
export interface SessionListEntry { id: string; entry_count: number; created_at: number }
export interface LogRunSummary { run_id: string; event_count: number; last_event: string; last_event_time: string }
export interface LogLine { timestamp: string; event_type: string; summary: string }
export interface FileEntry { name: string; is_dir: boolean; size: number }
export interface ProviderOption { name: string; models: string[] }

// Capability state
export interface CapabilityOverlayState {
  effective_tools: string[]; effective_skills: string[]; effective_mcp_servers: string[]
  available_tools: unknown[]; available_skills: unknown[]; available_mcp_servers: unknown[]
  base_tools: string[]; base_skills: string[]; base_mcp_servers: string[]
  loading: boolean; dirty: boolean
}

export type ToggleSavingState = { kind: 'saving' } | { kind: 'saved' } | { kind: 'error'; message: string }
```

- [ ] **Step 2: Create lib/protocol.ts with UiEvent and RPC signatures**

```typescript
// frontend/src/lib/protocol.ts
import type {
  AgentListEntry, ConnectedInfo, FileEntry, LogRunSummary, LogLine,
  McpPromptInfo, McpResourceInfo, McpResourceTemplateInfo, McpServerInfo, McpToolInfo,
  NodeListEntry, SessionListEntry, SkillDetail, SkillListEntry, TaskEntry
} from '@/types'

// UiEvent — discriminated union keyed by "type" field (matches Rust #[serde(tag = "type")])
export type UiEvent =
  | { type: 'agent_start'; run_id: string; input: string }
  | { type: 'agent_complete'; run_id: string; response: string }
  | { type: 'agent_aborted'; run_id: string; reason: string }
  | { type: 'agent_error'; run_id: string; message: string }
  | { type: 'thinking_start' }
  | { type: 'thinking_delta'; delta: string }
  | { type: 'thinking_complete' }
  | { type: 'content_start' }
  | { type: 'content_delta'; delta: string }
  | { type: 'content_complete'; content: string }
  | { type: 'tool_call_begin'; tool_name: string; arguments: string }
  | { type: 'tool_call_argument_delta'; delta: string }
  | { type: 'tool_call_complete'; tool_name: string; result: string; duration_ms?: number }
  | { type: 'tool_call_error'; tool_name: string; error: string; duration_ms?: number }
  | { type: 'tool_call_skipped'; tool_name: string; reason: string; duration_ms?: number }
  | { type: 'max_iterations_reached'; current: number; max: number }
  | { type: 'iteration_continued'; from_iteration: number }
  | { type: 'iteration_complete'; iteration: number; final_answer?: string }
  | { type: 'approval_request'; tool_name: string; reason: string; arguments: string }
  | { type: 'approval_resolved'; approved: boolean }
  | { type: 'ws_connected' } | { type: 'ws_connecting' }
  | { type: 'ws_disconnected'; reason?: string }
  | { type: 'ws_reconnecting'; attempt: number; delay_secs: number }
  | { type: 'ws_reconnect_failed' } | { type: 'ws_reconnected' }

// AgentStreamEvent — externally tagged from server ({"VariantName": {...fields}})
export type AgentStreamEvent = Record<string, unknown>

export interface AgentEvent {
  run_id: string
  event: AgentStreamEvent
}

// All RPC method signatures with parameter and return types
export interface RpcMethods {
  'agent.submit': { params: { input: string; target?: string }; result: string }
  'agent.approve': { params: { req_id: string; approved: boolean; reason?: string }; result: null }
  'agent.cancel': { params: { run_id: string }; result: null }
  'agent.list': { params: { node_id?: string }; result: AgentListEntry[] }
  'agent.status': { params: { agent_id: string }; result: { status: string; run_id?: string } }
  'agent.get_capabilities': { params: { agent_id: string; session_id: string }; result: GetCapabilitiesResult }
  'agent.update_capabilities': { params: { agent_id: string; session_id: string; effective_tools: string[]; effective_skills: string[]; effective_mcp_servers: string[] }; result: UpdateCapabilitiesResult }
  'agent.context_config': { params: { agent_id: string }; result: { contributors: ContributorInfo[] } }
  'agent.context_snapshot': { params: { agent_id: string; contributor: string }; result: { messages: ContextMessage[] } }
  'session.list': { params: { agent_id?: string }; result: SessionListEntry[] }
  'session.entries': { params: { session_id: string }; result: SessionEntry[] }
  'session.resume': { params: { session_id: string; agent_id?: string }; result: null }
  'file.list': { params: { path: string }; result: FileEntry[] }
  'file.read': { params: { path: string }; result: string }
  'tool.list': { params: {}; result: ToolDef[] }
  'tool.call': { params: { tool_name: string; arguments: Record<string, unknown> }; result: string }
  'skill.list': { params: {}; result: SkillListEntry[] }
  'skill.get': { params: { name: string }; result: SkillDetail }
  'skill.refresh': { params: {}; result: null }
  'mcp.list_servers': { params: {}; result: McpServerInfo[] }
  'mcp.list_tools': { params: { server?: string }; result: McpToolInfo[] }
  'mcp.list_resources': { params: {}; result: McpResourceInfo[] }
  'mcp.list_resource_templates': { params: {}; result: McpResourceTemplateInfo[] }
  'mcp.list_prompts': { params: {}; result: McpPromptInfo[] }
  'mcp.read_resource': { params: { uri: string }; result: string }
  'mcp.call_tool': { params: { server: string; tool_name: string; arguments: Record<string, unknown> }; result: string }
  'mcp.reconnect': { params: { server: string }; result: null }
  'mcp.get_prompt': { params: { server: string; prompt_name: string; arguments: Record<string, unknown> }; result: string }
  'task.list': { params: { status?: string; assignee?: string }; result: TaskEntry[] }
  'task.get': { params: { task_id: number }; result: TaskEntry }
  'log.list': { params: {}; result: LogRunSummary[] }
  'log.read': { params: { run_id: string }; result: LogLine[] }
  'system.connected': { params: {}; result: ConnectedInfo }
  'control.node_list': { params: {}; result: NodeListEntry[] }
  'control.node_get': { params: { node_id: string }; result: NodeListEntry }
  'control.capability_list': { params: { node_id: string }; result: CapabilityListResult }
}

// Supporting types for RPC results
export interface GetCapabilitiesResult {
  effective_tools: string[]; effective_skills: string[]; effective_mcp_servers: string[]
  available_tools: unknown[]; available_skills: unknown[]; available_mcp_servers: unknown[]
  base_tools: string[]; base_skills: string[]; base_mcp_servers: string[]
  providers?: { name: string; models: string[] }[]
  selected_provider?: string; selected_model?: string
}
export interface UpdateCapabilitiesResult extends GetCapabilitiesResult {}
export interface ContributorInfo { name: string; anchor_zone: string; position: number; estimated_tokens: number; message_count: number }
export interface ContextMessage { role: string; content: string }
export interface SessionEntry { id: string; session_id: string; created_at: string; parent_id?: string; type: string; data: unknown }
export interface ToolDef { name: string; description: string; parameters?: unknown }
export interface CapabilityListResult { node_id: string; revision: number; agents: unknown[]; tools: unknown[]; mcp_servers: unknown[]; skills: unknown[] }
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/types/ frontend/src/lib/protocol.ts
git commit -m "feat(frontend): define all wire protocol types and RPC signatures"
```

### Task 1.4: JsonRpcClient

**Files:**
- Create: `frontend/src/lib/jsonrpc-client.ts`
- Test: `frontend/tests/unit/jsonrpc-client.test.ts`

**Interfaces:**
- Produces: `JsonRpcClient` class with `call<T>(method, params?)`, `eventStream()`, `onStateChange(cb)`, `reconnect()`

- [ ] **Step 1: Write the test for request/response**

```typescript
// frontend/tests/unit/jsonrpc-client.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock WebSocket
class MockWebSocket {
  onopen: (() => void) | null = null
  onmessage: ((e: { data: string }) => void) | null = null
  onclose: ((e: { code: number }) => void) | null = null
  onerror: (() => void) | null = null
  readyState = 0 // CONNECTING
  sent: string[] = []
  static readonly CONNECTING = 0
  static readonly OPEN = 1
  static readonly CLOSING = 2
  static readonly CLOSED = 3

  constructor(public url: string) {
    setTimeout(() => { this.readyState = 1; this.onopen?.() }, 0)
  }
  send(data: string) { this.sent.push(data) }
  close() { this.readyState = 3; this.onclose?.({ code: 1000 }) }
  // helper to simulate receiving a message
  receive(data: object) { this.onmessage?.({ data: JSON.stringify(data) }) }
}

// Replace global WebSocket
vi.stubGlobal('WebSocket', MockWebSocket)

// We need to dynamic-import the module after the mock is in place
async function importClient() {
  return import('@/lib/jsonrpc-client')
}

describe('JsonRpcClient', () => {
  it('connects and invokes state change callback', async () => {
    const { JsonRpcClient } = await importClient()
    const states: string[] = []
    const client = new JsonRpcClient('ws://test/ws')
    client.onStateChange((s) => states.push(s))

    await new Promise(r => setTimeout(r, 10))
    expect(states).toContain('connecting')
    expect(states).toContain('connected')
  })

  it('sends JSON-RPC request and resolves with result', async () => {
    const { JsonRpcClient } = await importClient()
    const client = new JsonRpcClient('ws://test/ws', { autoSubscribe: false })
    await new Promise(r => setTimeout(r, 10))

    const resultPromise = client.call<{ name: string }>('agent.list', { node_id: 'n1' })
    // Simulate server response (id:1 because first call)
    const ws = (client as any).ws as MockWebSocket
    ws.receive({ jsonrpc: '2.0', id: 1, result: [{ name: 'test-agent' }] })

    const result = await resultPromise
    expect(result).toEqual([{ name: 'test-agent' }])
    expect(ws.sent.length).toBe(1)
    const sent = JSON.parse(ws.sent[0])
    expect(sent.method).toBe('agent.list')
    expect(sent.params).toEqual({ node_id: 'n1' })
    expect(sent.id).toBe(1)
    expect(sent.jsonrpc).toBe('2.0')
  })

  it('handles error responses', async () => {
    const { JsonRpcClient } = await importClient()
    const client = new JsonRpcClient('ws://test/ws', { autoSubscribe: false })
    await new Promise(r => setTimeout(r, 10))

    const resultPromise = client.call('agent.list', {})
    const ws = (client as any).ws as MockWebSocket
    ws.receive({ jsonrpc: '2.0', id: 1, error: { code: -1, message: 'Not found' } })

    await expect(resultPromise).rejects.toEqual({ code: -1, message: 'Not found' })
  })

  it('routes notifications to event stream', async () => {
    const { JsonRpcClient } = await importClient()
    const client = new JsonRpcClient('ws://test/ws', { autoSubscribe: false })
    await new Promise(r => setTimeout(r, 10))

    const ws = (client as any).ws as MockWebSocket
    ws.receive({ jsonrpc: '2.0', method: 'agent.event', params: { run_id: 'r1', event: { AgentStart: { input: 'hello' } } } })

    // Read from event stream
    const iterator = client.eventStream()[Symbol.asyncIterator]()
    const { value } = await iterator.next()
    expect(value).toEqual({ run_id: 'r1', event: { AgentStart: { input: 'hello' } } })
  })

  it('reconnect creates new WebSocket', async () => {
    const { JsonRpcClient } = await importClient()
    const client = new JsonRpcClient('ws://test/ws', { autoSubscribe: false })
    await new Promise(r => setTimeout(r, 10))
    const oldWs = (client as any).ws

    client.reconnect()
    await new Promise(r => setTimeout(r, 10))
    expect((client as any).ws).not.toBe(oldWs)
  })
})
```

- [ ] **Step 2: Run test — verify failure**

```bash
cd frontend && npx vitest run tests/unit/jsonrpc-client.test.ts
```
Expected: FAIL — module not found.

- [ ] **Step 3: Implement JsonRpcClient**

```typescript
// frontend/src/lib/jsonrpc-client.ts
import type { ConnectionState } from '@/types'
import type { AgentEvent } from './protocol'

type ResponseCallback = (result: unknown) => void
type ErrorCallback = (error: { code: number; message: string }) => void

interface PendingCall {
  resolve: ResponseCallback
  reject: ErrorCallback
}

export class JsonRpcClient {
  private ws: WebSocket | null = null
  private url: string
  private nextId = 1
  private pending = new Map<number, PendingCall>()
  private stateChangeCallback: ((state: ConnectionState) => void) | null = null
  private autoSubscribe: boolean
  private sendQueue: string[] = []

  // Event stream: push-based via callbacks stored by consumers
  private eventListeners: Array<(event: AgentEvent) => void> = []

  constructor(url: string, opts?: { autoSubscribe?: boolean }) {
    this.url = url
    this.autoSubscribe = opts?.autoSubscribe ?? true
    this.connect()
  }

  private connect(): void {
    this.stateChangeCallback?.('connecting')
    const ws = new WebSocket(this.url)
    this.ws = ws

    ws.onopen = () => {
      this.stateChangeCallback?.('connected')
      // Flush send queue
      for (const msg of this.sendQueue) { ws.send(msg) }
      this.sendQueue = []
      // Auto-subscribe to agent events
      if (this.autoSubscribe) {
        this.call('agent.subscribe').catch(() => {})
      }
    }

    ws.onmessage = (e: MessageEvent) => {
      const msg = JSON.parse(e.data as string)
      if (msg.id != null && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id)!
        this.pending.delete(msg.id)
        if (msg.error) {
          reject(msg.error)
        } else {
          resolve(msg.result)
        }
      } else if (msg.method === 'agent.event' && msg.params) {
        const event: AgentEvent = msg.params
        for (const listener of this.eventListeners) {
          listener(event)
        }
      }
    }

    ws.onclose = () => {
      this.stateChangeCallback?.('disconnected')
      // Fail all pending calls
      for (const [id, { reject }] of this.pending) {
        reject({ code: -1, message: 'WebSocket disconnected' })
      }
      this.pending.clear()
    }

    ws.onerror = () => { /* onclose will fire after this */ }
  }

  call<T>(method: string, params?: unknown): Promise<T> {
    return new Promise((resolve, reject) => {
      const id = this.nextId++
      const request = { jsonrpc: '2.0', method, params: params ?? {}, id }
      const message = JSON.stringify(request)

      this.pending.set(id, { resolve: resolve as ResponseCallback, reject })

      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(message)
      } else {
        this.sendQueue.push(message)
      }
    })
  }

  onStateChange(cb: (state: ConnectionState) => void): void {
    this.stateChangeCallback = cb
  }

  reconnect(): void {
    if (this.ws) {
      this.ws.onclose = null // prevent disconnect callback firing
      this.ws.close()
    }
    this.connect()
  }

  onEvent(listener: (event: AgentEvent) => void): () => void {
    this.eventListeners.push(listener)
    return () => {
      this.eventListeners = this.eventListeners.filter(l => l !== listener)
    }
  }
}
```

- [ ] **Step 4: Run test — verify pass**

```bash
cd frontend && npx vitest run tests/unit/jsonrpc-client.test.ts
```
Expected: 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/jsonrpc-client.ts frontend/tests/
git commit -m "feat(frontend): implement JsonRpcClient with request/response and event stream"
```

### Task 1.5: DpConnectionPool + Reconnect

**Files:**
- Create: `frontend/src/lib/dp-pool.ts`
- Create: `frontend/src/lib/reconnect.ts`
- Test: `frontend/tests/unit/dp-pool.test.ts`
- Test: `frontend/tests/unit/reconnect.test.ts`

**Interfaces:**
- Produces: `DpConnectionPool` class, `attemptReconnect()` function
- Consumes: `JsonRpcClient` from Task 1.4

- [ ] **Step 1: Write dp-pool.test.ts**

```typescript
// frontend/tests/unit/dp-pool.test.ts
import { describe, it, expect, vi } from 'vitest'

// Mock JsonRpcClient
vi.mock('@/lib/jsonrpc-client', () => ({
  JsonRpcClient: vi.fn().mockImplementation((url: string) => ({
    url,
    call: vi.fn().mockResolvedValue(null),
    onStateChange: vi.fn(),
    onEvent: vi.fn(() => () => {}),
    reconnect: vi.fn(),
  }))
}))

import { DpConnectionPool } from '@/lib/dp-pool'
import { JsonRpcClient } from '@/lib/jsonrpc-client'

describe('DpConnectionPool', () => {
  it('getOrCreate lazily creates connection for new node', () => {
    const pool = new DpConnectionPool()
    const client = pool.getOrCreate('node1', 'ws://n1/ws')

    expect(JsonRpcClient).toHaveBeenCalledWith('ws://n1/ws')
    expect(client).toBeDefined()
  })

  it('getOrCreate returns existing connection for same node', () => {
    const pool = new DpConnectionPool()
    const c1 = pool.getOrCreate('node1', 'ws://n1/ws')
    const c2 = pool.getOrCreate('node1', 'ws://n1/ws')

    expect(c1).toBe(c2)
    expect(JsonRpcClient).toHaveBeenCalledTimes(1)
  })

  it('get returns undefined for unknown node', () => {
    const pool = new DpConnectionPool()
    expect(pool.get('unknown')).toBeUndefined()
  })

  it('connections iterates all entries', () => {
    const pool = new DpConnectionPool()
    pool.getOrCreate('n1', 'ws://n1/ws')
    pool.getOrCreate('n2', 'ws://n2/ws')

    const entries = pool.connections()
    expect(entries.length).toBe(2)
  })
})
```

- [ ] **Step 2: Implement DpConnectionPool**

```typescript
// frontend/src/lib/dp-pool.ts
import { JsonRpcClient } from './jsonrpc-client'

export interface DpConnection {
  client: JsonRpcClient
  nodeId: string
  wsUrl: string
  agentIds: string[]
}

export class DpConnectionPool {
  private connections = new Map<string, DpConnection>()

  getOrCreate(nodeId: string, wsUrl: string, agentIds: string[] = []): JsonRpcClient {
    let entry = this.connections.get(nodeId)
    if (!entry) {
      const client = new JsonRpcClient(wsUrl)
      entry = { client, nodeId, wsUrl, agentIds }
      this.connections.set(nodeId, entry)
    }
    return entry.client
  }

  get(nodeId: string): JsonRpcClient | undefined {
    return this.connections.get(nodeId)?.client
  }

  connections(): DpConnection[] {
    return Array.from(this.connections.values())
  }
}
```

- [ ] **Step 3: Write reconnect.test.ts**

```typescript
// frontend/tests/unit/reconnect.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest'

// We need a controllable client mock
function createMockClient(isConnected: () => boolean) {
  return {
    reconnect: vi.fn(),
    call: vi.fn().mockResolvedValue(null),
    onStateChange: vi.fn(),
    onEvent: vi.fn(() => () => {}),
    _connected: isConnected,
  }
}

import { attemptReconnect } from '@/lib/reconnect'

describe('attemptReconnect', () => {
  beforeEach(() => { vi.useFakeTimers() })

  it('resolves true immediately if client is already connected', async () => {
    const client = createMockClient(() => true)
    const onAttempt = vi.fn()

    const resultPromise = attemptReconnect(client as any, onAttempt)
    // Fast-forward past any timers
    await vi.runAllTimersAsync()

    const result = await resultPromise
    expect(result).toBe(true)
    expect(onAttempt).not.toHaveBeenCalled()
  })

  it('tries up to 10 attempts with exponential backoff then fails', async () => {
    const client = createMockClient(() => false)
    const onAttempt = vi.fn()

    const resultPromise = attemptReconnect(client as any, onAttempt)

    // The function checks connected state after each reconnect call
    // Fast-forward all 10 attempts
    for (let i = 0; i < 10; i++) {
      await vi.advanceTimersByTimeAsync(1000) // wait for delay
      await vi.runAllTimersAsync()
    }

    const result = await resultPromise
    expect(result).toBe(false)
    expect(onAttempt).toHaveBeenCalledTimes(10)
    // Verify delays: 3,6,12,24,30,30,30,30,30,30
    expect(onAttempt.mock.calls[0][1]).toBe(3)
    expect(onAttempt.mock.calls[3][1]).toBe(24)
    expect(onAttempt.mock.calls[4][1]).toBe(30)
    expect(onAttempt.mock.calls[9][1]).toBe(30)
  })
})
```

- [ ] **Step 4: Implement reconnect.ts**

```typescript
// frontend/src/lib/reconnect.ts
const MAX_ATTEMPTS = 10
const MIN_DELAY = 3
const MAX_DELAY = 30

function delay(seconds: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, seconds * 1000))
}

export async function attemptReconnect(
  reconnectFn: () => void,
  isConnected: () => boolean,
  onAttempt: (attempt: number, delaySecs: number) => void,
): Promise<boolean> {
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
    const delaySecs = Math.min(MIN_DELAY * Math.pow(2, attempt - 1), MAX_DELAY)
    onAttempt(attempt, delaySecs)

    await delay(delaySecs)

    if (isConnected()) return true

    reconnectFn()

    // Wait briefly for connection to establish
    await delay(1)
    if (isConnected()) return true
  }

  return false
}
```

- [ ] **Step 5: Run tests**

```bash
cd frontend && npx vitest run tests/unit/dp-pool.test.ts tests/unit/reconnect.test.ts
```
Expected: All tests PASS.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/lib/dp-pool.ts frontend/src/lib/reconnect.ts frontend/tests/unit/
git commit -m "feat(frontend): implement DpConnectionPool and exponential backoff reconnect"
```

### Task 1.6: Core Jotai stores (connection + ui)

**Files:**
- Create: `frontend/src/stores/connection.ts`
- Create: `frontend/src/stores/ui.ts`

**Interfaces:**
- Produces: `wsConnectedAtom`, `serverModeAtom`, `reconnectStateAtom`, `activeTabAtom`, `globalRunStateAtom`
- Consumes: `JsonRpcClient` from Task 1.4

- [ ] **Step 1: Implement stores/connection.ts**

```typescript
// frontend/src/stores/connection.ts
import { atom } from 'jotai'
import type { ConnectionState, ServerType } from '@/types'

export const wsConnectedAtom = atom(false)
export const connectionStateAtom = atom<ConnectionState>('disconnected')
export const serverModeAtom = atom<ServerType>('Unknown')
export const wsUrlAtom = atom('')
export const wsLastErrorAtom = atom<string | null>(null)

// Session + run metrics
export const sessionIdAtom = atom('web-session')
export const runCountAtom = atom(0)
export const iterationAtom = atom(0)
export const toolCallCountAtom = atom(0)
export const runElapsedAtom = atom(0)  // ms
export const isRunningAtom = atom(false)
export const unsafeModeAtom = atom(false)
export const exitingAtom = atom(false)

// Per-agent running state
export const runningAgentsAtom = atom<Set<string>>(new Set())
export const runMapAtom = atom<Map<string, string>>(new Map()) // run_id → agent_id
export const pendingSubmitAgentAtom = atom<string | null>(null)
```

- [ ] **Step 2: Implement stores/ui.ts**

```typescript
// frontend/src/stores/ui.ts
import { atom } from 'jotai'
import type { ActiveTab } from '@/types'

export const activeTabAtom = atom<ActiveTab>('agents')
export const viewingNodeDetailAtom = atom<string | null>(null)
export const activeNodeIdAtom = atom<string | null>(null)
export const fileTreeDrawerOpenAtom = atom(false)
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/stores/
git commit -m "feat(frontend): create core Jotai stores for connection and UI state"
```

### Task 1.7: App Shell — StatusBar + TabBar + TabContent + FileTree skeleton

**Files:**
- Create: `frontend/src/App.tsx`
- Create: `frontend/src/components/layout/StatusBar.tsx`
- Create: `frontend/src/components/layout/TabBar.tsx`
- Create: `frontend/src/components/layout/TabContent.tsx`
- Create: `frontend/src/components/shared/ConnectionIndicator.tsx`
- Create: `frontend/src/components/panels/FileTree.tsx` (static skeleton)
- Create: `frontend/src/lib/ws-url.ts`

**Interfaces:**
- Consumes: Core stores from Task 1.6, `JsonRpcClient` from Task 1.4
- Produces: Visual App shell with StatusBar, tab switching, FileTree placeholder

- [ ] **Step 1: Create ws-url helper**

```typescript
// frontend/src/lib/ws-url.ts
export function deriveWsUrl(): string {
  if (typeof window === 'undefined') return 'ws://localhost:3001/ws'
  const hostname = window.location.hostname
  if (hostname === 'localhost' || hostname === '127.0.0.1') {
    return 'ws://localhost:3001/ws'
  }
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}/ws`
}
```

- [ ] **Step 2: Create ConnectionIndicator**

```tsx
// frontend/src/components/shared/ConnectionIndicator.tsx
import { useAtomValue } from 'jotai'
import { connectionStateAtom, wsLastErrorAtom } from '@/stores/connection'

export function ConnectionIndicator() {
  const state = useAtomValue(connectionStateAtom)
  const error = useAtomValue(wsLastErrorAtom)

  const dotColor = state === 'connected' ? '#40c040' :
    state === 'connecting' ? '#f0c040' : '#c04040'

  const label = state === 'connected' ? 'Connected' :
    state === 'connecting' ? 'Connecting...' :
    error ? `Error: ${error}` : 'No connection'

  return (
    <span className="flex items-center gap-1 mr-1">
      <span
        className="w-2 h-2 rounded-full inline-block flex-shrink-0"
        style={{ backgroundColor: dotColor, boxShadow: `0 0 4px ${dotColor}` }}
      />
      <span className="text-[11px] text-[#888] hidden sm:inline">{label}</span>
    </span>
  )
}
```

- [ ] **Step 3: Create StatusBar**

```tsx
// frontend/src/components/layout/StatusBar.tsx
import { useAtomValue } from 'jotai'
import { ConnectionIndicator } from '@/components/shared/ConnectionIndicator'
import {
  sessionIdAtom, runCountAtom, iterationAtom, toolCallCountAtom,
  isRunningAtom, exitingAtom, unsafeModeAtom, runElapsedAtom,
} from '@/stores/connection'

function formatElapsed(ms: number): string {
  const secs = Math.floor(ms / 1000)
  return `${String(Math.floor(secs / 60)).padStart(2, '0')}:${String(secs % 60).padStart(2, '0')}`
}

export function StatusBar() {
  const sessionId = useAtomValue(sessionIdAtom)
  const runCount = useAtomValue(runCountAtom)
  const iteration = useAtomValue(iterationAtom)
  const toolCallCount = useAtomValue(toolCallCountAtom)
  const isRunning = useAtomValue(isRunningAtom)
  const elapsed = useAtomValue(runElapsedAtom)
  const exiting = useAtomValue(exitingAtom)
  const unsafeMode = useAtomValue(unsafeModeAtom)

  const statusLabel = isRunning ? 'Running' : 'Idle'
  const statusCls = isRunning
    ? 'flex items-center justify-between px-3 py-1 bg-[#2d2d44] text-[12px] font-mono flex-shrink-0 text-[#f0c040]'
    : 'flex items-center justify-between px-3 py-1 bg-[#2d2d44] text-[12px] font-mono flex-shrink-0 text-[#80c080]'

  return (
    <div className={statusCls}>
      <div className="flex items-center gap-1.5 overflow-hidden flex-nowrap sm:gap-1">
        <ConnectionIndicator />
        <span className="text-[#888] text-[11px] hidden sm:inline">Session: {sessionId.slice(0, 8)}</span>
        <span className="text-[#888] text-[11px]">Run: {runCount}</span>
        <span className="text-[#888] text-[11px]">Iter: {iteration}</span>
        <span className="text-[#888] text-[11px]">Tools: {toolCallCount}</span>
        <span className="text-[#888] text-[11px]">Time: {formatElapsed(elapsed)}</span>
        {isRunning && <span className="px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#3a3a20] text-[#f0c040]">{statusLabel}</span>}
        {!isRunning && <span className="px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#203a20] text-[#80c080]">{statusLabel}</span>}
        {unsafeMode && <span className="px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#3a2020] text-[#ff4040]">!! UNSAFE</span>}
        {exiting && <span className="px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#3a2020] text-[#ff4040]">QUITTING</span>}
      </div>
      <div className="flex items-center gap-1 text-[11px] text-[#888]">
        <span>UI: {__BUILD_TIME__}</span>
      </div>
    </div>
  )
}
```

Note: `__BUILD_TIME__` should be defined in `vite.config.ts` via `define: { __BUILD_TIME__: JSON.stringify(new Date().toISOString()) }`.

- [ ] **Step 4: Create TabBar**

```tsx
// frontend/src/components/layout/TabBar.tsx
import { useAtom } from 'jotai'
import { activeTabAtom } from '@/stores/ui'
import type { ActiveTab } from '@/types'
import { cn } from '@/lib/utils'

const TABS: { id: ActiveTab; label: string }[] = [
  { id: 'tasks', label: 'Tasks' },
  { id: 'agents', label: 'Agents' },
  { id: 'tools', label: 'Tools' },
  { id: 'workspace', label: 'Workspace' },
  { id: 'skills', label: 'Skills' },
  { id: 'mcp', label: 'MCP' },
  { id: 'logs', label: 'Logs' },
]

export function TabBar() {
  const [active, setActive] = useAtom(activeTabAtom)

  return (
    <div className="flex flex-nowrap bg-[#252540] border-b border-[#333355] flex-shrink-0 overflow-x-auto">
      {TABS.map(tab => (
        <button
          key={tab.id}
          onClick={() => setActive(tab.id)}
          className={cn(
            'px-2 sm:px-4 py-1 sm:py-1.5 cursor-pointer text-[11px] sm:text-[13px] whitespace-nowrap flex-shrink-0 border-b-2',
            active === tab.id
              ? 'bg-[#1a1a2e] text-[#e0e0e0] border-[#80a0ff]'
              : 'bg-transparent text-[#888] border-transparent hover:text-[#ccc] hover:bg-[#2a2a44]'
          )}
        >
          {tab.label}
        </button>
      ))}
    </div>
  )
}
```

- [ ] **Step 5: Create TabContent with placeholder panels**

```tsx
// frontend/src/components/layout/TabContent.tsx
import { useAtomValue } from 'jotai'
import { activeTabAtom } from '@/stores/ui'

function PlaceholderPanel({ name }: { name: string }) {
  return (
    <div className="flex items-center justify-center h-full text-[#666] text-sm">
      {name} — coming soon
    </div>
  )
}

export function TabContent() {
  const active = useAtomValue(activeTabAtom)

  switch (active) {
    case 'tasks': return <PlaceholderPanel name="Tasks" />
    case 'agents': return <PlaceholderPanel name="Agents" />
    case 'tools': return <PlaceholderPanel name="Tools" />
    case 'workspace': return <PlaceholderPanel name="Workspace" />
    case 'skills': return <PlaceholderPanel name="Skills" />
    case 'mcp': return <PlaceholderPanel name="MCP" />
    case 'logs': return <PlaceholderPanel name="Logs" />
    default: return <PlaceholderPanel name="Agents" />
  }
}
```

- [ ] **Step 6: Create FileTree skeleton**

```tsx
// frontend/src/components/panels/FileTree.tsx
export function FileTree() {
  return (
    <div className="hidden md:block w-[240px] flex-shrink-0 bg-[#1e1e32] border-r border-[#333355] overflow-y-auto p-2.5">
      <div className="text-[#666] text-xs font-mono">Workspace files</div>
    </div>
  )
}
```

- [ ] **Step 7: Create App.tsx with WS connection and provider tree**

```tsx
// frontend/src/App.tsx
import { useEffect, useRef } from 'react'
import { useSetAtom } from 'jotai'
import { Provider } from 'jotai'
import { StatusBar } from '@/components/layout/StatusBar'
import { TabBar } from '@/components/layout/TabBar'
import { TabContent } from '@/components/layout/TabContent'
import { FileTree } from '@/components/panels/FileTree'
import { JsonRpcClient } from '@/lib/jsonrpc-client'
import { deriveWsUrl } from '@/lib/ws-url'
import { connectionStateAtom, serverModeAtom, wsUrlAtom } from '@/stores/connection'

function AppInner() {
  const setConnectionState = useSetAtom(connectionStateAtom)
  const setServerMode = useSetAtom(serverModeAtom)
  const setWsUrl = useSetAtom(wsUrlAtom)
  const clientRef = useRef<JsonRpcClient | null>(null)

  useEffect(() => {
    const url = deriveWsUrl()
    setWsUrl(url)
    const client = new JsonRpcClient(url)
    clientRef.current = client

    client.onStateChange((state) => {
      setConnectionState(state)
      if (state === 'connected') {
        client.call<{ server_type: string }>('system.connected').then(info => {
          setServerMode(info.server_type as any)
        }).catch(() => {})
      }
    })

    return () => { /* cleanup handled by browser */ }
  }, [])

  return (
    <div className="relative h-[100dvh] w-[100vw] font-[system-ui] text-[14px] text-[#e0e0e0] bg-[#1a1a2e]">
      <div className="flex flex-col h-full w-full overflow-hidden">
        <StatusBar />
        <div className="flex flex-1 overflow-hidden relative">
          <FileTree />
          <div className="min-w-0 flex-1 flex flex-col overflow-hidden">
            <TabBar />
            <TabContent />
          </div>
        </div>
      </div>
    </div>
  )
}

export function App() {
  return (
    <Provider>
      <AppInner />
    </Provider>
  )
}
```

- [ ] **Step 8: Update vite.config.ts with BUILD_TIME define**

```typescript
// Add to vite.config.ts
define: {
  __BUILD_TIME__: JSON.stringify(new Date().toISOString()),
},
```

- [ ] **Step 9: Verify app shell renders with tabs and dark theme**

Run: `cd frontend && npm run dev`
Expected: browser shows dark app with StatusBar (connection indicator shows "No connection"), TabBar (7 tabs clickable), FileTree sidebar placeholder.

- [ ] **Step 10: Commit**

```bash
git add frontend/src/App.tsx frontend/src/components/ frontend/src/lib/ws-url.ts frontend/vite.config.ts
git commit -m "feat(frontend): App shell with StatusBar, TabBar, TabContent, and FileTree skeleton"
```

---

## Phase 2: Agents + Conversation

### Task 2.1: Agents store + agents list store

**Files:**
- Create: `frontend/src/stores/agents.ts`
- Create: `frontend/src/stores/conversation.ts`
- Create: `frontend/src/stores/cache.ts`

**Interfaces:**
- Produces: `agentsAtom`, `selectedAgentIdAtom`, `agentsLoadingAtom`, `conversationByAgentFamily`, `activeAgentIdAtom`, `nodeDataCacheFamily`
- Consumes: `JsonRpcClient` from Task 1.4

- [ ] **Step 1: Create stores/agents.ts**

```typescript
// frontend/src/stores/agents.ts
import { atom } from 'jotai'
import type { AgentListEntry, AgentSubTab } from '@/types'

export const agentsAtom = atom<AgentListEntry[]>([])
export const selectedAgentIdAtom = atom<string | null>(null)
export const agentsLoadingAtom = atom(false)
export const agentsErrorAtom = atom<string | null>(null)
export const agentSubTabAtom = atom<AgentSubTab>('conversation')

// Per-agent status: { agentId: { status: 'idle'|'running', runId?: string } }
export const agentStatusMapAtom = atom<Record<string, { status: string; runId?: string }>>({})
```

- [ ] **Step 2: Create stores/conversation.ts**

```typescript
// frontend/src/stores/conversation.ts
import { atom } from 'jotai'
import type { AgentConversation } from '@/types'

// atomFamily equivalent: a derived atom that reads from a Map atom
export const conversationMapAtom = atom<Map<string, AgentConversation>>(new Map())
export const activeAgentIdAtom = atom<string | null>(null)

// Derived: get conversation for a specific agent
export const conversationByAgentAtom = atom((get) => {
  const agentId = get(activeAgentIdAtom)
  if (!agentId) return { entries: [], autoScroll: true } as AgentConversation
  return get(conversationMapAtom).get(agentId) ?? { entries: [], autoScroll: true }
})
```

- [ ] **Step 3: Create stores/cache.ts**

```typescript
// frontend/src/stores/cache.ts
import { atom } from 'jotai'

// Per-node JSON cache, keyed by [nodeId, cacheKey]
// e.g., cacheMap.get('{"nodeId":"n1","key":"tools"}') = serialized tools data
export const nodeDataCacheAtom = atom<Map<string, Map<string, unknown>>>(new Map())

export function getCacheKey(nodeId: string, key: string): string {
  return JSON.stringify({ nodeId, key })
}
```

- [ ] **Step 4: Commit**

```bash
git add frontend/src/stores/agents.ts frontend/src/stores/conversation.ts frontend/src/stores/cache.ts
git commit -m "feat(frontend): add agents, conversation, and cache Jotai stores"
```

### Task 2.2: Event stream loop (App level)

**Files:**
- Modify: `frontend/src/App.tsx`
- Create: `frontend/src/lib/event-handlers.ts`

**Interfaces:**
- Consumes: `JsonRpcClient.eventStream()`, Jotai stores
- Produces: Centralized event dispatch that writes atoms

- [ ] **Step 1: Create lib/event-handlers.ts — UiEvent → atom write logic**

```typescript
// frontend/src/lib/event-handlers.ts
import type { UiEvent } from './protocol'
import { getDefaultStore } from 'jotai'
import {
  runCountAtom, iterationAtom, toolCallCountAtom, isRunningAtom,
  runElapsedAtom, runningAgentsAtom, runMapAtom, pendingSubmitAgentAtom,
  sessionIdAtom,
} from '@/stores/connection'
import { agentStatusMapAtom } from '@/stores/agents'
import { conversationMapAtom, activeAgentIdAtom } from '@/stores/conversation'
import type { AgentConversation, ConversationEntry, ToolCallEntry } from '@/types'

const store = getDefaultStore()

// Look up owning agent for a run_id from runMap
function agentForRun(runId: string): string | undefined {
  return store.get(runMapAtom).get(runId)
}

// Helper: get or create conversation for agent
function getConversation(agentId: string): AgentConversation {
  const map = new Map(store.get(conversationMapAtom))
  let conv = map.get(agentId)
  if (!conv) {
    conv = { entries: [], autoScroll: true }
    map.set(agentId, conv)
    store.set(conversationMapAtom, map)
  }
  return conv
}

function updateConversation(agentId: string, fn: (conv: AgentConversation) => void) {
  const map = new Map(store.get(conversationMapAtom))
  const conv = map.get(agentId) ?? { entries: [], autoScroll: true }
  fn(conv)
  map.set(agentId, conv)
  store.set(conversationMapAtom, map)
}

let runStartTime = 0

export function handleUiEvent(event: UiEvent, runId: string) {
  switch (event.type) {
    case 'agent_start': {
      // Init run state
      store.set(runCountAtom, store.get(runCountAtom) + 1)
      store.set(iterationAtom, 0)
      store.set(toolCallCountAtom, 0)
      store.set(isRunningAtom, true)
      runStartTime = Date.now()

      // Attribute to agent
      const pendingAgent = store.get(pendingSubmitAgentAtom)
      if (pendingAgent) {
        const map = new Map(store.get(runMapAtom))
        map.set(runId, pendingAgent)
        store.set(runMapAtom, map)
        const agents = new Set(store.get(runningAgentsAtom))
        agents.add(pendingAgent)
        store.set(runningAgentsAtom, agents)
        store.set(pendingSubmitAgentAtom, null)

        // Set agent status
        const statusMap = { ...store.get(agentStatusMapAtom) }
        statusMap[pendingAgent] = { status: 'running', runId }
        store.set(agentStatusMapAtom, statusMap)

        // Append UserInput
        updateConversation(pendingAgent, conv => {
          conv.entries.push({ type: 'UserInput', text: event.input })
        })
      }
      break
    }

    case 'agent_complete':
    case 'agent_aborted':
    case 'agent_error': {
      store.set(isRunningAtom, false)
      store.set(runElapsedAtom, Date.now() - runStartTime)

      const agentId = agentForRun(runId)
      if (agentId) {
        const statusMap = { ...store.get(agentStatusMapAtom) }
        statusMap[agentId] = { status: 'idle' }
        store.set(agentStatusMapAtom, statusMap)

        const agents = new Set(store.get(runningAgentsAtom))
        agents.delete(agentId)
        store.set(runningAgentsAtom, agents)

        const map = new Map(store.get(runMapAtom))
        map.delete(runId)
        store.set(runMapAtom, map)
      }

      if (event.type === 'agent_aborted' && agentId) {
        updateConversation(agentId, conv => {
          conv.entries.push({ type: 'Error', message: event.reason })
        })
      }
      if (event.type === 'agent_error' && agentId) {
        updateConversation(agentId, conv => {
          conv.entries.push({ type: 'Error', message: event.message })
        })
      }
      break
    }

    case 'thinking_start': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({ type: 'Thinking', content: '' })
      })
      break
    }
    case 'thinking_delta': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const last = conv.entries[conv.entries.length - 1]
        if (last?.type === 'Thinking') {
          last.content += event.delta
        }
      })
      break
    }
    case 'thinking_complete': break // no-op

    case 'content_start': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({ type: 'ContentStreaming', content: '' })
      })
      break
    }
    case 'content_delta': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const last = conv.entries[conv.entries.length - 1]
        if (last?.type === 'ContentStreaming') {
          last.content += event.delta
        }
      })
      break
    }
    case 'content_complete': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const last = conv.entries[conv.entries.length - 1]
        if (last?.type === 'ContentStreaming') {
          conv.entries[conv.entries.length - 1] = {
            type: 'AgentAnswer',
            text: event.content
          }
        } else if (event.content) {
          conv.entries.push({ type: 'AgentAnswer', text: event.content })
        }
      })
      break
    }

    case 'tool_call_begin': {
      const seq = store.get(toolCallCountAtom) + 1
      store.set(toolCallCountAtom, seq)

      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const preview = formatToolArgs(event.arguments)
        conv.entries.push({
          type: 'ToolCall',
          toolName: event.tool_name,
          argPreview: preview,
          fullArguments: event.arguments,
        })
      })
      break
    }
    case 'tool_call_complete': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const preview = truncatePreview(event.result, 200)
        conv.entries.push({
          type: 'ToolResult',
          toolName: event.tool_name,
          preview,
          fullResult: event.result,
          success: true,
        })
      })
      break
    }
    case 'tool_call_error': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({
          type: 'ToolResult',
          toolName: event.tool_name,
          preview: event.error,
          fullResult: event.error,
          success: false,
        })
      })
      break
    }
    case 'tool_call_skipped': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({
          type: 'ToolResult',
          toolName: event.tool_name,
          preview: event.reason,
          fullResult: event.reason,
          success: false,
        })
      })
      break
    }

    case 'iteration_complete': {
      store.set(iterationAtom, event.iteration)
      break
    }
    case 'max_iterations_reached': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({
          type: 'Error',
          message: `Max iterations reached (${event.current}/${event.max}) — waiting for user decision...`
        })
      })
      break
    }
    case 'iteration_continued': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({
          type: 'AgentAnswer',
          text: `Continuing from iteration ${event.from_iteration} (counter reset to 0)`
        })
      })
      break
    }

    case 'approval_request':
    case 'approval_resolved':
    case 'ws_connected':
    case 'ws_connecting':
    case 'ws_disconnected':
    case 'ws_reconnecting':
    case 'ws_reconnect_failed':
    case 'ws_reconnected':
      // Handled at connection/approval dialog level
      break

    default: break
  }
}

// Helpers mirroring Rust state/mod.rs
export function formatToolArgs(arguments_: string): string {
  try {
    const parsed = JSON.parse(arguments_)
    if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
      const entries = Object.entries(parsed)
      if (entries.length === 0) return ''
      if (entries.length === 1) return jsonValueToDisplay(entries[0][1])
      return entries.map(([k, v]) => `${k}=${jsonValueToDisplay(v)}`).join(', ')
    }
    return jsonValueToDisplay(parsed)
  } catch { return arguments_ }
}

function jsonValueToDisplay(v: unknown): string {
  if (typeof v === 'string') return v
  if (typeof v === 'number' || typeof v === 'boolean') return String(v)
  if (v === null) return 'null'
  const s = JSON.stringify(v)
  return s.length > 60 ? s.slice(0, 57) + '…' : s
}

export function truncatePreview(s: string, maxChars: number): string {
  if (s.length <= maxChars) return s
  return s.slice(0, maxChars) + '...'
}
```

- [ ] **Step 2: Update App.tsx — add event stream loop**

Add to `AppInner` useEffect, after client creation:

```typescript
// After "clientRef.current = client"
const clientForEvents = client

// Spawn event stream consumer
let running = true
;(async () => {
  clientForEvents.onEvent((agentEvent) => {
    const rawEvent = agentEvent.event
    // Server sends externally-tagged: {"VariantName": {fields}}
    const entries = Object.entries(rawEvent)
    if (entries.length === 0) return
    const [variant, data] = entries[0]
    const uiEvent = agentEventToUiEvent(variant, data as Record<string, unknown>, agentEvent.run_id)
    if (uiEvent) {
      handleUiEvent(uiEvent, agentEvent.run_id)
    }
  })
})()

return () => { running = false }
```

- [ ] **Step 3: Add agentEventToUiEvent converter to event-handlers.ts**

```typescript
export function agentEventToUiEvent(
  variant: string,
  data: Record<string, unknown>,
  runId: string,
): UiEvent | null {
  const s = (k: string) => (data[k] as string) ?? ''
  const n = (k: string) => (data[k] as number)

  switch (variant) {
    case 'AgentStart': return { type: 'agent_start', run_id: runId, input: s('input') }
    case 'AgentComplete': return { type: 'agent_complete', run_id: runId, response: s('response') }
    case 'AgentAborted': return { type: 'agent_aborted', run_id: runId, reason: s('reason') }
    case 'ThinkingStart': return { type: 'thinking_start' }
    case 'ThinkingDelta': return { type: 'thinking_delta', delta: s('delta') }
    case 'ThinkingComplete': return { type: 'thinking_complete' }
    case 'ContentStart': return { type: 'content_start' }
    case 'ContentDelta': return { type: 'content_delta', delta: s('delta') }
    case 'ContentComplete': return { type: 'content_complete', content: s('content') }
    case 'ToolCallBegin': return { type: 'tool_call_begin', tool_name: s('tool_name'), arguments: s('arguments') }
    case 'ToolCallArgumentDelta': return { type: 'tool_call_argument_delta', delta: s('delta') }
    case 'ToolCallComplete': return { type: 'tool_call_complete', tool_name: s('tool_name'), result: s('result'), duration_ms: n('duration_ms') as number | undefined }
    case 'ToolCallError': return { type: 'tool_call_error', tool_name: s('tool_name'), error: s('error'), duration_ms: n('duration_ms') as number | undefined }
    case 'ToolCallSkipped': return { type: 'tool_call_skipped', tool_name: s('tool_name'), reason: s('reason'), duration_ms: n('duration_ms') as number | undefined }
    case 'MaxIterationsReached': return { type: 'max_iterations_reached', current: (n('current_iteration') ?? 0) as number, max: (n('max_iterations') ?? 0) as number }
    case 'IterationContinued': return { type: 'iteration_continued', from_iteration: (n('from_iteration') ?? 0) as number }
    case 'IterationComplete': return { type: 'iteration_complete', iteration: (n('iteration') ?? 0) as number, final_answer: s('final_answer') || undefined }
    default: return null
  }
}
```

- [ ] **Step 4: Update vite.config.ts — add global declare for BUILD_TIME**

```typescript
// frontend/src/vite-env.d.ts
/// <reference types="vite/client" />
declare const __BUILD_TIME__: string
```

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/event-handlers.ts frontend/src/App.tsx frontend/src/vite-env.d.ts
git commit -m "feat(frontend): event stream loop and UiEvent dispatch to Jotai atoms"
```

### Task 2.3: ConversationView + Markdown component

**Files:**
- Create: `frontend/src/components/panels/ConversationView.tsx`
- Create: `frontend/src/components/shared/Markdown.tsx`
- Create: `frontend/src/hooks/useAutoScroll.ts`
- Create: `frontend/src/hooks/useThrottledValue.ts`

**Interfaces:**
- Consumes: `conversationByAgentAtom` from Task 2.1, event handlers from Task 2.2
- Produces: Streaming conversation timeline with markdown rendering

- [ ] **Step 1: Install markdown dependencies**

```bash
cd frontend
npm install react-markdown remark-gfm rehype-sanitize rehype-highlight highlight.js
```

- [ ] **Step 2: Create useThrottledValue hook**

```typescript
// frontend/src/hooks/useThrottledValue.ts
import { useState, useEffect, useRef } from 'react'

export function useThrottledValue<T>(value: T, delayMs: number): T {
  const [throttled, setThrottled] = useState(value)
  const lastUpdate = useRef(Date.now())
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    const elapsed = Date.now() - lastUpdate.current
    if (elapsed >= delayMs) {
      lastUpdate.current = Date.now()
      setThrottled(value)
    } else {
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => {
        lastUpdate.current = Date.now()
        setThrottled(value)
      }, delayMs - elapsed)
    }
    return () => { if (timerRef.current) clearTimeout(timerRef.current) }
  }, [value, delayMs])

  return throttled
}
```

- [ ] **Step 3: Create useAutoScroll hook**

```typescript
// frontend/src/hooks/useAutoScroll.ts
import { useRef, useCallback, useEffect } from 'react'

export function useAutoScroll(deps: unknown[]) {
  const containerRef = useRef<HTMLDivElement>(null)
  const autoScrollRef = useRef(true)
  const programmaticScrollRef = useRef(false)

  const scrollToBottom = useCallback(() => {
    const el = containerRef.current
    if (!el) return
    programmaticScrollRef.current = true
    el.scrollTop = el.scrollHeight
    // Reset after a frame
    requestAnimationFrame(() => { programmaticScrollRef.current = false })
  }, [])

  const handleScroll = useCallback(() => {
    if (programmaticScrollRef.current) return
    const el = containerRef.current
    if (!el) return
    const threshold = 2
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight <= threshold
    autoScrollRef.current = atBottom
  }, [])

  useEffect(() => {
    if (autoScrollRef.current) {
      scrollToBottom()
    }
  }, deps)

  return { containerRef, handleScroll, scrollToBottom, autoScrollRef }
}
```

- [ ] **Step 4: Create Markdown component**

```tsx
// frontend/src/components/shared/Markdown.tsx
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeSanitize from 'rehype-sanitize'
import rehypeHighlight from 'rehype-highlight'
import { useThrottledValue } from '@/hooks/useThrottledValue'

interface MarkdownProps {
  content: string
  throttle?: number  // ms, default 80
}

export function Markdown({ content, throttle = 80 }: MarkdownProps) {
  const throttled = useThrottledValue(content, throttle)

  return (
    <div className="text-[#e0e0e0] leading-[1.5] prose prose-invert max-w-none prose-sm">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[
          [rehypeSanitize, {
            tagNames: ['p', 'div', 'span', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
              'ul', 'ol', 'li', 'blockquote', 'pre', 'code', 'table', 'thead',
              'tbody', 'tr', 'th', 'td', 'em', 'strong', 'del', 'a', 'br', 'hr',
              'input', 'details', 'summary'],
            attributes: {
              '*': ['className', 'id', 'data-*'],
              'a': ['href', 'title', 'target', 'rel'],
              'input': ['type', 'checked', 'disabled'],
              'code': ['className'],
              'pre': ['className'],
              'details': ['open'],
            },
            strip: ['script', 'iframe', 'object', 'embed', 'img', 'video', 'audio'],
          }],
          rehypeHighlight,
        ]}
      >
        {throttled || (content === '' ? '_Thinking..._' : '')}
      </ReactMarkdown>
    </div>
  )
}
```

- [ ] **Step 5: Create ConversationView**

```tsx
// frontend/src/components/panels/ConversationView.tsx
import { useAtomValue } from 'jotai'
import { useState, useCallback } from 'react'
import { conversationByAgentAtom } from '@/stores/conversation'
import { isRunningAtom } from '@/stores/connection'
import { Markdown } from '@/components/shared/Markdown'
import { useAutoScroll } from '@/hooks/useAutoScroll'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { ConversationEntry } from '@/types'

function ToolDetailModal({
  entry, onClose
}: {
  entry: { toolCall: ConversationEntry & { type: 'ToolCall' }; result?: ConversationEntry & { type: 'ToolResult' } }
  onClose: () => void
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="bg-[#252540] border border-[#333355] rounded-lg p-4 max-w-2xl w-full mx-4 max-h-[80vh] overflow-y-auto"
        onClick={e => e.stopPropagation()}>
        <h3 className="text-lg font-bold mb-2">Tool: {entry.toolCall.toolName}</h3>
        <div className="mb-4">
          <div className="text-xs text-[#888] mb-1">Arguments</div>
          <pre className="bg-[#1a1a2e] p-2 rounded text-xs overflow-x-auto whitespace-pre-wrap">
            {entry.toolCall.fullArguments}
          </pre>
        </div>
        {entry.result && (
          <div>
            <div className="text-xs text-[#888] mb-1">
              Result {entry.result.success
                ? <span className="text-[#40c040]">OK</span>
                : <span className="text-[#c04040]">ERR</span>}
            </div>
            <Markdown content={entry.result.fullResult} />
          </div>
        )}
        <Button variant="outline" size="sm" className="mt-4" onClick={onClose}>Close</Button>
      </div>
    </div>
  )
}

function TimelineEntry({
  entry, index, entries, isLast
}: {
  entry: ConversationEntry; index: number; entries: ConversationEntry[]; isLast: boolean
}) {
  const [detailOpen, setDetailOpen] = useState(false)

  // Find matching ToolResult after a ToolCall
  const toolDetail = entry.type === 'ToolCall' ? (() => {
    const resultEntry = entries.slice(index + 1).find(
      e => e.type === 'ToolResult' && e.toolName === entry.toolName
    )
    return { toolCall: entry, result: resultEntry as (ConversationEntry & { type: 'ToolResult' }) | undefined }
  })() : null

  const dotColor = entry.type === 'UserInput' ? '#80a0ff' :
    entry.type === 'Error' ? '#c04040' : '#888'

  return (
    <div className="flex gap-2">
      {/* Left rail */}
      <div className="flex flex-col items-center w-5 flex-shrink-0 pt-1">
        {entry.type === 'UserInput'
          ? <span className="text-[#80a0ff] text-xs">❯</span>
          : <span className="w-2 h-2 rounded-full" style={{ backgroundColor: dotColor, boxShadow: `0 0 3px ${dotColor}` }} />
        }
        {index < entries.length - 1 && <div className="w-px flex-1 bg-[#333355] mt-1" />}
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0 pb-3">
        {entry.type === 'UserInput' && (
          <div className="text-[#e0e0e0] whitespace-pre-wrap">{entry.text}</div>
        )}
        {entry.type === 'Thinking' && (
          <div className="text-[#888] italic text-sm">{entry.content || 'Thinking...'}</div>
        )}
        {entry.type === 'ContentStreaming' && (
          <Markdown content={entry.content} />
        )}
        {entry.type === 'ToolCall' && (
          <div className="flex items-center gap-2 cursor-pointer group" onClick={() => setDetailOpen(true)}>
            <span className="text-[#f0c040] text-xs">[tool]</span>
            <span className="text-[#e0e0e0] text-sm">{entry.toolName}</span>
            <span className="text-[#888] text-xs truncate">{entry.argPreview}</span>
            <span className="hidden group-hover:inline text-[#888] text-xs">more »</span>
          </div>
        )}
        {entry.type === 'ToolResult' && (
          <div className="cursor-pointer" onClick={() => setDetailOpen(true)}>
            <span className={`text-xs px-1 py-0.5 rounded mr-1 ${entry.success ? 'text-[#40c040] bg-[#1a3a1a]' : 'text-[#c04040] bg-[#3a1a1a]'}`}>
              {entry.success ? 'OK' : 'ERR'}
            </span>
            <span className="text-[#e0e0e0] text-sm line-clamp-2">{entry.preview}</span>
          </div>
        )}
        {entry.type === 'AgentAnswer' && <Markdown content={entry.text} />}
        {entry.type === 'Error' && (
          <div className="text-[#c04040] text-sm">{entry.message}</div>
        )}
        {entry.type === 'RunningBanner' && (
          <div className="text-[#f0c040] text-xs italic">Agent running (run: {entry.runId.slice(0, 8)}...)</div>
        )}
        {entry.type === 'RunSummary' && (
          <div className="text-[#888] text-xs">
            Done | {entry.iterations} iterations | {entry.toolCalls} tool calls | {entry.elapsedMs}ms
          </div>
        )}
        {entry.type === 'EntryCheckpoint' && (
          <div className="text-[#888] text-xs italic">Checkpoint: {entry.reason}</div>
        )}
      </div>

      {/* Tool detail modal */}
      {detailOpen && toolDetail && (
        <ToolDetailModal entry={toolDetail} onClose={() => setDetailOpen(false)} />
      )}
    </div>
  )
}

export function ConversationView() {
  const conv = useAtomValue(conversationByAgentAtom)
  const isRunning = useAtomValue(isRunningAtom)
  const { containerRef, handleScroll } = useAutoScroll([conv.entries.length])

  const entries = conv.entries

  if (entries.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-[#666] text-sm">
        Select an agent and start a conversation
      </div>
    )
  }

  return (
    <ScrollArea className="flex-1" ref={containerRef} onScroll={handleScroll}>
      <div className="p-3 sm:p-4">
        {entries.map((entry, i) => (
          <TimelineEntry
            key={i}
            entry={entry}
            index={i}
            entries={entries}
            isLast={i === entries.length - 1}
          />
        ))}
        {isRunning && entries.length > 0 && entries[entries.length - 1].type === 'RunningBanner' && (
          <div className="flex items-center gap-2 text-[#f0c040] text-xs">
            <span className="w-2 h-2 rounded-full bg-[#f0c040] animate-pulse" />
            Running...
          </div>
        )}
      </div>
    </ScrollArea>
  )
}
```

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/panels/ConversationView.tsx frontend/src/components/shared/Markdown.tsx frontend/src/hooks/
git commit -m "feat(frontend): ConversationView with streaming markdown and tool detail modals"
```

### Task 2.4: AgentsPanel

**Files:** Create `frontend/src/components/panels/AgentsPanel.tsx`

**Interfaces:** Consumes `agentsAtom`, `selectedAgentIdAtom`, `agentSubTabAtom`, `activeNodeIdAtom`; reads `JsonRpcClient` from App context (passed via a new `useClient()` hook)

**Steps:**
- [ ] Create `useClient` hook (`frontend/src/hooks/useClient.ts`) that creates a ref-based singleton `JsonRpcClient` stored in a Jotai atom. Components call `useClient()` to get `{ client, dpPool }`.
- [ ] Implement `AgentsPanel`: fetch `agent.list` from DP/CP client on mount when `activeNodeId` is set. Render agent card grid (scrollable, max-h 200px): name, scope badge (repo=#80a0ff, user=#40c040), description. Click toggles selection → sets `selectedAgentIdAtom`, checks `agent.status` → if running, load running session entries and prepend `RunningBanner` into conversation.
- [ ] Sub-tabs: Conversation (renders `ConversationView` + `CapabilityBar` + `InputArea`), Sessions (renders `SessionsPanel`), Context (renders `ContextPanel`), Tasks (renders `TasksPanel` with `assignee_filter=agent.name`)
- [ ] Info bar shows selected agent name + description
- [ ] Error state: "No agent selected. Please select an agent" when `activeNodeId` is null. Loading state with spinner during `agent.list` fetch.
- [ ] Commit: `git commit -m "feat(frontend): AgentsPanel with agent cards and sub-tab routing"`

### Task 2.5: InputArea + CapabilityBar + CapabilityDrawer

**Files:** Create `InputArea.tsx`, `CapabilityBar.tsx`, `CapabilityDrawer.tsx` in respective dirs

**Steps:**
- [ ] **InputArea**: Textarea (16px font mobile / 14px sm), disabled while `isRunningAtom` is true. Enter submits (Ctrl+Enter = newline), Esc×2 clears. Submit: calls `client.call('agent.submit', {input, target})`, sets `pendingSubmitAgentAtom`. **Gap fix**: Cancel button visible when running, calls `client.call('agent.cancel', {run_id})`. "+ New Session" generates `web-{Date.now().toString(36)}` session id.
- [ ] **CapabilityBar**: Between conversation and input. Shows `🛠 N tools · N skills · N MCPs` from `capabilityOverlayAtom`. Edit button opens drawer. Fetches `agent.get_capabilities(agent_id, session_id)` on agent change.
- [ ] **CapabilityDrawer**: Right-side fixed panel (w-80). On open: fetch capabilities → fill `selTools/selSkills/selMcps` atoms. Sections: collapsible headers, search input, per-item toggle switches with saving feedback (spinner → checkmark ✓ → persistent). `handleToggle`: optimistic local update → `client.call('agent.update_capabilities', {...})` → on success update effective atoms, on error rollback + show warning. Race guard: discard stale responses.
- [ ] Commit: `git commit -m "feat(frontend): InputArea with cancel, CapabilityBar, and CapabilityDrawer"`

---

## Phase 3: Tools + MCP + Skills

### Task 3.1: SchemaForm component

**Files:** Create `frontend/src/components/inputs/SchemaForm.tsx`

**Steps:**
- [ ] JSON Schema → form renderer: string (text input or enum select), number/integer, boolean (checkbox), nested object (recursive). Required `*` markers. Title/description labels. Uses shadcn Input, Select, Checkbox, Label. Writes into shared `serde_json`-like object via `onChange` callback.
- [ ] Test with schema: `{type: "object", properties: {command: {type: "string", description: "Shell command"}, timeout: {type: "integer", default: 30}}}`
- [ ] Commit: `git commit -m "feat(frontend): JSON Schema form renderer"`

### Task 3.2: ToolsTab

**Files:** Create `frontend/src/components/panels/ToolsTab.tsx`, `frontend/src/components/dialogs/ToolCallDialog.tsx`

**Steps:**
- [ ] Loads `tool.list` from node cache. System Tools section: name + description rows (cards mobile, table desktop). Run button → `ToolCallDialog` (SchemaForm from tool parameters, Execute → `tool.call(name, args)` → show result/error).
- [ ] Call History section: reads `ToolCallEntry[]` from tool calls store (fed by event handlers). Expandable rows: seq, name, status, duration, arg preview. Refresh button re-fetches tool list.
- [ ] Commit: `git commit -m "feat(frontend): ToolsTab with system tools and call history"`

### Task 3.3: McpPanel (4 sub-tabs + dialogs)

**Files:** Create `McpPanel.tsx`, `McpToolDialog.tsx`, `ResourceViewer.tsx`, `PromptViewer.tsx`

**Steps:**
- [ ] Node-cached `mcp_state`. 5 parallel RPC calls (`mcp.list_*`) with "last one clears loading" pattern. Stale-response guard on node switch.
- [ ] **Servers sub-tab**: status dot per server (connected=green, disconnected=gray, else red). Reconnect button → `mcp.reconnect(server)` → re-fetch all 5 lists. Per-server "Reconnecting..." pulse.
- [ ] **Tools sub-tab**: grouped by server. Call → `ToolCallDialog` (SchemaForm from `input_schema`, Execute → `mcp.call_tool(args)`).
- [ ] **Resources sub-tab**: Read → `ResourceViewer` (URI, Read button → `mcp.read_resource`, display content).
- [ ] **Prompts sub-tab**: Get → `PromptViewer`. **Gap fix**: `mcp.get_prompt(server, prompt_name, args)` — must actually call RPC, not stub.
- [ ] Commit: `git commit -m "feat(frontend): McpPanel with 4 sub-tabs and all dialogs"`

### Task 3.4: SkillsPanel + SkillDetailDialog + ApprovalDialog

**Files:** Create `SkillsPanel.tsx`, `SkillDetailDialog.tsx`; already created `ApprovalDialog.tsx`

**Steps:**
- [ ] **SkillsPanel**: Node-cached `skill.list`. Table (desktop) / cards (mobile): name, version, scope badge, description. Click → `skill.get(name)` → `SkillDetailDialog`. Refresh → `skill.refresh()` then re-list.
- [ ] **SkillDetailDialog**: Modal with name/version/scope, description, trigger chips, SKILL.md content block, file listing → click `file.read(dir/file)` → preview pane.
- [ ] **ApprovalDialog**: HITL modal reading from `approvalAtom` (populated by event handler on `ApprovalRequest`). Shows tool name `[!]`, reason, arguments. **Gap fix**: Approve calls `client.call('agent.approve', {req_id, approved: true})`; Reject calls same with `approved: false`.
- [ ] Commit: `git commit -m "feat(frontend): Skills panel, detail dialog, and working approval dialog"`

### Task 3.5: Stores for Phase 3 (tools, mcp, skills, dialogs)

**Files:** Create `frontend/src/stores/tools.ts`, `mcp.ts`, `skills.ts`, `dialogs.ts`, `capability.ts`

**Steps:**
- [ ] `tools.ts`: `toolCallsAtom`, `systemToolsAtom`, `toolsLoadingAtom`
- [ ] `mcp.ts`: `mcpStateAtom` (servers/tools/resources/prompts/loading/error/activeSubtab)
- [ ] `skills.ts`: `skillsAtom`, `skillsLoadingAtom`, `skillsErrorAtom`
- [ ] `dialogs.ts`: `approvalAtom` (toolName, reason, arguments, reqId), `mcpDialogAtom` (toolCall/resourceViewer/promptViewer state), `skillDialogAtom`, `debugPanelAtom`
- [ ] `capability.ts`: `capOverlayAtom`, `drawerOpenAtom`, `drawerSearchAtom`, `savingStatesAtom`, `selectedToolsAtom`, `selectedSkillsAtom`, `selectedMcpsAtom`
- [ ] Commit: `git commit -m "feat(frontend): add stores for tools, MCP, skills, dialogs, and capabilities"`

---

## Phase 4: Tasks + Sessions + Context

### Task 4.1: TasksPanel + TaskDepGraph

**Files:** Create `TasksPanel.tsx`, `TaskDepGraph.tsx`; stores in `stores/tasks.ts`

**Steps:**
- [ ] `tasks.ts`: `tasksAtom`, `statusFilterAtom`, `selectedTaskIdAtom`, `tasksLoadingAtom`
- [ ] **TasksPanel**: Fetch `task.list(status?, assignee?)` from node cache. Status filter chips: all/pending/running/completed. Mobile cards / desktop rows: `t{id}`, status badge (pending=#888, running=#80a0ff, completed=#40c040, failed=#c04040, killed=#ff8800), subject, assignee, "⇄ deps" button → `TaskDepGraph`. Click row toggles expanded detail (description, dependencies, blocks).
- [ ] **TaskDepGraph**: SVG dependency graph. Pure `build_graph_layout(tasks, centerTaskId)` function: longest-path layering (upstream above, downstream below), cycle-safe via visited set, unknown nodes dashed with "(not loaded)". Arrow markers + rounded rects (status-colored fill, gold star ★ for center). Click node → detail panel inside modal.
- [ ] Commit: `git commit -m "feat(frontend): TasksPanel with filters and SVG dependency graph"`

### Task 4.2: SessionsPanel

**Files:** Create `SessionsPanel.tsx`, `SessionDetailOverlay.tsx`

**Steps:**
- [ ] Fetch `session.list(agent_id)` on mount. Rows/cards: truncated id, entry count, age label (s/m/h/d ago). View → `session.entries(session_id)` → parse wire format (message/checkpoint/summary entries) → `SessionDetailOverlay` modal renders entries as conversation.
- [ ] Resume button: `session.resume(session_id, agent_id)` → replace active conversation entries → switch to Conversation sub-tab. 15s safety timeout resets button.
- [ ] `session_entries_to_conversation` function mirrors Rust implementation: "message" entries (user/assistant/tool roles with thinking + tool_calls extraction), "checkpoint" → EntryCheckpoint, "summary" → RunSummary.
- [ ] Commit: `git commit -m "feat(frontend): SessionsPanel with detail overlay and resume"`

### Task 4.3: ContextPanel

**Files:** Create `ContextPanel.tsx`, `ContextDialog.tsx`; stores in `stores/context.ts`

**Steps:**
- [ ] `context.ts`: `contributorsAtom`, `contextLoadingAtom`, `contextDialogAtom` (open, contributorName, messages, loading)
- [ ] **ContextPanel**: Fetches `agent.context_config(agent_id)` → contributor rows: anchor_zone badge (head=#80a0ff, middle=#c0a040, tail=#40c040), name, estimated_tokens, message_count. Click → `agent.context_snapshot(agent_id, contributor_name)` → `ContextDialog` modal with role-colored message blocks (system=#888, user=#80a0ff, assistant=#e0e0e0, tool=#c0a040).
- [ ] Commit: `git commit -m "feat(frontend): ContextPanel with contributor list and message viewer"`

---

## Phase 5: Workspace + Logs + Debug + Nodes

### Task 5.1: FileTree (full) + FileContentView

**Files:** Update `FileTree.tsx` (from skeleton); create `FileContentView.tsx`, store `workspace.ts`

**Steps:**
- [ ] `workspace.ts`: `workspaceTreeAtom` (WorkspaceTreeNode recursive), `openFilesAtom` (OpenFileTab[]), `selectedFileTabAtom`, `collapsedDirsAtom`, `fileTreeDrawerOpenAtom`
- [ ] **FileTree**: Left sidebar. On node change: restore from cache (`"files"` or `"workspace_tree"`), else `file.list(".")`. TreeNode: dirs with custom CSS chevron, click toggles collapse / lazy `file.list(path)` with stale-response guard, ⟳ refresh per dir. Files: click → add `OpenFileTab`, `file.read(path)` async fill, switch `activeTabAtom` to `'workspace'`. Emoji icons per extension (🦀 .rs, ⚙️ .toml, 📝 .md, etc.). Mobile: collapsed rail (vertical "Files") with drawer overlay on `fileTreeDrawerOpenAtom`. Desktop: 240px sidebar.
- [ ] **FileContentView**: Tab strip (icon, name, × close with selection fixup) + content in `<pre>` or error or loading state.
- [ ] Commit: `git commit -m "feat(frontend): full FileTree with lazy loading, mobile drawer, and file content viewer"`

### Task 5.2: LogViewer

**Files:** Create `LogViewer.tsx`; store `logs.ts`

**Steps:**
- [ ] `logs.ts`: `logRunsAtom`, `selectedRunAtom`, `logEntriesAtom`, `logAutoScrollAtom`
- [ ] Run list (`log.list`): truncated run_id, event count, last event + time. Click → `log.read(run_id)` → entries with timestamp + color-coded event type (green=yellow=red). "← Back to run list" button. Node-cached.
- [ ] Commit: `git commit -m "feat(frontend): LogViewer with run list and per-run entry drilldown"`

### Task 5.3: DebugPanel

**Files:** Create `DebugPanel.tsx`

**Steps:**
- [ ] Full-screen modal toggled by 🐛 button in StatusBar. Tabs: "WS" (WS messages captured via `client.onEvent` debug hook — each `call()` and `eventStream()` also pushes to `debugMessagesAtom`). Direction arrows ←/→ colored, timestamp (HH:MM:SS.mmm), method, expandable pretty-printed JSON payload. "N messages · Recording since page load" footer.
- [ ] `debugPanelAtom` in `dialogs.ts`: `{ open: boolean, activeTab: 'ws', messages: WsMessage[] }`
- [ ] Commit: `git commit -m "feat(frontend): DebugPanel with WebSocket message inspector"`

### Task 5.4: NodesDropdown + NodesPanel + NodeDetailPanel

**Files:** Create `NodesDropdown.tsx`, `NodesPanel.tsx`, `NodeDetailPanel.tsx`

**Steps:**
- [ ] **NodesDropdown**: In StatusBar, only visible when `serverModeAtom === 'ControlPlane'`. Collapsible "▾ Nodes(N)" button. Fixed-position panel: per-node rows (status dot green=online, name — click opens NodeDetail, row click selects + sets `activeNodeIdAtom` + creates DP connection via `dpPool.getOrCreate(nodeId, wsUrl)`, shows ✓ checkmark. Shows `R:{running} Q:{queued}`, agent count.
- [ ] **NodesPanel**: Tab content (accessible via `activeTabAtom = 'nodes'`). Lists nodes via CP `node_list`. If `viewingNodeDetailAtom` is set → renders `NodeDetailPanel`.
- [ ] **NodeDetailPanel**: Overview (id/name/version/status/last_seen/cap_revision), Resource Usage (running/queued stat cards), Agents on Node (via DP agent_list), Capabilities (badge counts + lists from `capability_list`). 5s auto-refresh polling with cleanup on unmount. "← Back" clears `viewingNodeDetailAtom`.
- [ ] Commit: `git commit -m "feat(frontend): Nodes dropdown, panel, and detail with auto-refresh"`

---

## Phase 6: Polish + Tests

### Task 6.1: Mobile responsive audit + iOS zoom fix

**Steps:**
- [ ] Audit every panel at 480px width: FileTree must collapse to drawer (vertical rail + overlay), tables must switch to cards (`sm:hidden` / `hidden sm:block` pattern), StatusBar must not overflow
- [ ] Ensure all text inputs have `text-base` (16px) on mobile to prevent iOS zoom
- [ ] Verify viewport meta tag: `<meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />`
- [ ] Verify CapabilityDrawer uses full-width on mobile
- [ ] Commit: `git commit -m "fix(frontend): mobile responsive audit and iOS zoom prevention"`

### Task 6.2: Playwright test adaptation

**Files:** Create `frontend/tests/e2e/` with adapted Playwright tests

**Steps:**
- [ ] Copy `crates/vol-llm-ui/tests/web/playwright.config.js` → update for React dev server (baseURL `http://localhost:5173`)
- [ ] Adapt `capability_drawer.spec.js`: update selectors for new DOM structure (shadcn classes). Verify: drawer hidden on load, opens on click, toggle works
- [ ] Adapt `markdown.spec.js`: update selectors. Verify: markdown renders, streaming throttles to ≤12 renders/sec, CDN fallback is no longer relevant (remove that test)
- [ ] Run: `npx playwright test` — all adapted tests pass
- [ ] Commit: `git commit -m "test(frontend): adapt Playwright tests for React version"`

### Task 6.3: Dark theme consistency + final integration

**Steps:**
- [ ] Scan every component for hardcoded colors — ensure all use the dark theme tokens or shadcn variables
- [ ] Verify shadcn `Dialog`, `Tabs`, `ScrollArea`, `Badge`, `Button` all render correctly in dark theme
- [ ] Run full integration smoke test against local `vol-agent-server`:
  - Connect → StatusBar shows "Connected" + server type
  - Select agent → agents list loads
  - Submit message → conversation streams (thinking → content → tools)
  - Tools tab shows system tools
  - MCP tab shows servers/tools/resources
  - Capability drawer opens and toggles work
  - Reconnect works (kill backend, verify countdown, restart, verify reconnection)
- [ ] Commit: `git commit -m "fix(frontend): dark theme consistency pass and final integration"`

---

## Task Summary

| Phase | Tasks | Files Created | Key Deliverable |
|---|---|---|---|
| P1: Shell | 1.1–1.7 | `package.json`, `vite.config.ts`, `tsconfig*.json`, `index.html`, `main.tsx`, `App.tsx`, `index.css`, `components.json`, `lib/utils.ts`, 5 `ui/*.tsx`, `types/index.ts`, `lib/protocol.ts`, `lib/jsonrpc-client.ts`, `lib/dp-pool.ts`, `lib/reconnect.ts`, `stores/connection.ts`, `stores/ui.ts`, `lib/ws-url.ts`, `StatusBar.tsx`, `TabBar.tsx`, `TabContent.tsx`, `ConnectionIndicator.tsx`, `FileTree.tsx` (skeleton), 3 test files | App shell with dark theme, WS connection, tab switching |
| P2: Core UX | 2.1–2.5 | `stores/agents.ts`, `conversation.ts`, `cache.ts`, `lib/event-handlers.ts`, `ConversationView.tsx`, `Markdown.tsx`, `AgentsPanel.tsx`, `InputArea.tsx`, `CapabilityBar.tsx`, `CapabilityDrawer.tsx`, 3 hooks | Full conversation flow working end-to-end |
| P3: Tools | 3.1–3.5 | `SchemaForm.tsx`, `ToolsTab.tsx`, `ToolCallDialog.tsx`, `McpPanel.tsx`, `McpToolDialog.tsx`, `ResourceViewer.tsx`, `PromptViewer.tsx`, `SkillsPanel.tsx`, `SkillDetailDialog.tsx`, `ApprovalDialog.tsx`, `stores/tools.ts`, `stores/mcp.ts`, `stores/skills.ts`, `stores/dialogs.ts`, `stores/capability.ts` | All tool/MCP/skill panels + gap fixes |
| P4: Data | 4.1–4.3 | `TasksPanel.tsx`, `TaskDepGraph.tsx`, `SessionsPanel.tsx`, `SessionDetailOverlay.tsx`, `ContextPanel.tsx`, `ContextDialog.tsx`, `stores/tasks.ts`, `stores/sessions.ts`, `stores/context.ts` | Tasks, sessions, context panels |
| P5: Infra | 5.1–5.4 | `FileContentView.tsx`, `LogViewer.tsx`, `DebugPanel.tsx`, `NodesDropdown.tsx`, `NodesPanel.tsx`, `NodeDetailPanel.tsx`, `stores/workspace.ts`, `stores/logs.ts` | All remaining panels + node management |
| P6: Polish | 6.1–6.3 | Playwright config adaptation, test updates | Mobile, E2E tests, dark theme audit |

**Total: ~55 files created, 3 test files, 6 phases, ~30 tasks**