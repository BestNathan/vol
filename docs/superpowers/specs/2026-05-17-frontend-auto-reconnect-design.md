# Frontend Auto-Reconnect Design

## Problem

WebSocket 连接断开后，前端没有重连机制，客户端完全失效，用户必须刷新页面。

## Solution

WebSocket 断开时自动指数退避重连，重连成功后自动恢复最近一次会话的对话历史。

### 改动范围

1. **`client.rs`** — 添加 `reconnect()` 方法，断开时启动指数退避重连循环（3s → 6s → 12s，最大 30s）
2. **`app.rs`** — 重连成功后调用 `session.list` → `session.resume`（最近会话）→ `session.entries` → 重建 `conversation_signal`

### 关键细节

- 重连次数上限 10 次，超过后停止重连，显示"连接失败，请刷新页面"
- 重连成功后 auto-subscribe agent events（现有逻辑已支持）
- 恢复会话只取 `session.list` 第一条（按时间排序的最新会话）
- 重连过程中 UI 显示 "Reconnecting... (3s)" 倒计时提示
- 如果没有任何持久化会话，重连后正常连接即可

### 用户体验

| 阶段 | UI 显示 |
|------|---------|
| 正常连接 | 绿色连接状态 |
| 断开 | 红色 "Disconnected" |
| 重连中 | 黄色 "Reconnecting... (3s)" |
| 重连成功 | 自动恢复最近会话 |
| 重连失败(10次) | "Connection lost. Please refresh." |
