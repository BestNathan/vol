# Agent Manager Frontend Design Spec

## Architecture

A single-page application (SPA) built with React 18 + TypeScript + Vite, served from `frontend/` directory at the repository root.

**Tech Stack:**
- React 18 + TypeScript
- Vite 5.x as build tool and dev server
- Ant Design 5.x as UI component library
- React Router 6.x for client-side routing
- Native WebSocket API (no library) for real-time communication
- EventSource API for SSE
- Axios for REST API calls
- No global state manager — React `useState`/`useReducer` per component scope

**No external state management library** (Redux, Zustand, Jotai) — the app is small enough that local component state + context for connection status is sufficient. This keeps bundle size minimal and avoids additional learning curve.

## Project Structure

```
frontend/
├── package.json
├── tsconfig.json
├── vite.config.ts
├── index.html
├── nginx.conf              # Nginx reverse proxy config
├── Dockerfile              # Multi-stage: node builder -> nginx alpine
├── src/
│   ├── main.tsx            # Entry point, React mount
│   ├── App.tsx             # Router + Layout wrapper
│   ├── api/                # REST API client layer
│   │   ├── client.ts       # Axios instance (relative baseURL)
│   │   ├── agentTypes.ts   # GET /api/v1/agent-types
│   │   ├── instances.ts    # GET/DELETE /api/v1/agent-instances
│   │   └── health.ts       # GET /health
│   ├── hooks/              # Custom React hooks
│   │   ├── useWebSocket.ts # WS connection with auto-reconnect
│   │   └── useEvents.ts    # SSE event stream
│   ├── pages/              # Route-level page components
│   │   ├── Dashboard.tsx   # Overview: counts, health, recent events
│   │   ├── AgentTypes.tsx  # Agent type catalog with descriptions
│   │   ├── Instances.tsx   # Instance lifecycle (create/destroy/list)
│   │   ├── Chat.tsx        # Real-time agent conversation
│   │   └── Events.tsx      # Live event stream viewer
│   ├── components/         # Shared UI components
│   │   ├── AppLayout.tsx   # Sider + Header + Content shell
│   │   ├── ChatWindow.tsx  # Reusable chat UI (bubbles, input)
│   │   └── StatusBadge.tsx # WS connection status indicator
│   ├── types/              # TypeScript type definitions
│   │   └── index.ts
│   └── styles/
│       └── global.css
```

## Component Responsibilities

### `useWebSocket` hook

**Purpose:** Encapsulate WebSocket lifecycle for agent conversation.

**Signature:**
```typescript
function useWebSocket(
  url: string | null,  // null = don't connect
  options?: { enabled?: boolean }
): {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  send: (message: object) => void;
  messages: WsMessage[];
}
```

**Behavior:**
- Connects to `/ws/agents/:agentType/session/:sessionId` (relative URL, proxied in dev)
- On connect: receives welcome message, adds to message list
- On message: parses JSON, appends to `messages` array
- On disconnect: exponential backoff reconnect (1s → 2s → 4s → 8s → 16s → 30s cap)
- Max reconnect attempts: unlimited (user can close tab to stop)
- `send()` queues messages if disconnected, sends when connected
- Cleanup: closes connection on unmount

### `ChatWindow` component

**Props:** `{ agentType: string, sessionId: string }`

**Layout:**
- Top bar: agent type name, session ID, connection status badge
- Middle: scrollable message list with styled bubbles
- Bottom: text input + send button

**Message rendering:**
- User messages: right-aligned, blue bubble (`ant-design` primary color)
- Agent messages: left-aligned, gray bubble
- System messages: center-aligned, small text with icon (e.g., "Connected", "Agent completed", "Error: ...")
- JSON payload from backend parsed by `message_type` field:
  - `connected` → system message
  - `agent_complete` → agent message with content
  - `agent_error` → error system message

### `AppLayout` component

Ant Design `Layout` with:
- `Sider` (fixed width 220px): navigation menu
- `Header` (48px): app title, agent-manager health status indicator
- `Content`: routed page content

## Pages

### Dashboard (`/`)

Displays at-a-glance metrics:
- Card: Agent Types count (from `/api/v1/agent-types`)
- Card: Running Instances count (from `/api/v1/agent-instances`)
- Card: Health status (from `/health` — green/red badge)
- Card: Recent events (last 5 from SSE)

Data fetched on mount, no auto-refresh (user can navigate away and back).

### Agent Types (`/agent-types`)

Table listing all discovered agent types:
- Columns: Name, Type, Description, Scope, Actions
- Actions: "Start Chat" button → navigates to `/chat/:agentType/new`

Empty state: "No agents discovered. Add .md files to `.agents/agents/` directory."

### Instances (`/instances`)

Table of running instances:
- Columns: Agent Type, Session ID, Parent Session, Status, Connections, Created, Actions
- Actions: "Destroy" (with confirmation modal), "Open Chat" → `/chat/:agentType/:sessionId`
- Top toolbar: "New Instance" modal (agent type dropdown + session ID input + optional parent session ID)
- Data refreshed on mount and after any mutation (create/destroy)

### Chat (`/chat/:agentType/:sessionId`)

Single agent conversation page:
- If `sessionId` is `new`, shows creation modal first
- Once connected, displays `ChatWindow` component
- Sidebar (collapsible): session info (agent type, session ID, connection details)
- Handles WebSocket lifecycle: connecting → show spinner, error → show retry button

### Events (`/events`)

Live SSE event stream:
- Table with columns: Timestamp, Event Type, Details
- Auto-scrolls to bottom
- Limits display to last 100 entries
- Pause/Resume toggle button

## Data Flow

```
Browser                          Agent Manager (port 8080)
   |                                        |
   |--- GET /api/v1/agent-types ----------->|
   |<-- JSON: { agent_types: [...] } -------|
   |                                        |
   |--- WS /ws/agents/:type/session/:id --->|
   |<-- { message_type: "connected" } ------|
   |--- WS { content: "hello" } ----------->|
   |<-- { message_type: "agent_complete",   |
   |     content: "...", iterations: 3 } ---|
   |                                        |
   |--- SSE /api/v1/events ---------------->|
   |<-- stream: ManagerEvent JSON ----------|
```

**Development mode:** Vite dev server (`localhost:3000`) proxies `/api/*` and `/ws/*` to `http://localhost:8080`.
**Production mode:** Nginx serves static files and proxies `/api/*` and `/ws/*` to `http://vol-agent-manager:8080`.

## Error Handling

1. **Agent manager unavailable:** Dashboard shows red health badge. All API calls fail gracefully with `message.error` notification.
2. **WebSocket connection failure:** Chat shows error banner with "Reconnect" button. Auto-reconnect with exponential backoff continues in background.
3. **Agent runtime error:** Backend sends `{ message_type: "agent_error", error: "..." }`, displayed as red system message. User can send a new message to retry.
4. **Empty agent types:** AgentTypes page shows Ant Design Empty component with instructions to add agent files.
5. **SSE flood:** Events page caps at 100 displayed entries. Oldest entries dropped.

## Deployment

### Nginx Configuration

Single `server` block listening on port 80:
- `/` serves static files from `/usr/share/nginx/html` with SPA fallback (`try_files`)
- `/api/` proxies to `http://vol-agent-manager:8080`
- `/ws/` proxies with WebSocket upgrade headers
- `/health` proxies to agent-manager for Kubernetes readiness checks

### Dockerfile (Multi-stage)

Stage 1 (`node:20-alpine`):
```dockerfile
FROM node:20-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build
```

Stage 2 (`nginx:alpine`):
```dockerfile
FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
```

### Vite Dev Proxy

```typescript
server: {
  port: 3000,
  proxy: {
    '/api': { target: 'http://localhost:8080', changeOrigin: true },
    '/ws': {
      target: 'http://localhost:8080',
      ws: true,
    },
    '/health': { target: 'http://localhost:8080' },
  },
}
```

## Type Definitions

```typescript
// Agent type metadata (from /api/v1/agent-types)
interface AgentTypeMeta {
  name: string;
  type: string;
  description: string;
  scope: string;  // "User" | "Repo"
}

// Agent instance summary (from /api/v1/agent-instances)
interface AgentInstanceSummary {
  agent_type: string;
  session_id: string;
  parent_session_id: string | null;
  status: string;  // "Running" | "Stopped"
  connection_count: number;
  created_at: string;
}

// WebSocket message types
interface WsConnected { message_type: "connected"; agent_type: string; session_id: string; }
interface WsAgentComplete { message_type: "agent_complete"; content: string; iterations: number; }
interface WsAgentError { message_type: "agent_error"; error: string; }
type WsMessage = WsConnected | WsAgentComplete | WsAgentError;

// SSE event types
interface ManagerEvent {
  event_type: string;
  timestamp: string;
  payload: Record<string, unknown>;
}
```

## Bundle Size Budget

Target: < 5MB raw, < 1MB gzip after build.
- React + ReactDOM: ~42KB gzip
- Ant Design (tree-shaken): ~200-300KB gzip
- Application code: < 100KB gzip
- Source maps excluded from production Docker image
