# Node Selection and Data Routing Design

**Date:** 2026-07-23  
**Status:** Draft  
**Author:** Claude Code (brainstorming skill)

---

## 1. 概述

### 1.1 问题

当前 UI 的 nodes 列表能获取到，但存在以下问题：

1. **Node 无法选择：** `NodesPanel` 只显示 node 列表，没有选择功能
2. **数据路由错误：** Files、MCP、Tools、Tasks、Skills、Logs 等 tab 默认从 CP client 获取数据，但这些数据实际存储在 DP node 上
3. **缺少切换机制：** 用户无法切换 node 查看不同 node 的数据
4. **隐式 node 选择：** 只有 `AgentsPanel` 在选中 agent 时隐式创建 DP 连接，其他 tab 无感知

### 1.2 目标

- 主 UI 从"CP 中心"转为"DP 中心"：所有 tab 数据从**当前选中 node 的 DP 连接**获取
- CP 降级为"node 管理 + 全局聚合"角色
- 引入集中式 `NodeDataCache` 实现 per-node 缓存和秒开切换
- 在 status bar 内嵌可收起/展开的 Nodes dropdown，支持 node 选择和切换
- 提供 Node Detail UI（CP 视角），展示 node 专有信息

### 1.3 非目标

- 不实现多 node 同时查看（一次只能看一个 node 的数据）
- 不实现跨 node 的数据聚合（如"所有 node 的 tasks 列表"）
- 不实现 node 配置编辑（只读展示）
- 不实现 node 间的数据迁移或同步

---

## 2. 架构

### 2.1 数据流

```
┌─────────────────────────────────────────────────────────┐
│  StatusBar (top)                                        │
│  ┌──────────────────────────────────────────────────┐  │
│  │ CP: ● Connected  │ DP: ● node-A  │ [Nodes ▾]    │  │
│  └──────────────────────────────────────────────────┘  │
│                          │                              │
│                          ▼                              │
│              ┌─────────────────────┐                   │
│              │ active_node_id      │                   │
│              │ Signal<Option<Str>> │                   │
│              └─────────┬───────────┘                   │
│                        │                                │
└────────────────────────┼────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
   ┌──────────┐    ┌──────────┐    ┌──────────┐
   │ FileTree │    │ McpPanel │    │ TasksPanel│  ...
   └────┬─────┘    └────┬─────┘    └────┬─────┘
        │               │               │
        └───────────────┼───────────────┘
                        ▼
              ┌─────────────────────┐
              │ NodeDataCache       │
              │ HashMap<node_id,    │
              │          NodeData>  │
              └─────────┬───────────┘
                        │
                        ▼
              ┌─────────────────────┐
              │ DpConnectionPool    │
              │ HashMap<node_id,    │
              │       DpConnection> │
              └─────────┬───────────┘
                        │
                        ▼
              ┌─────────────────────┐
              │ DP WebSocket        │
              │ (node-A:3001/ws)    │
              └─────────────────────┘
```

### 2.2 关键组件

| 组件 | 职责 |
|------|------|
| `StatusBar` | 显示 CP/DP 连接状态，内嵌 Nodes dropdown |
| `NodesDropdown` | 可展开/收起，列出所有 node，点击切换 `active_node_id` |
| `NodeDataCache` | 集中式 per-node 缓存，存储各 tab 的数据 |
| `DpConnectionPool` | 管理 per-node 的 DP WebSocket 连接（已有） |
| 各 Tab 组件 | 从 `NodeDataCache[active_node_id]` 读取数据，不再直接调用 `rpc_client` |
| `NodeDetailPanel` | CP 视角，展示 node 专有信息（overview、resource、agents、capabilities） |

---

## 3. Nodes Dropdown UI

### 3.1 位置

Status bar 右侧，当前 "DP: node-A" 指示器的位置扩展为可点击的 dropdown。

### 3.2 收起状态（默认）

```
┌──────────────────────────────────────────────────────────┐
│ ● CP  │ ● DP: node-A  │ ▾ Nodes(3)  │ Session: xxx ... │
└──────────────────────────────────────────────────────────┘
```

- 点击 "▾ Nodes(3)" 展开 dropdown
- 显示当前选中 node 名（高亮）

### 3.3 展开状态

```
┌──────────────────────────────────────────────────────────┐
│ ● CP  │ ● DP: node-A  │ ▾ Nodes(3)  │ Session: xxx ... │
├──────────────────────────────────────────────────────────┤
│ ┌─ Nodes ──────────────────────────────────────────────┐ │
│ │ ● node-A  v0.1.0  online   cpu:12% mem:256MB  ← 选中│ │
│ │ ○ node-B  v0.1.0  online   cpu:5%  mem:128MB        │ │
│ │ ○ node-C  v0.1.0  offline  ---                       │ │
│ └──────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

### 3.4 交互

1. **点击 node 行** → 设置为 `active_node_id` → dropdown 收起 → 所有 tab 切换到该 node 的数据
2. **首次加载** → `control.node_list` → 如果有 node，自动选中第一个 online 的 node
3. **Node 状态变化** → CP heartbeat 更新 node 状态 → dropdown 列表实时刷新
4. **点击 node 卡片上的 name** → 进入 Node Detail UI（见 Section 7）

### 3.5 实现细节

```rust
// StatusBar 内部
#[component]
fn StatusBar() -> Element {
    // ... 现有代码 ...
    let nodes_dropdown_open = use_signal(|| false);
    
    rsx! {
        div { class: "status-bar",
            // ... 现有 CP/DP 指示器 ...
            
            // Nodes dropdown
            div { class: "relative",
                button {
                    class: "nodes-toggle",
                    onclick: move |_| nodes_dropdown_open.toggle(),
                    "▾ Nodes({nodes.len()})"
                }
                if *nodes_dropdown_open.read() {
                    NodesDropdown {
                        nodes: nodes.clone(),
                        selected: active_node_id.clone(),
                        on_select: move |node_id| {
                            active_node_id.set(Some(node_id));
                            nodes_dropdown_open.set(false);
                        },
                    }
                }
            }
        }
    }
}
```

---

## 4. NodeDataCache 结构

### 4.1 职责

集中存储每个 node 的各 tab 数据，实现 per-node 缓存和秒开切换。

### 4.2 数据结构

```rust
// state/mod.rs 新增

/// Per-node cached data for all DP-scoped tabs.
#[derive(Debug, Clone, Default)]
pub struct NodeData {
    pub files: Option<WorkspaceState>,
    pub mcp: Option<McpState>,
    pub tools: Option<ToolState>,
    pub tasks: Option<TaskState>,
    pub skills: Option<SkillsState>,
    pub logs: Option<LogsState>,
}

/// Centralized cache: node_id → NodeData.
#[derive(Debug, Clone, Default)]
pub struct NodeDataCache {
    cache: HashMap<String, NodeData>,
}

impl NodeDataCache {
    pub fn get(&self, node_id: &str) -> Option<&NodeData> {
        self.cache.get(node_id)
    }
    
    pub fn get_mut(&mut self, node_id: &str) -> Option<&mut NodeData> {
        self.cache.get_mut(node_id)
    }
    
    pub fn get_or_insert(&mut self, node_id: &str) -> &mut NodeData {
        self.cache.entry(node_id.to_string()).or_default()
    }
    
    pub fn invalidate(&mut self, node_id: &str) {
        self.cache.remove(node_id);
    }
}
```

### 4.3 AppState 集成

```rust
pub struct AppState {
    pub event_bus: EventBus,
    pub rpc_client: JsonRpcClient,           // CP client
    pub active_tab: Signal<ActiveTab>,
    pub cp_client: JsonRpcClient,            // alias for rpc_client
    pub dp_pool: Signal<DpConnectionPool>,
    pub active_node_id: Signal<Option<String>>,
    pub node_data_cache: Signal<NodeDataCache>,  // 新增
}
```

### 4.4 数据流

```
用户切换 node (点击 Nodes dropdown)
    ↓
active_node_id.set(Some("node-B"))
    ↓
所有 tab 组件触发 re-render
    ↓
FileTree 组件:
    let cache = app.node_data_cache.read();
    let node_data = cache.get("node-B");
    
    match node_data {
        Some(data) if data.files.is_some() => {
            // 缓存命中，直接显示
            render_file_tree(&data.files.unwrap())
        }
        _ => {
            // 缓存未命中，加载
            show_loading();
            let dp_client = app.dp_pool.read().get("node-B").map(|c| c.client.clone());
            if let Some(client) = dp_client {
                client.file_list(".", move |result| {
                    let mut cache = app.node_data_cache.write();
                    let node_data = cache.get_or_insert("node-B");
                    node_data.files = Some(build_workspace_state(result));
                    // 触发 re-render
                });
            }
        }
    }
```

### 4.5 缓存失效策略

- **手动刷新**：用户在 tab 里点 "Refresh" → `node_data_cache.invalidate(active_node_id)` → 重新加载
- **Node 断线重连**：DP WebSocket 断开重连后 → `invalidate(active_node_id)` → 重新加载
- **永不过期**：除非手动刷新或断线，缓存一直有效（符合"秒开"需求）

---

## 5. Tab 数据路由改造

### 5.1 改造清单

| Tab | 当前数据源 | 改造后 |
|-----|-----------|--------|
| FileTree | `app.rpc_client.file_list()` | `dp_pool.get(active_node_id).file_list()` |
| McpPanel | `app.rpc_client.mcp_list_*()` | `dp_pool.get(active_node_id).mcp_list_*()` |
| ToolsTab | `app.rpc_client` (event stream) | `dp_pool.get(active_node_id)` (event stream) |
| TasksPanel | `app.rpc_client.task_list()` | `dp_pool.get(active_node_id).task_list()` |
| SkillsPanel | `app.rpc_client.skill_list()` | `dp_pool.get(active_node_id).skill_list()` |
| LogViewer | `app.rpc_client` (event stream) | `dp_pool.get(active_node_id)` (event stream) |
| AgentsPanel | `app.rpc_client.agent_list()` | **保持不变** — agent 列表从 CP 获取（CP 聚合所有 node 的 agent） |
| NodesPanel | `app.cp_client.node_list()` | **保持不变** — node 列表本身就是 CP 数据 |

### 5.2 Tab 组件改造示例（FileTree）

```rust
// 改造前
#[component]
pub fn FileTree() -> Element {
    let app = use_context::<AppState>();
    let ws: Signal<WorkspaceState> = use_context();
    
    use_hook(move || {
        let rpc = app.rpc_client.clone();  // ← CP client
        let sig = ws;
        rpc.file_list(".", move |result| {
            // ...
        });
    });
}

// 改造后
#[component]
pub fn FileTree() -> Element {
    let app = use_context::<AppState>();
    let active_node = app.active_node_id;
    let cache = app.node_data_cache;
    
    // 从缓存读取，或触发加载
    let files = use_memo(move || {
        let node_id = active_node.read().clone()?;
        let cache_read = cache.read();
        let node_data = cache_read.get(&node_id)?;
        node_data.files.clone()
    });
    
    // 缓存未命中时加载
    use_effect(move || {
        let node_id = active_node.read().clone();
        if let Some(ref nid) = node_id {
            let cache_read = cache.read();
            let needs_load = cache_read.get(nid)
                .and_then(|d| d.files.as_ref())
                .is_none();
            
            if needs_load {
                let dp_client = app.dp_pool.read().get(nid).map(|c| c.client.clone());
                if let Some(client) = dp_client {
                    let cache_mut = cache;
                    let nid_clone = nid.clone();
                    client.file_list(".", move |result| {
                        let mut c = cache_mut.write();
                        let node_data = c.get_or_insert(&nid_clone);
                        node_data.files = Some(build_workspace_state(result));
                    });
                }
            }
        }
    });
    
    // 渲染
    match files.read().as_ref() {
        Some(ws_state) => render_file_tree(ws_state),
        None => rsx! { div { "Loading..." } },
    }
}
```

### 5.3 关键设计决策

1. **Tab 组件不再维护自己的 `Signal<WorkspaceState>`** — 数据从 `NodeDataCache` 读取
2. **加载逻辑在 `use_effect` 里** — 监听 `active_node_id` 变化，缓存未命中时触发
3. **DP 连接懒创建** — 切换 node 时才从 `dp_pool.get_or_create()` 获取连接
4. **Event stream（Tools/Logs）** — 需要为每个 DP 连接启动独立的 event loop

### 5.4 Event stream 改造（Tools/Logs）

```rust
// 当前：App 级别启动一个 event loop，监听 CP client 的 events
// 改造后：每个 DP 连接有自己的 event loop

// 在 dp_pool.get_or_create() 时启动
pub fn get_or_create(&mut self, node_id: &str, ws_url: &str) -> &DpConnection {
    if !self.connections.contains_key(node_id) {
        let client = JsonRpcClient::new(ws_url);
        
        // 启动该 DP 连接的 event loop
        let bus = self.event_bus.clone();
        let client_clone = client.clone();
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                match client_clone.next_event().await {
                    Some(event) => bus.publish(&event),
                    None => break,
                }
            }
        });
        
        self.connections.insert(...);
    }
    self.connections.get(node_id).unwrap()
}
```

---

## 6. Node 选择流程

### 6.1 初始加载流程

```
App 启动
    ↓
CP WebSocket 连接成功
    ↓
调用 control.node_list 获取 node 列表
    ↓
如果有 node:
    ├─ 找到第一个 status == "online" 的 node
    ├─ 设置为 active_node_id
    ├─ 从 dp_pool.get_or_create(node_id, ws_url) 获取 DP 连接
    └─ 各 tab 的 use_effect 触发加载（缓存未命中）
如果无 node:
    └─ 显示 "No nodes available" 提示
```

### 6.2 切换 node 流程

```
用户点击 Nodes dropdown 里的 node-B
    ↓
active_node_id.set(Some("node-B"))
    ↓
触发所有 tab 的 use_effect（监听 active_node_id 变化）
    ↓
各 tab 检查 NodeDataCache[node-B]:
    ├─ 缓存命中 → 直接显示（秒开）
    └─ 缓存未命中 → 显示 loading → 从 DP 加载 → 写入缓存
```

### 6.3 Node 离线/断线处理

```rust
// CP heartbeat 检测到 node 离线
if node.status == "offline" {
    // 1. 如果离线的是当前选中 node
    if active_node_id == Some(node_id) {
        // 2. 尝试切换到另一个 online 的 node
        if let Some(backup) = nodes.iter().find(|n| n.status == "online") {
            active_node_id.set(Some(backup.node_id.clone()));
        } else {
            active_node_id.set(None);  // 无可用 node
        }
    }
    // 3. 关闭该 node 的 DP 连接
    dp_pool.write().remove(&node_id);
    // 4. 清理缓存
    node_data_cache.write().invalidate(&node_id);
}
```

### 6.4 Node 重连后恢复

```rust
// CP heartbeat 检测到 node 重新上线
if node.status == "online" && prev_status == "offline" {
    // 1. 重新建立 DP 连接（懒创建，用户切换回来时才创建）
    // 2. 缓存已被 invalidate，下次访问时会重新加载
}
```

### 6.5 边界情况

**Edge case 1：首次加载时没有 online node**

```rust
// 所有 node 都是 offline
if nodes.iter().all(|n| n.status != "online") {
    active_node_id.set(None);
    // Tab 显示 "No node selected" 占位符
    // Nodes dropdown 显示所有 node（offline 标记为红色）
    // 用户可以手动点击 offline node，但 tab 显示 "Node offline, waiting for reconnection..."
}
```

**Edge case 2：用户手动选择了 offline node**

```rust
// 允许选择 offline node（用户可能想查看缓存数据）
active_node_id.set(Some(offline_node_id));

// Tab 检查 DP 连接是否存在
if dp_pool.get(node_id).is_none() {
    // 显示缓存数据（如果有）+ "Node offline" 提示
    if let Some(cached) = node_data_cache.get(node_id) {
        render_with_offline_banner(cached);
    } else {
        rsx! { div { "Node offline, no cached data" } }
    }
}
```

**Edge case 3：快速切换 node**

用户在加载过程中切换到另一个 node → 取消当前加载（通过 `CancellationToken` 或检查 `active_node_id` 是否变化）

**Edge case 4：多个 tab 同时加载**

使用 `Promise.all` 并行加载，但限制并发数（如最多 3 个请求）

**Edge case 5：缓存大小**

如果 node 数量很多（>10），考虑 LRU 淘汰策略（先不实现，后续按需添加）

如果 node 数量很多（>10），考虑 LRU 淘汰策略（先不实现，后续按需添加）

---

## 7. Node Detail UI（CP 视角）

### 7.1 入口

在 Nodes dropdown 中，点击 node 卡片的 name 进入 Node Detail UI。

### 7.2 布局

```
┌──────────────────────────────────────────────────────────┐
│ StatusBar: ● CP  │ ● DP: node-A  │ ▾ Nodes(3)          │
├──────────────────────────────────────────────────────────┤
│ ┌─ Node Detail: node-A ──────────────────────────────┐  │
│ │  ← Back to DP View                                  │  │
│ │                                                      │  │
│ │  ## Overview                                         │  │
│ │  ┌──────────────────────────────────────────────┐   │  │
│ │  │ Node ID:  node-A                              │   │  │
│ │  │ Name:     Ansible Playbook Runner             │   │  │
│ │  │ Version:  0.1.0                               │   │  │
│ │  │ Status:   ● online                            │   │  │
│ │  │ WS URL:   ws://node-a.vol.bestnathan.top/ws   │   │  │
│ │  │ Last seen: 2026-07-23 14:32:15                │   │  │
│ │  └──────────────────────────────────────────────┘   │  │
│ │                                                      │  │
│ │  ## Resource Usage                                   │  │
│ │  ┌──────────────────────────────────────────────┐   │  │
│ │  │ CPU:    12%  ████████░░░░░░░░░░░░░░          │   │  │
│ │  │ Memory: 256MB ████████████░░░░░░░░░░          │   │  │
│ │  └──────────────────────────────────────────────┘   │  │
│ │                                                      │  │
│ │  ## Agents on this Node                              │  │
│ │  ┌──────────────────────────────────────────────┐   │  │
│ │  │ ● advice-agent    repo   Running              │   │  │
│ │  │ ● coding-agent    repo   Idle                 │   │  │
│ │  │ ● qa-agent        user   Idle                 │   │  │
│ │  └──────────────────────────────────────────────┘   │  │
│ │                                                      │  │
│ │  ## Capabilities                                     │  │
│ │  ┌──────────────────────────────────────────────┐   │  │
│ │  │ Tools:   bash, read, write, grep (12 total)   │   │  │
│ │  │ Skills:  debug, refactor, test (3 total)       │   │  │
│ │  │ MCP:     filesystem, git (2 servers)           │   │  │
│ │  └──────────────────────────────────────────────┘   │  │
│ └──────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

### 7.3 数据来源

| Section | 数据源 | 协议操作 |
|---------|--------|----------|
| Overview | `NodeRecord` | `control.node_get(node_id)` |
| Resource Usage | `NodeRecord.load` | 同上（CPU/memory 在 heartbeat 中更新） |
| Agents on this Node | 过滤后的 agent 列表 | `agent.list` (CP) → 过滤 `node_id == selected` |
| Capabilities | `CapabilitySnapshot` | `control.capability_list(node_id)` |

### 7.4 状态管理

```rust
// AppState 新增
pub struct AppState {
    // ... 现有字段 ...
    pub viewing_node_detail: Signal<Option<String>>,  // None = DP view, Some(node_id) = Node Detail view
}

// NodeDetailState
#[derive(Debug, Clone, Default)]
pub struct NodeDetailState {
    pub node: Option<NodeRecord>,
    pub agents: Vec<AgentListEntry>,
    pub capabilities: Option<CapabilitySnapshot>,
    pub loading: bool,
    pub error: Option<String>,
}
```

### 7.5 交互

1. **进入 Node Detail：** 点击 node 卡片的 name → `viewing_node_detail.set(Some(node_id))` → 路由到 NodeDetailPanel
2. **返回 DP View：** 点击 "← Back" → `viewing_node_detail.set(None)` → 路由回主 tab 视图
3. **切换 node：** 在 Node Detail 中点击其他 node → 更新 `viewing_node_detail` + 重新加载数据
4. **自动刷新：** 每 5 秒轮询 `control.node_get` 更新 resource usage（或等 CP push heartbeat 事件）

### 7.6 组件结构

```rust
// app.rs TabContent 路由
fn TabContent() -> Element {
    let viewing_node = use_context::<Signal<Option<String>>>();
    
    if let Some(ref node_id) = *viewing_node.read() {
        rsx! { NodeDetailPanel { node_id: node_id.clone() } }
    } else {
        // 正常 DP tab 路由
        match active_tab {
            ActiveTab::Files => rsx! { FileTree {} },
            // ...
        }
    }
}
```

---

## 8. 错误处理

### 8.1 错误场景

| 场景 | 处理方式 |
|------|----------|
| **CP 连接断开** | Status bar 显示 "CP disconnected"，Nodes dropdown 禁用，所有 tab 显示 "Waiting for CP reconnection..." |
| **DP 连接断开** | Tab 显示 "Node disconnected, retrying..."，3 秒后自动重连，3 次失败后显示 "Node unreachable" |
| **Node 列表为空** | Nodes dropdown 显示 "No nodes registered"，tab 显示 "Waiting for first node..." |
| **选中的 node 离线** | 自动切换到另一个 online node（如果有），否则 `active_node_id = None`，tab 显示 "No node selected" |
| **DP 请求失败（如 file_list 超时）** | Tab 显示错误消息 + "Retry" 按钮，不写入缓存 |
| **缓存数据过期（node 重连后）** | 自动 invalidate 缓存，重新加载 |

### 8.2 错误 UI 组件

```rust
// 通用错误组件
#[component]
fn TabError(message: String, on_retry: EventHandler<()>) -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center h-full gap-3",
            div { class: "text-[#ff6060] text-[14px]", "{message}" }
            button {
                class: "px-4 py-1.5 bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                onclick: move |_| on_retry.call(()),
                "Retry"
            }
        }
    }
}

// 离线提示 banner
#[component]
fn OfflineBanner() -> Element {
    rsx! {
        div { class: "bg-[#3a2020] text-[#ff8080] px-3 py-1 text-[12px] text-center",
            "⚠ Node offline — showing cached data"
        }
    }
}
```

### 8.3 重试策略

```rust
// Tab 加载失败时的重试
fn load_with_retry<F>(load_fn: F, max_attempts: u32)
where
    F: Fn() -> Future<Output = Result<Data, String>>,
{
    spawn_local(async move {
        for attempt in 1..=max_attempts {
            match load_fn().await {
                Ok(data) => {
                    // 写入缓存
                    return;
                }
                Err(e) => {
                    if attempt == max_attempts {
                        // 显示错误
                        set_error(e);
                    } else {
                        // 等待 2^attempt 秒后重试
                        sleep(Duration::from_secs(2u64.pow(attempt - 1))).await;
                    }
                }
            }
        }
    });
}
```

---

## 9. 测试策略

### 9.1 单元测试（state/mod.rs）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_data_cache_get_returns_none_for_missing_node() {
        let cache = NodeDataCache::default();
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn node_data_cache_get_or_insert_creates_entry() {
        let mut cache = NodeDataCache::default();
        let data = cache.get_or_insert("node-A");
        data.files = Some(WorkspaceState::new("."));
        
        assert!(cache.get("node-A").is_some());
        assert!(cache.get("node-A").unwrap().files.is_some());
    }

    #[test]
    fn node_data_cache_invalidates_removes_entry() {
        let mut cache = NodeDataCache::default();
        cache.get_or_insert("node-A");
        cache.invalidate("node-A");
        
        assert!(cache.get("node-A").is_none());
    }
}
```

### 9.2 集成测试（tests/node_selection.rs）

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn switching_nodes_updates_active_node_id() {
        let mut app = create_test_app();
        
        // 模拟 node 列表
        app.set_nodes(vec![
            NodeListEntry { node_id: "node-A".into(), status: "online".into(), .. },
            NodeListEntry { node_id: "node-B".into(), status: "online".into(), .. },
        ]);
        
        // 初始自动选中第一个
        assert_eq!(app.active_node_id(), Some("node-A"));
        
        // 切换到 node-B
        app.select_node("node-B");
        assert_eq!(app.active_node_id(), Some("node-B"));
    }

    #[test]
    fn cached_data_persists_across_node_switches() {
        let mut app = create_test_app();
        app.set_nodes(vec![node_a(), node_b()]);
        
        // 加载 node-A 的文件
        app.load_files_for_node("node-A");
        assert!(app.get_cached_files("node-A").is_some());
        
        // 切换到 node-B
        app.select_node("node-B");
        app.load_files_for_node("node-B");
        
        // 切回 node-A，缓存仍在
        app.select_node("node-A");
        assert!(app.get_cached_files("node-A").is_some());
    }

    #[test]
    fn offline_node_triggers_auto_switch() {
        let mut app = create_test_app();
        app.set_nodes(vec![
            node_online("node-A"),
            node_online("node-B"),
        ]);
        app.select_node("node-A");
        
        // node-A 离线
        app.mark_node_offline("node-A");
        
        // 自动切换到 node-B
        assert_eq!(app.active_node_id(), Some("node-B"));
    }
}
```

### 9.3 覆盖率目标

| 模块 | 覆盖率目标 |
|------|-----------|
| `NodeDataCache` | ≥ 90% |
| `NodesDropdown` | ≥ 80% |
| Tab 路由逻辑 | ≥ 80% |
| 错误处理 | ≥ 70% |

---

## 10. 实施计划

### 10.1 Phase 1: 基础设施（2-3 天）

1. 实现 `NodeDataCache` 结构和单元测试
2. 在 `AppState` 中添加 `node_data_cache` 和 `viewing_node_detail` 字段
3. 改造 `DpConnectionPool`，支持 event loop 自动启动

### 10.2 Phase 2: Node 选择 UI（2-3 天）

1. 实现 `NodesDropdown` 组件（收起/展开状态）
2. 集成到 `StatusBar`
3. 实现初始加载时的自动选中逻辑
4. 实现 node 离线时的自动切换

### 10.3 Phase 3: Tab 数据路由改造（3-4 天）

1. 改造 `FileTree` 从 `NodeDataCache` 读取
2. 改造 `McpPanel`、`ToolsTab`、`TasksPanel`、`SkillsPanel`、`LogViewer`
3. 为每个 DP 连接启动独立的 event loop
4. 实现缓存失效和重新加载逻辑

### 10.4 Phase 4: Node Detail UI（2-3 天）

1. 实现 `NodeDetailPanel` 组件
2. 实现 `control.node_get` 和 `control.capability_list` 调用
3. 实现 "← Back" 导航
4. 实现自动刷新（轮询或 heartbeat 监听）

### 10.5 Phase 5: 错误处理和测试（2 天）

1. 实现错误 UI 组件（`TabError`、`OfflineBanner`）
2. 实现重试逻辑
3. 编写集成测试
4. 达到覆盖率目标

**总计：11-15 天**

---

## 11. 后续扩展

- **Node 配置编辑：** 在 Node Detail UI 中添加编辑功能
- **跨 node 数据聚合：** 如"所有 node 的 tasks 列表"
- **Node 间数据迁移：** 将 agent 或 task 从一个 node 迁移到另一个
- **Node 性能监控：** 添加 CPU/memory 历史图表
- **LRU 缓存淘汰：** 当 node 数量 >10 时，淘汰最久未访问的缓存

---

## 12. 附录

### 12.1 CP 协议可用数据

```rust
// NodeRecord (from CP)
pub struct NodeRecord {
    pub node_id: String,
    pub name: String,
    pub version: String,
    pub status: String,
    pub last_seen_at_ms: Option<u64>,
    pub capability_revision: u64,
    pub load: NodeLoad,
}

// 可用操作
- control.node_list → Vec<NodeRecord>
- control.node_get(node_id) → Option<NodeRecord>
- control.capability_list(node_id) → Vec<CapabilitySnapshot>
```

### 12.2 测试 fixtures

```rust
fn mock_node_online(node_id: &str) -> NodeListEntry {
    NodeListEntry {
        node_id: node_id.into(),
        name: format!("Test Node {}", node_id),
        version: "0.1.0".into(),
        status: "online".into(),
        agent_count: Some(3),
        load: Some(NodeLoadInfo { cpu: Some(12.0), memory_mb: Some(256) }),
    }
}
```
