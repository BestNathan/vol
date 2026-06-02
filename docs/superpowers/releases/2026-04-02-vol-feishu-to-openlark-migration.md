# vol-feishu 迁移到 openlark 重构文档

## 提交信息

- **Commit**: bc18dff3b35f02dab51d7b1cc3fe6fb263fb1d6b
- **日期**: 2026-04-02
- **作者**: BestNathan <nathan@quantitative.com>

## 改动概述

将飞书通知功能从自研的 `vol-feishu` crate 迁移到官方 `openlark` SDK。

## 变更文件统计

```
9 files changed, 1434 insertions(+), 566 deletions(-)
```

### 删除的文件
- `crates/vol-feishu/Cargo.toml`
- `crates/vol-feishu/src/client.rs`
- `crates/vol-feishu/src/lib.rs`

### 修改的文件
- `Cargo.toml` - 从 workspace 移除 vol-feishu
- `Cargo.lock` - 依赖更新
- `crates/vol-notification/Cargo.toml` - 添加 reqwest 依赖
- `crates/vol-notification/src/feishu.rs` - 使用 openlark + reqwest 重写
- `crates/vol-monitor/Cargo.toml` - 依赖更新
- `crates/vol-monitor/src/bin/upload-doc.rs` - 修复 openlark 导入

## 技术实现

### 认证方式

使用 OAuth 2.0 Client Credentials Flow:

```http
POST /open-apis/auth/v3/app_access_token/internal
Content-Type: application/json

{
  "app_id": "cli_xxx",
  "app_secret": "xxx"
}

Response:
{
  "code": 0,
  "app_access_token": "xxx",
  "expire": 7200
}
```

### 消息发送

```http
POST /open-apis/im/v1/messages
Authorization: Bearer {access_token}
Content-Type: application/json

{
  "receive_id": "oc_xxx",
  "receive_id_type": "chat_id",
  "msg_type": "interactive_text",
  "content": "{...}"
}
```

### 支持的消息类型

1. **Interactive Card** (交互式卡片) - 首选格式
   - 红色标题头
   - 格式化表格显示告警详情
   - 支持中文标题和标签

2. **Text Message** (文本消息) - 降级格式
   - 当卡片发送失败时自动降级
   - 使用配置的消息模板

### 接收者类型支持

| ID 前缀 | 类型 | 说明 |
|---------|------|------|
| `oc_*` | chat_id | 群聊 |
| `ou_*` | open_id | 用户 |
| `og_*` | group_id | 群组 |

## 配置示例

```toml
[notifications.feishu]
app_id = "<your-feishu-app-id>"
app_secret = "<your-feishu-app-secret>"
receive_id = "<your-feishu-receive-id>"
message_template = "🚨 {tenor} {alert_type}: {symbol} | IV={value:.1}%"
```

## 代码结构

### FeishuNotification 结构体

```rust
pub struct FeishuNotification {
    app_id: String,
    app_secret: String,
    receive_id: String,
    message_template: String,
    http_client: reqwest::Client,
}
```

### 核心方法

| 方法 | 说明 |
|------|------|
| `get_access_token()` | 获取 OAuth 访问令牌 |
| `format_message()` | 格式化文本消息 |
| `format_interactive_card()` | 格式化交互式卡片 |
| `send_message()` | 发送消息到飞书 API |
| `send()` | NotificationHandler trait 实现 |

## 依赖变更

### vol-notification/Cargo.toml

```toml
[dependencies]
tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
chrono = "0.4"
reqwest = { workspace = true }      # 新增
openlark = { workspace = true }     # 替代 vol-feishu
vol-core = { workspace = true }
vol-config = { workspace = true }
vol-deribit = { workspace = true }
```

## 构建验证

```bash
cargo build --release
# ✓ 编译成功
```

## 迁移优势

| 优势 | 说明 |
|------|------|
| 官方支持 | openlark 是飞书官方 SDK |
| 维护活跃 | 持续更新和 bug 修复 |
| 文档完善 | 完整的 API 参考和示例 |
| 类型安全 | Rust 原生类型定义 |
| 错误处理 | 统一的 SDKResult 错误类型 |

## 后续计划

- [ ] 考虑添加文档上传功能（需要 openlark 支持 Drive API）
- [ ] 实现消息发送重试机制
- [ ] 添加消息发送成功率监控

---

*此文档由 upload-doc 脚本自动生成*
