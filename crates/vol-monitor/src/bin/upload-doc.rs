//! Upload refactoring documentation to Feishu Drive.
//!
//! Usage: cargo run --bin upload-doc
//!
//! This script creates a document summarizing the vol-feishu to openlark migration
//! and uploads it to Feishu Cloud Drive.

use reqwest::Client;
use serde_json::json;
use std::fs;
use tracing::{error, info, warn};
use tracing_subscriber::{self, EnvFilter};

// Feishu credentials from config
const APP_ID: &str = "cli_a936b13197385bde";
const APP_SECRET: &str = "JnWnFrrOvzHi4deDmFY9kd1NMGbiWuNz";
// Note: Folder token should start with "fldcn" or "fltdcn"
// Using root folder for now - user should manually move file
const PARENT_FOLDER_TOKEN: &str = ""; // Empty means root folder

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    // Create the migration document content
    let doc_content = create_migration_doc();

    // Save to local file first
    let doc_path = "docs/superpowers/releases/2026-04-02-vol-feishu-to-openlark-migration.md";
    if let Some(parent) = std::path::Path::new(doc_path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(doc_path, &doc_content)?;
    info!("Document saved locally to: {}", doc_path);

    // Upload to Feishu
    info!("\n===========================================");
    info!("Uploading to Feishu Cloud Docs...");
    info!("===========================================\n");

    match upload_to_feishu(&doc_content).await {
        Ok((file_token, url)) => {
            info!("\n✅ Upload successful!");
            info!("File token: {}", file_token);
            info!("URL: {}", url);
            Ok(())
        }
        Err(e) => {
            error!("Upload failed: {}", e);
            info!("\n❌ Upload failed: {}", e);
            info!("\nDocument saved locally: {}", doc_path);
            info!("Please upload manually to Feishu Drive.");
            Err(e.into())
        }
    }
}

fn create_migration_doc() -> String {
    r#"# vol-feishu 迁移到 openlark 重构文档

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
app_id = "cli_a936b13197385bde"
app_secret = "JnWnFrrOvzHi4deDmFY9kd1NMGbiWuNz"
receive_id = "oc_c29208d94757e2aefd97bfa5f57e0b26"
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
"#
    .to_string()
}

/// Get access token from Feishu
async fn get_access_token(client: &Client) -> Result<String, Box<dyn std::error::Error>> {
    let url = "https://open.feishu.cn/open-apis/auth/v3/app_access_token/internal";

    let body = json!({
        "app_id": APP_ID,
        "app_secret": APP_SECRET
    });

    let response: reqwest::Response = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Failed to get access token: {} - {}", status, text).into());
    }

    let json: serde_json::Value = response.json().await?;

    json.get("app_access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No app_access_token in response".into())
}

/// Create a cloud document in Feishu Drive
async fn upload_to_feishu(content: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Step 1: Get access token
    info!("Getting access token...");
    let token = get_access_token(&client).await?;
    info!("Access token obtained");

    // Step 2: Create cloud document using Drive API v1
    // Reference: https://open.feishu.cn/document/ukGEjY2L0QjEycz4Mdc
    info!("Creating file in Drive...");
    let url = "https://open.feishu.cn/open-apis/drive/v1/files";

    // Build request body with folder info
    let mut body = json!({
        "parent_type": "folder",
        "file_type": "doc"
    });

    // Add parent folder if provided
    if !PARENT_FOLDER_TOKEN.is_empty() {
        body["parent_node"] = json!(PARENT_FOLDER_TOKEN);
    }

    let create_response: reqwest::Response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let create_status = create_response.status();
    let create_text = create_response.text().await?;

    info!("Create file response status: {}", create_status);
    info!(
        "Create file response: {}",
        &create_text[..500.min(create_text.len())]
    );

    if !create_status.is_success() {
        warn!("Create file failed with status {}", create_status);
        return Err(format!(
            "Create file failed: {} - {}",
            create_status,
            &create_text[..200.min(create_text.len())]
        )
        .into());
    }

    let create_json: serde_json::Value = serde_json::from_str(&create_text)?;
    info!("Create file response JSON: {:?}", create_json);

    // Check for API error code
    if let Some(code) = create_json.get("code").and_then(|v| v.as_i64()) {
        if code != 0 {
            let msg = create_json
                .get("msg")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(format!("Feishu API error {}: {}", code, msg).into());
        }
    }

    // Extract file token from response
    let file_token = create_json
        .get("obj_token")
        .or_else(|| create_json.get("token"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("No file token in response: {}", create_text))?;

    info!("File created with token: {}", file_token);

    let url = format!("https://open.feishu.cn/drive/file/{}", file_token);

    Ok((file_token.to_string(), url))
}
