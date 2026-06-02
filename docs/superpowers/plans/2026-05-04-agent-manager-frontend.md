# Agent Manager Frontend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a React + TypeScript + Vite SPA that provides a management console and chat interface for the vol-agent-manager backend service.

**Architecture:** Single-page application with client-side routing. Vite dev server proxies API/WS to agent-manager on port 8080. Production served via Nginx with reverse proxy. No global state manager — local component state only.

**Tech Stack:** React 18, TypeScript, Vite 5, Ant Design 5, React Router 6, Axios, native WebSocket API, EventSource API.

---

### Task 1: Project Scaffolding

Create the foundational project files: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`.

**Files:**
- Create: `frontend/package.json`
- Create: `frontend/vite.config.ts`
- Create: `frontend/tsconfig.json`
- Create: `frontend/index.html`

- [ ] **Step 1: Create package.json**

```json
{
  "name": "vol-agent-manager-ui",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "react-router-dom": "^6.26.0",
    "antd": "^5.20.0",
    "axios": "^1.7.0",
    "@ant-design/icons": "^5.4.0"
  },
  "devDependencies": {
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "~5.5.0",
    "vite": "^5.4.0"
  }
}
```

- [ ] **Step 2: Create vite.config.ts**

```typescript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 3000,
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
      '/ws': {
        target: 'http://localhost:8080',
        ws: true,
      },
      '/health': {
        target: 'http://localhost:8080',
      },
    },
  },
});
```

- [ ] **Step 3: Create tsconfig.json**

```json
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
    "forceConsistentCasingInFileNames": true
  },
  "include": ["src"]
}
```

- [ ] **Step 4: Create index.html**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Vol Agent Manager</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: Create directory structure and install**

Run: `mkdir -p frontend/src/{api,hooks,pages,components,types,styles}`

Run: `cd frontend && npm install`

Expected: `added XX packages` with no errors.

- [ ] **Step 6: Verify dev server starts**

Run: `cd frontend && timeout 10 npx vite --host 2>&1 | head -20 || true`

Expected: Shows "Local: http://localhost:3000/" in output, no type errors.

- [ ] **Step 7: Commit**

Run: `cd /root/nq-deribit && git add frontend/package.json frontend/vite.config.ts frontend/tsconfig.json frontend/index.html && git commit -m "feat: scaffold frontend project with Vite, React, Ant Design"`

---

### Task 2: Types and API Client Layer

Define TypeScript types and create the Axios-based API client.

**Files:**
- Create: `frontend/src/types/index.ts`
- Create: `frontend/src/api/client.ts`
- Create: `frontend/src/api/agentTypes.ts`
- Create: `frontend/src/api/instances.ts`
- Create: `frontend/src/api/health.ts`

- [ ] **Step 1: Create types/index.ts**

```typescript
export interface AgentTypeMeta {
  name: string;
  type: string;
  description: string;
  scope: string;
}

export interface AgentInstanceSummary {
  agent_type: string;
  session_id: string;
  parent_session_id: string | null;
  status: string;
  connection_count: number;
  created_at: string;
}

export interface WsConnected {
  message_type: 'connected';
  agent_type: string;
  session_id: string;
}

export interface WsAgentComplete {
  message_type: 'agent_complete';
  content: string;
  iterations: number;
}

export interface WsAgentError {
  message_type: 'agent_error';
  error: string;
}

export type WsMessage = WsConnected | WsAgentComplete | WsAgentError;

export interface ManagerEvent {
  event_type: string;
  timestamp: string;
  payload: Record<string, unknown>;
}

export interface HealthResponse {
  status: string;
}

export interface AgentTypesResponse {
  agent_types: AgentTypeMeta[];
}

export interface InstancesResponse {
  instances: AgentInstanceSummary[];
}
```

- [ ] **Step 2: Create api/client.ts**

```typescript
import axios from 'axios';

export const apiClient = axios.create({
  baseURL: '/',
  timeout: 10000,
  headers: { 'Content-Type': 'application/json' },
});

apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.code === 'ERR_NETWORK') {
      console.error('Agent manager is unreachable');
    }
    return Promise.reject(error);
  },
);
```

- [ ] **Step 3: Create api/agentTypes.ts**

```typescript
import { apiClient } from './client';
import type { AgentTypesResponse, AgentTypeMeta } from '../types';

export async function fetchAgentTypes(): Promise<AgentTypeMeta[]> {
  const { data } = await apiClient.get<AgentTypesResponse>('/api/v1/agent-types');
  return data.agent_types;
}
```

- [ ] **Step 4: Create api/instances.ts**

```typescript
import { apiClient } from './client';
import type { InstancesResponse, AgentInstanceSummary } from '../types';

export async function fetchInstances(): Promise<AgentInstanceSummary[]> {
  const { data } = await apiClient.get<InstancesResponse>('/api/v1/agent-instances');
  return data.instances;
}

export async function destroyInstance(agentType: string, sessionId: string): Promise<void> {
  await apiClient.delete(`/api/v1/agent-instances/${agentType}/${sessionId}`);
}
```

- [ ] **Step 5: Create api/health.ts**

```typescript
import { apiClient } from './client';
import type { HealthResponse } from '../types';

export async function checkHealth(): Promise<boolean> {
  try {
    const { data } = await apiClient.get<HealthResponse>('/health');
    return data.status === 'ok';
  } catch {
    return false;
  }
}
```

- [ ] **Step 6: Create src/main.tsx (minimal entry point)**

```typescript
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 7: Create src/App.tsx (minimal placeholder)**

```typescript
function App() {
  return <div>Agent Manager UI</div>;
}

export default App;
```

- [ ] **Step 8: Verify TypeScript compiles**

Run: `cd frontend && npx tsc --noEmit`

Expected: No errors.

- [ ] **Step 9: Commit**

Run: `cd /root/nq-deribit && git add frontend/src/types frontend/src/api frontend/src/main.tsx frontend/src/App.tsx && git commit -m "feat: add TypeScript types and API client layer"`

---

### Task 3: Custom Hooks (useWebSocket + useEvents)

Implement the two core React hooks that encapsulate real-time communication.

**Files:**
- Create: `frontend/src/hooks/useWebSocket.ts`
- Create: `frontend/src/hooks/useEvents.ts`
- Test: Manual verification (no unit test framework — hooks depend on browser APIs)

- [ ] **Step 1: Create hooks/useWebSocket.ts**

```typescript
import { useState, useEffect, useRef, useCallback } from 'react';
import type { WsMessage } from '../types';

export type WsStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

interface UseWebSocketOptions {
  enabled?: boolean;
}

export function useWebSocket(url: string | null, options: UseWebSocketOptions = {}) {
  const { enabled = true } = options;
  const [status, setStatus] = useState<WsStatus>('disconnected');
  const [messages, setMessages] = useState<WsMessage[]>([]);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reconnectAttempts = useRef(0);
  const pendingMessages = useRef<string[]>([]);

  const getReconnectDelay = useCallback(() => {
    const delay = Math.min(1000 * Math.pow(2, reconnectAttempts.current), 30000);
    reconnectAttempts.current += 1;
    return delay;
  }, []);

  const connect = useCallback(() => {
    if (!url || !enabled) return;

    setStatus('connecting');
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      setStatus('connected');
      reconnectAttempts.current = 0;
      // Flush pending messages
      while (pendingMessages.current.length > 0) {
        ws.send(pendingMessages.current.shift()!);
      }
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data) as WsMessage;
        setMessages((prev) => [...prev, msg]);
      } catch {
        console.warn('Failed to parse WS message:', event.data);
      }
    };

    ws.onclose = () => {
      setStatus('disconnected');
      wsRef.current = null;
      if (enabled && url) {
        const delay = getReconnectDelay();
        reconnectTimerRef.current = setTimeout(connect, delay);
      }
    };

    ws.onerror = () => {
      setStatus('error');
    };
  }, [url, enabled, getReconnectDelay]);

  const disconnect = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    setStatus('disconnected');
  }, []);

  const send = useCallback(
    (message: object) => {
      const text = JSON.stringify(message);
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(text);
      } else {
        pendingMessages.current.push(text);
      }
    },
    [],
  );

  useEffect(() => {
    if (url && enabled) {
      connect();
    }
    return () => {
      disconnect();
    };
  }, [url, enabled, connect, disconnect]);

  // Reset messages when URL changes (new session)
  useEffect(() => {
    setMessages([]);
  }, [url]);

  return { status, send, messages, connect, disconnect };
}
```

- [ ] **Step 2: Create hooks/useEvents.ts**

```typescript
import { useState, useEffect, useRef, useCallback } from 'react';
import type { ManagerEvent } from '../types';

const MAX_EVENTS = 100;

interface UseEventsOptions {
  enabled?: boolean;
}

export function useEvents(options: UseEventsOptions = {}) {
  const { enabled = true } = options;
  const [events, setEvents] = useState<ManagerEvent[]>([]);
  const [paused, setPaused] = useState(false);
  const [connected, setConnected] = useState(false);
  const esRef = useRef<EventSource | null>(null);

  const togglePause = useCallback(() => {
    setPaused((prev) => !prev);
  }, []);

  useEffect(() => {
    if (!enabled) return;

    const es = new EventSource('/api/v1/events');
    esRef.current = es;

    es.onopen = () => setConnected(true);

    es.onmessage = (event) => {
      if (paused) return;
      try {
        const evt = JSON.parse(event.data) as ManagerEvent;
        setEvents((prev) => {
          const next = [...prev, evt];
          if (next.length > MAX_EVENTS) {
            return next.slice(next.length - MAX_EVENTS);
          }
          return next;
        });
      } catch {
        console.warn('Failed to parse SSE event:', event.data);
      }
    };

    es.onerror = () => {
      setConnected(false);
      // EventSource auto-reconnects, but we track connection state
    };

    return () => {
      es.close();
      esRef.current = null;
    };
  }, [enabled, paused]);

  const clear = useCallback(() => {
    setEvents([]);
  }, []);

  return { events, connected, paused, togglePause, clear };
}
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd frontend && npx tsc --noEmit`

Expected: No errors.

- [ ] **Step 4: Commit**

Run: `cd /root/nq-deribit && git add frontend/src/hooks && git commit -m "feat: add useWebSocket and useEvents hooks"`

---

### Task 4: Layout, Routing, Dashboard, StatusBadge

Create the application shell with sidebar navigation, routing, and the Dashboard page.

**Files:**
- Create: `frontend/src/components/AppLayout.tsx`
- Create: `frontend/src/components/StatusBadge.tsx`
- Create: `frontend/src/pages/Dashboard.tsx`
- Modify: `frontend/src/App.tsx` — replace placeholder with full router + layout

- [ ] **Step 1: Create components/StatusBadge.tsx**

```typescript
import React from 'react';
import { Badge } from 'antd';
import type { WsStatus } from '../hooks/useWebSocket';

interface StatusBadgeProps {
  status: WsStatus;
}

const statusMap: Record<WsStatus, { color: string; text: string }> = {
  connecting: { color: 'orange', text: 'Connecting...' },
  connected: { color: 'green', text: 'Connected' },
  disconnected: { color: 'default', text: 'Disconnected' },
  error: { color: 'red', text: 'Connection Error' },
};

export const StatusBadge: React.FC<StatusBadgeProps> = ({ status }) => {
  const { color, text } = statusMap[status];
  return <Badge status={color as 'default' | 'error' | 'processing' | 'success' | 'warning'} text={text} />;
};
```

- [ ] **Step 2: Create components/AppLayout.tsx**

```typescript
import React, { useState, useEffect } from 'react';
import { Layout, Menu, Badge } from 'antd';
import {
  DashboardOutlined,
  AppstoreOutlined,
  ClusterOutlined,
  MessageOutlined,
  NotificationOutlined,
} from '@ant-design/icons';
import { useNavigate, useLocation } from 'react-router-dom';
import { checkHealth } from '../api/health';

const { Sider, Header, Content } = Layout;

const menuItems = [
  { key: '/', icon: <DashboardOutlined />, label: 'Dashboard' },
  { key: '/agent-types', icon: <AppstoreOutlined />, label: 'Agent Types' },
  { key: '/instances', icon: <ClusterOutlined />, label: 'Instances' },
  { key: '/events', icon: <NotificationOutlined />, label: 'Events' },
];

interface AppLayoutProps {
  children: React.ReactNode;
}

export const AppLayout: React.FC<AppLayoutProps> = ({ children }) => {
  const navigate = useNavigate();
  const location = useLocation();
  const [healthy, setHealthy] = useState<boolean | null>(null);

  useEffect(() => {
    checkHealth().then(setHealthy);
    const timer = setInterval(() => checkHealth().then(setHealthy), 30000);
    return () => clearInterval(timer);
  }, []);

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider theme="light" width={220}>
        <div style={{ padding: '16px', textAlign: 'center', fontSize: '16px', fontWeight: 'bold' }}>
          Vol Agent Manager
        </div>
        <Menu
          mode="inline"
          selectedKeys={[location.pathname.split('/chat')[0] || '/']}
          items={menuItems}
          onClick={({ key }) => navigate(key)}
        />
      </Sider>
      <Layout>
        <Header style={{ background: '#fff', padding: '0 24px', display: 'flex', alignItems: 'center' }}>
          <span>Agent Manager Console</span>
          <div style={{ marginLeft: 'auto' }}>
            {healthy === null ? null : healthy ? (
              <Badge status="success" text="Healthy" />
            ) : (
              <Badge status="error" text="Unavailable" />
            )}
          </div>
        </Header>
        <Content style={{ margin: '24px 16px', padding: 24, background: '#fff', minHeight: 280 }}>
          {children}
        </Content>
      </Layout>
    </Layout>
  );
};
```

- [ ] **Step 3: Create pages/Dashboard.tsx**

```typescript
import React, { useState, useEffect } from 'react';
import { Row, Col, Card, Statistic, Spin, Alert, List } from 'antd';
import {
  AppstoreOutlined,
  ClusterOutlined,
  CheckCircleOutlined,
} from '@ant-design/icons';
import { fetchAgentTypes } from '../api/agentTypes';
import { fetchInstances } from '../api/instances';
import { checkHealth } from '../api/health';
import type { AgentTypeMeta, AgentInstanceSummary, ManagerEvent } from '../types';

export const Dashboard: React.FC = () => {
  const [agentTypes, setAgentTypes] = useState<AgentTypeMeta[]>([]);
  const [instances, setInstances] = useState<AgentInstanceSummary[]>([]);
  const [healthy, setHealthy] = useState<boolean | null>(null);
  const [recentEvents, setRecentEvents] = useState<ManagerEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function loadData() {
      try {
        setLoading(true);
        const [types, insts, ok] = await Promise.all([
          fetchAgentTypes(),
          fetchInstances(),
          checkHealth(),
        ]);
        if (!cancelled) {
          setAgentTypes(types);
          setInstances(insts);
          setHealthy(ok);
        }
      } catch (e: unknown) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : 'Failed to load data');
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    loadData();
    return () => { cancelled = true; };
  }, []);

  // Fetch recent events via SSE
  useEffect(() => {
    const es = new EventSource('/api/v1/events');
    const events: ManagerEvent[] = [];
    es.onmessage = (event) => {
      try {
        const evt = JSON.parse(event.data) as ManagerEvent;
        events.push(evt);
        if (events.length > 5) events.shift();
        setRecentEvents([...events]);
      } catch { /* ignore */ }
    };
    return () => es.close();
  }, []);

  if (loading) return <Spin size="large" style={{ display: 'block', margin: '48px auto' }} />;
  if (error) return <Alert message="Failed to load dashboard data" description={error} type="error" showIcon />;

  return (
    <div>
      <Row gutter={[16, 16]}>
        <Col span={8}>
          <Card>
            <Statistic
              title="Agent Types"
              value={agentTypes.length}
              prefix={<AppstoreOutlined />}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card>
            <Statistic
              title="Running Instances"
              value={instances.length}
              prefix={<ClusterOutlined />}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card>
            <Statistic
              title="Health Status"
              value={healthy ? 'OK' : 'Down'}
              prefix={healthy ? <CheckCircleOutlined style={{ color: '#52c41a' }} /> : <CheckCircleOutlined style={{ color: '#ff4d4f' }} />}
            />
          </Card>
        </Col>
      </Row>
      <Card title="Recent Events" style={{ marginTop: 16 }}>
        {recentEvents.length === 0 ? (
          <div style={{ color: '#999' }}>No events received yet</div>
        ) : (
          <List
            size="small"
            dataSource={recentEvents}
            renderItem={(evt) => (
              <List.Item>
                <List.Item.Meta
                  title={evt.event_type}
                  description={new Date(evt.timestamp).toLocaleTimeString()}
                />
                <span style={{ fontSize: 12, color: '#666' }}>
                  {JSON.stringify(evt.payload).slice(0, 80)}...
                </span>
              </List.Item>
            )}
          />
        )}
      </Card>
    </div>
  );
};
```

- [ ] **Step 4: Update App.tsx with routing**

```typescript
import React from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { ConfigProvider } from 'antd';
import { AppLayout } from './components/AppLayout';
import { Dashboard } from './pages/Dashboard';

function App() {
  return (
    <ConfigProvider>
      <BrowserRouter>
        <AppLayout>
          <Routes>
            <Route path="/" element={<Dashboard />} />
          </Routes>
        </AppLayout>
      </BrowserRouter>
    </ConfigProvider>
  );
}

export default App;
```

- [ ] **Step 5: Add missing import in AppLayout**

Ensure `AppLayout.tsx` has the proper imports at the top:

```typescript
import React, { useState, useEffect } from 'react';
import { Layout, Menu, Badge } from 'antd';
import {
  DashboardOutlined,
  AppstoreOutlined,
  ClusterOutlined,
  NotificationOutlined,
} from '@ant-design/icons';
import { useNavigate, useLocation } from 'react-router-dom';
import { checkHealth } from '../api/health';
```

- [ ] **Step 6: Verify TypeScript compiles**

Run: `cd frontend && npx tsc --noEmit`

Expected: No errors.

- [ ] **Step 7: Commit**

Run: `cd /root/nq-deribit && git add frontend/src/components/AppLayout.tsx frontend/src/components/StatusBadge.tsx frontend/src/pages/Dashboard.tsx frontend/src/App.tsx && git commit -m "feat: add layout, routing, and dashboard page"`

---

### Task 5: Agent Types Page

Display all discovered agent types in a table with a "Start Chat" action.

**Files:**
- Create: `frontend/src/pages/AgentTypes.tsx`
- Modify: `frontend/src/App.tsx` — add route for AgentTypes

- [ ] **Step 1: Create pages/AgentTypes.tsx**

```typescript
import React, { useState, useEffect } from 'react';
import { Table, Button, Empty, Spin, Alert } from 'antd';
import { MessageOutlined } from '@ant-design/icons';
import { useNavigate } from 'react-router-dom';
import { fetchAgentTypes } from '../api/agentTypes';
import type { AgentTypeMeta } from '../types';

const columns = [
  { title: 'Name', dataIndex: 'name', key: 'name' },
  { title: 'Type', dataIndex: 'type', key: 'type' },
  { title: 'Description', dataIndex: 'description', key: 'description' },
  { title: 'Scope', dataIndex: 'scope', key: 'scope', width: 100 },
  {
    title: 'Actions',
    key: 'actions',
    width: 140,
    render: (_: unknown, record: AgentTypeMeta) => {
      const navigate = useNavigate();
      return (
        <Button
          type="primary"
          icon={<MessageOutlined />}
          size="small"
          onClick={() => navigate(`/chat/${record.name}/new`)}
        >
          Start Chat
        </Button>
      );
    },
  },
];

export const AgentTypes: React.FC = () => {
  const navigate = useNavigate();
  const [types, setTypes] = useState<AgentTypeMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchAgentTypes()
      .then((data) => { if (!cancelled) setTypes(data); })
      .catch((e) => { if (!cancelled) setError(e.message); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, []);

  if (loading) return <Spin size="large" style={{ display: 'block', margin: '48px auto' }} />;
  if (error) return <Alert message="Failed to load agent types" description={error} type="error" showIcon />;
  if (types.length === 0) {
    return (
      <Empty
        description="No agent types discovered"
        image={Empty.PRESENTED_IMAGE_SIMPLE}
      >
        <p style={{ color: '#999' }}>
          Add .md agent definition files to <code>.agents/agents/</code> directory.
        </p>
      </Empty>
    );
  }

  return (
    <Table
      columns={columns}
      dataSource={types}
      rowKey="name"
      pagination={false}
    />
  );
};
```

- [ ] **Step 2: Add AgentTypes route to App.tsx**

Import and add route:

```typescript
import { AgentTypes } from './pages/AgentTypes';
```

Inside `<Routes>`:

```typescript
<Route path="/agent-types" element={<AgentTypes />} />
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd frontend && npx tsc --noEmit`

Expected: No errors.

- [ ] **Step 4: Commit**

Run: `cd /root/nq-deribit && git add frontend/src/pages/AgentTypes.tsx frontend/src/App.tsx && git commit -m "feat: add agent types listing page"`

---

### Task 6: Instances Page

Display running instances with create/destroy/chat actions.

**Files:**
- Create: `frontend/src/pages/Instances.tsx`
- Modify: `frontend/src/App.tsx` — add route for Instances

- [ ] **Step 1: Create pages/Instances.tsx**

```typescript
import React, { useState, useEffect, useCallback } from 'react';
import {
  Table, Button, Modal, Form, Input, Select, Space,
  Popconfirm, message, Tag, Alert, Spin,
} from 'antd';
import { PlusOutlined, DeleteOutlined, MessageOutlined, ReloadOutlined } from '@ant-design/icons';
import { useNavigate } from 'react-router-dom';
import { fetchInstances, destroyInstance } from '../api/instances';
import { fetchAgentTypes } from '../api/agentTypes';
import type { AgentInstanceSummary, AgentTypeMeta } from '../types';

export const Instances: React.FC = () => {
  const navigate = useNavigate();
  const [instances, setInstances] = useState<AgentInstanceSummary[]>([]);
  const [agentTypes, setAgentTypes] = useState<AgentTypeMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [form] = Form.useForm();

  const loadInstances = useCallback(async () => {
    try {
      const data = await fetchInstances();
      setInstances(data);
      setError(null);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to load');
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    Promise.all([fetchAgentTypes(), loadInstances()])
      .then(([types]) => { if (!cancelled) setAgentTypes(types); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [loadInstances]);

  const handleCreate = async (values: { agentType: string; sessionId: string; parentSessionId?: string }) => {
    // Creation: open chat with new session, backend creates instance on WS connect
    setCreateModalOpen(false);
    navigate(`/chat/${values.agentType}/${values.sessionId}`);
  };

  const handleDestroy = async (agentType: string, sessionId: string) => {
    try {
      await destroyInstance(agentType, sessionId);
      message.success('Instance destroyed');
      loadInstances();
    } catch {
      message.error('Failed to destroy instance');
    }
  };

  const columns = [
    { title: 'Agent Type', dataIndex: 'agent_type', key: 'agent_type' },
    { title: 'Session ID', dataIndex: 'session_id', key: 'session_id', ellipsis: true },
    { title: 'Parent Session', dataIndex: 'parent_session_id', key: 'parent_session_id', ellipsis: true, render: (v: string | null) => v || '-' },
    {
      title: 'Status', dataIndex: 'status', key: 'status', width: 100,
      render: (status: string) => <Tag color={status === 'Running' ? 'green' : 'default'}>{status}</Tag>,
    },
    { title: 'Connections', dataIndex: 'connection_count', key: 'connection_count', width: 120 },
    {
      title: 'Created', dataIndex: 'created_at', key: 'created_at', width: 200,
      render: (v: string) => new Date(v).toLocaleString(),
    },
    {
      title: 'Actions', key: 'actions', width: 200,
      render: (_: unknown, record: AgentInstanceSummary) => (
        <Space>
          <Button
            type="link" size="small" icon={<MessageOutlined />}
            onClick={() => navigate(`/chat/${record.agent_type}/${record.session_id}`)}
          >
            Chat
          </Button>
          <Popconfirm
            title="Destroy this instance?"
            onConfirm={() => handleDestroy(record.agent_type, record.session_id)}
            okText="Destroy"
            cancelText="Cancel"
          >
            <Button type="link" danger size="small" icon={<DeleteOutlined />}>
              Destroy
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  if (loading) return <Spin size="large" style={{ display: 'block', margin: '48px auto' }} />;

  return (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between' }}>
        <Space>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => setCreateModalOpen(true)}>
            New Instance
          </Button>
          <Button icon={<ReloadOutlined />} onClick={loadInstances}>Refresh</Button>
        </Space>
      </div>
      {error && <Alert message="Failed to load instances" description={error} type="error" showIcon style={{ marginBottom: 16 }} />}
      <Table
        columns={columns}
        dataSource={instances}
        rowKey={(r) => `${r.agent_type}/${r.session_id}`}
        pagination={false}
      />
      <Modal
        title="Create New Instance"
        open={createModalOpen}
        onCancel={() => setCreateModalOpen(false)}
        footer={null}
      >
        <Form form={form} onFinish={handleCreate} layout="vertical">
          <Form.Item name="agentType" label="Agent Type" rules={[{ required: true }]}>
            <Select>
              {agentTypes.map((t) => (
                <Select.Option key={t.name} value={t.name}>{t.name} ({t.type})</Select.Option>
              ))}
            </Select>
          </Form.Item>
          <Form.Item name="sessionId" label="Session ID" rules={[{ required: true }]}>
            <Input placeholder="Enter session ID" />
          </Form.Item>
          <Form.Item name="parentSessionId" label="Parent Session ID (optional)">
            <Input placeholder="For forked sessions" />
          </Form.Item>
          <Form.Item>
            <Button type="primary" htmlType="submit">Create & Chat</Button>
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
};
```

- [ ] **Step 2: Add Instances route to App.tsx**

Import and add route:

```typescript
import { Instances } from './pages/Instances';
```

Inside `<Routes>`:

```typescript
<Route path="/instances" element={<Instances />} />
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd frontend && npx tsc --noEmit`

Expected: No errors.

- [ ] **Step 4: Commit**

Run: `cd /root/nq-deribit && git add frontend/src/pages/Instances.tsx frontend/src/App.tsx && git commit -m "feat: add instances management page with create/destroy"`

---

### Task 7: Chat Page and ChatWindow Component

The core interactive feature — real-time agent conversation via WebSocket.

**Files:**
- Create: `frontend/src/components/ChatWindow.tsx`
- Create: `frontend/src/pages/Chat.tsx`
- Modify: `frontend/src/App.tsx` — add route for Chat with params

- [ ] **Step 1: Create components/ChatWindow.tsx**

```typescript
import React, { useRef, useEffect } from 'react';
import { Input, Button, Space, Typography } from 'antd';
import { SendOutlined } from '@ant-design/icons';
import { useWebSocket } from '../hooks/useWebSocket';
import { StatusBadge } from './StatusBadge';
import type { WsMessage } from '../types';

const { TextArea } = Input;
const { Text } = Typography;

interface ChatWindowProps {
  agentType: string;
  sessionId: string;
}

function renderMessage(msg: WsMessage, index: number) {
  switch (msg.message_type) {
    case 'connected':
      return (
        <div key={index} style={{ textAlign: 'center', color: '#999', fontSize: 12, margin: '8px 0' }}>
          Connected to {msg.agent_type} (session: {msg.session_id})
        </div>
      );
    case 'agent_complete':
      return (
        <div key={index} style={{ display: 'flex', justifyContent: 'flex-start', marginBottom: 12 }}>
          <div style={{
            background: '#f0f0f0', borderRadius: 12, padding: '10px 16px',
            maxWidth: '70%', whiteSpace: 'pre-wrap', wordBreak: 'break-word',
          }}>
            {msg.content}
            <div style={{ fontSize: 11, color: '#999', marginTop: 4 }}>
              Completed in {msg.iterations} iterations
            </div>
          </div>
        </div>
      );
    case 'agent_error':
      return (
        <div key={index} style={{ textAlign: 'center', color: '#ff4d4f', margin: '8px 0' }}>
          Error: {msg.error}
        </div>
      );
    default:
      return null;
  }
}

export const ChatWindow: React.FC<ChatWindowProps> = ({ agentType, sessionId }) => {
  const [input, setInput] = React.useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const wsUrl = `/ws/agents/${agentType}/session/${sessionId}`;
  const { status, send, messages } = useWebSocket(wsUrl);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSend = () => {
    if (!input.trim() || status !== 'connected') return;
    send({ content: input.trim() });
    setInput('');
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: 'calc(100vh - 200px)' }}>
      {/* Header */}
      <div style={{ padding: '12px 16px', borderBottom: '1px solid #f0f0f0', display: 'flex', alignItems: 'center' }}>
        <Text strong>{agentType}</Text>
        <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>{sessionId}</Text>
        <div style={{ marginLeft: 'auto' }}>
          <StatusBadge status={status} />
        </div>
      </div>

      {/* Messages */}
      <div style={{ flex: 1, overflowY: 'auto', padding: 16 }}>
        {messages.map((msg, i) => renderMessage(msg, i))}
        {messages.length === 0 && status === 'connecting' && (
          <div style={{ textAlign: 'center', color: '#999', marginTop: 48 }}>Connecting...</div>
        )}
        {messages.length === 0 && status === 'error' && (
          <div style={{ textAlign: 'center', color: '#ff4d4f', marginTop: 48 }}>
            Connection failed. Retrying automatically...
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div style={{ padding: '12px 16px', borderTop: '1px solid #f0f0f0' }}>
        <Space.Compact style={{ width: '100%' }}>
          <TextArea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a message... (Enter to send)"
            autoSize={{ minRows: 1, maxRows: 4 }}
            disabled={status !== 'connected'}
          />
          <Button
            type="primary"
            icon={<SendOutlined />}
            onClick={handleSend}
            disabled={status !== 'connected' || !input.trim()}
          >
            Send
          </Button>
        </Space.Compact>
      </div>
    </div>
  );
};
```

- [ ] **Step 2: Create pages/Chat.tsx**

```typescript
import React from 'react';
import { useParams } from 'react-router-dom';
import { ChatWindow } from '../components/ChatWindow';

export const Chat: React.FC = () => {
  const { agentType, sessionId } = useParams<{ agentType: string; sessionId: string }>();

  if (!agentType || !sessionId) {
    return <div style={{ padding: 24, color: '#999' }}>Invalid agent or session parameters</div>;
  }

  return <ChatWindow agentType={agentType} sessionId={sessionId} />;
};
```

- [ ] **Step 3: Add Chat route to App.tsx**

Import and add route:

```typescript
import { Chat } from './pages/Chat';
```

Inside `<Routes>`:

```typescript
<Route path="/chat/:agentType/:sessionId" element={<Chat />} />
```

- [ ] **Step 4: Verify TypeScript compiles**

Run: `cd frontend && npx tsc --noEmit`

Expected: No errors.

- [ ] **Step 5: Commit**

Run: `cd /root/nq-deribit && git add frontend/src/components/ChatWindow.tsx frontend/src/pages/Chat.tsx frontend/src/App.tsx && git commit -m "feat: add chat page with real-time WebSocket conversation"`

---

### Task 8: Events Page

Live SSE event stream viewer with pause/resume.

**Files:**
- Create: `frontend/src/pages/Events.tsx`
- Modify: `frontend/src/App.tsx` — add route for Events

- [ ] **Step 1: Create pages/Events.tsx**

```typescript
import React from 'react';
import { Table, Button, Space, Tag, Badge } from 'antd';
import { PauseOutlined, PlayOutlined, ClearOutlined } from '@ant-design/icons';
import { useEvents } from '../hooks/useEvents';
import type { ManagerEvent } from '../types';

const columns = [
  {
    title: 'Timestamp', dataIndex: 'timestamp', key: 'timestamp', width: 180,
    render: (v: string) => new Date(v).toLocaleString(),
  },
  {
    title: 'Event Type', dataIndex: 'event_type', key: 'event_type', width: 200,
    render: (type: string) => <Tag color="blue">{type}</Tag>,
  },
  {
    title: 'Details', dataIndex: 'payload', key: 'payload',
    render: (payload: Record<string, unknown>) => (
      <pre style={{ margin: 0, fontSize: 12, maxHeight: 60, overflow: 'auto' }}>
        {JSON.stringify(payload, null, 2)}
      </pre>
    ),
  },
];

export const Events: React.FC = () => {
  const { events, connected, paused, togglePause, clear } = useEvents();

  return (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <Space>
          <Badge status={connected ? 'success' : 'error'} text={connected ? 'Stream Connected' : 'Stream Disconnected'} />
          <span style={{ color: '#999' }}>{events.length} events</span>
        </Space>
        <Space>
          <Button icon={paused ? <PlayOutlined /> : <PauseOutlined />} onClick={togglePause}>
            {paused ? 'Resume' : 'Pause'}
          </Button>
          <Button icon={<ClearOutlined />} onClick={clear}>Clear</Button>
        </Space>
      </div>
      <Table
        columns={columns}
        dataSource={events}
        rowKey={(_, i) => String(i)}
        pagination={false}
        size="small"
        scroll={{ y: 500 }}
      />
    </div>
  );
};
```

- [ ] **Step 2: Add Events route to App.tsx**

Import and add route:

```typescript
import { Events } from './pages/Events';
```

Inside `<Routes>`:

```typescript
<Route path="/events" element={<Events />} />
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd frontend && npx tsc --noEmit`

Expected: No errors.

- [ ] **Step 4: Commit**

Run: `cd /root/nq-deribit && git add frontend/src/pages/Events.tsx frontend/src/App.tsx && git commit -m "feat: add events stream page with SSE and pause/resume"`

---

### Task 9: Deployment Configuration (Nginx, Dockerfile)

Create production deployment artifacts.

**Files:**
- Create: `frontend/nginx.conf`
- Create: `frontend/Dockerfile`
- Create: `frontend/.dockerignore`

- [ ] **Step 1: Create nginx.conf**

```nginx
server {
    listen 80;
    server_name localhost;

    root /usr/share/nginx/html;
    index index.html;

    # SPA fallback
    location / {
        try_files $uri $uri/ /index.html;
    }

    # REST API proxy
    location /api/ {
        proxy_pass http://vol-agent-manager:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    # WebSocket proxy
    location /ws/ {
        proxy_pass http://vol-agent-manager:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 86400s;
    }

    # Health check
    location /health {
        proxy_pass http://vol-agent-manager:8080;
    }
}
```

- [ ] **Step 2: Create Dockerfile**

```dockerfile
FROM node:20-alpine AS builder

WORKDIR /app

COPY package*.json ./
RUN npm ci --ignore-scripts

COPY . .
RUN npm run build

FROM nginx:alpine

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
```

- [ ] **Step 3: Create .dockerignore**

```
node_modules
dist
.git
```

- [ ] **Step 4: Verify build works**

Run: `cd frontend && npm run build`

Expected: `dist/` directory created, no errors. Build output shows total size.

Run: `du -sh frontend/dist/`

Expected: < 5MB total.

- [ ] **Step 5: Commit**

Run: `cd /root/nq-deribit && git add frontend/nginx.conf frontend/Dockerfile frontend/.dockerignore && git commit -m "feat: add Nginx config and Dockerfile for frontend deployment"`

---

### Task 10: Global Styles and Final Polish

Add global CSS, verify full app works end-to-end.

**Files:**
- Create: `frontend/src/styles/global.css`
- Modify: `frontend/src/main.tsx` — import global CSS

- [ ] **Step 1: Create styles/global.css**

```css
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

#root {
  min-height: 100vh;
}

/* Chat message animations */
@keyframes fadeIn {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

/* Scrollbar styling for chat */
::-webkit-scrollbar {
  width: 6px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: #d9d9d9;
  border-radius: 3px;
}

::-webkit-scrollbar-thumb:hover {
  background: #bfbfbf;
}
```

- [ ] **Step 2: Import global.css in main.tsx**

Add at the top of `main.tsx`:

```typescript
import './styles/global.css';
```

- [ ] **Step 3: Final TypeScript check**

Run: `cd frontend && npx tsc --noEmit`

Expected: No errors.

- [ ] **Step 4: Final build check**

Run: `cd frontend && npm run build && du -sh dist/`

Expected: Build succeeds, size < 5MB.

- [ ] **Step 5: Commit**

Run: `cd /root/nq-deribit && git add frontend/src/styles/global.css frontend/src/main.tsx && git commit -m "chore: add global styles and final polish"`

---
