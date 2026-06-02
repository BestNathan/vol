# Requirements: Agent Manager Frontend (React + TypeScript + Vite)

## Background
Agent manager 已经实现了完整的后端服务（agent 发现、实例化、WebSocket 路由、REST API），但缺乏用户界面。需要构建一个前端应用来：
1. 管理和监控 agent 实例（管理控制台）
2. 与 agent 进行实时交互（用户交互界面）

## Goals

1. **Agent 管理** — 查看可用 agent 类型、创建/销毁 agent 实例、查看实例运行状态和健康指标
2. **Agent 交互** — 通过 WebSocket 与 agent 进行实时对话，支持多 session 并发
3. **事件监控** — 通过 SSE 接收 agent manager 事件，实时更新界面状态
4. **动态适配** — 根据 `/api/v1/agent-types` 返回的数据自动渲染 agent 列表和详情面板，不硬编码 agent 类型
5. **独立部署** — 构建产物通过 Nginx 静态托管，反向代理到 agent-manager 服务

## Non-Goals

1. **不动态生成交互表单** — agent frontmatter 不做扩展，所有 agent 使用统一的聊天界面
2. **不实现用户认证** — 前端本身不含登录/注册，依赖 agent-manager 侧的网络隔离
3. **不实现 agent 编辑** — agent 定义文件（.md）的创建和修改通过编辑器完成，不在前端操作
4. **不实现任务编排 UI** — 子任务分叉、父 session 等概念在后端处理，前端只展示结果
5. **不实现移动端原生 App** — 仅 Web SPA，需响应式布局但不做 PWA/原生封装

## Scope

### Included
- React + TypeScript + Vite SPA 项目（新建在 `frontend/` 目录）
- Ant Design 组件库
- React Router 用于页面路由
- 自定义 WebSocket hook 连接 `/ws/agents/:agent_type/session/:session_id`
- SSE 事件订阅 `/api/v1/events`
- REST API 调用封装：
  - `GET /api/v1/agent-types` — 获取可用 agent 类型
  - `GET /api/v1/agent-instances` — 获取运行实例列表
  - `DELETE /api/v1/agent-instances/:type/:session_id` — 销毁实例
  - `GET /health` — 健康检查
  - `GET /metrics` — Prometheus 指标
- 页面结构：
  - **首页/Dashboard** — 概览（agent 类型数量、运行实例数、健康状态）
  - **Agent 类型列表** — 展示所有可用 agent 类型及描述
  - **Agent 实例管理** — 创建新实例（选择类型 + 输入 session ID）、查看运行中实例、销毁实例
  - **Agent 对话** — 选择/创建 session，通过 WebSocket 实时对话，展示聊天历史
  - **事件流** — SSE 实时显示 agent manager 事件
- Nginx 配置文件（`frontend/nginx.conf`）：
  - 静态文件服务
  - `/api/` 和 `/ws/` 反向代理到 agent-manager（同一域名下避免跨域）
- Dockerfile 构建前端产物（可选，独立于 agent-manager 的 Dockerfile）

### Excluded
- 后端 API 改动（使用现有 API，不修改 agent-manager）
- 用户认证/权限系统
- agent 定义文件的在线编辑
- 持久化聊天历史（依赖 agent-manager 的 session store）

## Constraints

1. **技术栈** — React 18 + TypeScript + Vite + Ant Design 5.x + React Router 6.x
2. **网络** — 部署环境存在网络限制（需要 HTTP 代理），前端构建产物为纯静态文件
3. **端口** — agent-manager 监听 8080，Nginx 监听 80/443
4. **CORS** — Nginx 反向代理解决跨域，前端开发时通过 Vite proxy 配置
5. **AgentLoader 动态性** — agent 类型列表在运行时通过 API 获取，前端不预定义任何 agent 类型

## Success Criteria

1. `cd frontend && npm run build` 成功，产物目录 < 5MB（gzip 后 < 1MB）
2. Nginx 容器能同时服务静态文件和反向代理到 `http://vol-agent-manager:8080`
3. 前端页面能在浏览器中完成以下操作：
   - 列出所有 agent 类型（name, type, description）
   - 创建新 agent 实例（选择类型，输入 session ID）
   - 查看运行中实例列表及其状态
   - 销毁指定实例
   - 连接 WebSocket 并发送消息，实时收到 agent 回复
   - 同时打开多个 session 独立对话
4. WebSocket 断连后自动重连（< 5 秒恢复）
5. 页面首屏加载 < 2s（局域网环境）

## Edge Cases

1. **agent-manager 未启动** — 前端显示服务不可用提示，提供重试按钮
2. **agent 类型为空** — 显示空状态引导用户添加 agent 定义文件
3. **WebSocket 连接失败** — 显示错误提示，自动重连（指数退避，最大 30 秒）
4. **Session 冲突** — 两个用户同时连接同一 session ID，后端已支持广播，前端需显示多用户连接提示
5. **agent 运行失败** — 后端返回 `agent_error` 消息，前端需展示错误信息并允许重新发送
6. **大量事件** — SSE 事件流可能很频繁，前端需限制显示数量（最近 100 条）或提供过滤

## Open Questions

无。
