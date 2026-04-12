# Session 包重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Session 相关逻辑重构为独立的 `vol-session` 包，专注于会话消息的记录和还原。

**Architecture:** 创建独立 vol-session 包，包含 FileMessageStore 实现 JSONL 存储，SessionListener 订阅 event bus 过滤关键事件。

**Tech Stack:** Rust, Tokio async runtime, broadcast channels, JSONL file format

---

## 文件结构

### 新建文件

| 文件 | 说明 |
|------|------|
| `crates/vol-session/Cargo.toml` | 包配置 |
| `crates/vol-session/src/lib.rs` | 包入口和导出 |
| `crates/vol-session/src/message.rs` | SessionMessage 类型（从 vol-llm-agent 移动） |
| `crates/vol-session/src/session.rs` | Session 容器（从 vol-llm-agent 移动） |
| `crates/vol-session/src/store.rs` | Store traits（从 vol-llm-agent 移动） |
| `crates/vol-session/src/memory_store.rs` | InMemory 实现（从 vol-llm-agent 移动） |
| `crates/vol-session/src/file_store.rs` | **新增** JSONL 文件存储实现 |
| `crates/vol-session/src/listener.rs` | **新增** SessionListener 事件监听器 |
| `crates/vol-session/README.md` | 包文档 |

### 修改文件

| 文件 | 修改说明 |
|------|----------|
| `crates/vol-llm-agent/Cargo.toml` | 添加 `vol-session` 依赖 |
| `crates/vol-llm-agent/src/lib.rs` | 导出改为 re-export vol-session 的类型 |
| `crates/vol-llm-agent/src/session/mod.rs` | 改为 re-export vol-session |
| `crates/vol-llm-agent/src/react/agent.rs` | 集成 SessionListener |

---

## Task 1: 创建 vol-session 包骨架

**Files:**
- Create: `crates/vol-session/Cargo.toml`
- Create: `crates/vol-session/src/lib.rs`
- Create: `crates/vol-session/README.md`

- [ ] **Step 1: 创建 vol-session 目录结构**

```bash
mkdir -p crates/vol-session/src
```

- [ ] **Step 2: 创建 Cargo.toml**

```toml
[package]
name = "vol-session"
version.workspace = true
edition.workspace = true

[dependencies]
tokio = { workspace = true }
tracing = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
async-trait = { workspace = true }
thiserror = { workspace = true }
uuid = { workspace = true }
vol-llm-core = { workspace = true }
vol-core = { workspace = true }
chrono = "0.4"
```

- [ ] **Step 3: 创建 src/lib.rs**

```rust
//! vol-session: Session management and message persistence.
//!
//! Provides session management and message persistence for ReAct Agent.
//!
//! # Architecture
//!
//! - `SessionMessage` - Wrapper around `core::Message` with session context
//! - `Session` - Session container with store references
//! - `SessionStore` / `MessageStore` - Storage traits
//! - `FileMessageStore` - JSONL file storage implementation
//! - `SessionListener` - Event listener for recording key events

pub mod message;
pub mod session;
pub mod store;
pub mod memory_store;
pub mod file_store;
pub mod listener;

pub use message::SessionMessage;
pub use session::Session;
pub use store::{SessionStore, MessageStore};
pub use memory_store::{InMemorySessionStore, InMemoryMessageStore};
pub use file_store::FileMessageStore;
pub use listener::SessionListener;
```

- [ ] **Step 4: 创建 README.md**

```markdown
# vol-session

Session management and message persistence for ReAct Agent.

## Features

- Session lifecycle management
- Message persistence with JSONL format
- Event-driven recording via SessionListener
- In-memory and file-based storage backends

## Usage

```rust
use vol_session::{Session, FileMessageStore, SessionListener};

// Create session
let session = Session::new(
    "session-123".to_string(),
    Arc::new(InMemorySessionStore::new()),
    Arc::new(FileMessageStore::new("logs/sessions", "session-123")?),
);

// Create listener
let listener = SessionListener::new(
    event_rx,
    Arc::new(FileMessageStore::new("logs/sessions", "session-123")?),
    "session-123".to_string(),
);
tokio::spawn(listener.run());
```
```

- [ ] **Step 5: 验证包骨架**

```bash
cd crates/vol-session && cargo check
```

Expected: Compiles (empty modules)

- [ ] **Step 6: 提交**

```bash
git add crates/vol-session/
git commit -m "feat: create vol-session package skeleton"
```

---

## Task 2: 移动 SessionMessage 到 vol-session

**Files:**
- Create: `crates/vol-session/src/message.rs`
- Modify: `crates/vol-session/src/lib.rs`

- [ ] **Step 1: 复制 message.rs 内容**

从 `crates/vol-llm-agent/src/session/message.rs` 复制到 `crates/vol-session/src/message.rs`：

```rust
//! Session message wrapper.
//!
//! Wraps `vol_llm_core::Message` with session-related fields.

use std::collections::HashMap;
use vol_llm_core::Message;

/// Session message wrapper
#[derive(Clone, Debug)]
pub struct SessionMessage {
    /// Message unique ID (UUID)
    pub id: String,

    /// Session ID this message belongs to
    pub session_id: String,

    /// Core message body
    pub message: Message,

    /// Parent message ID, supports tree conversation structure
    pub parent_id: Option<String>,

    /// Creation timestamp (Unix seconds)
    pub created_at: i64,

    /// Metadata for extensible purposes
    pub metadata: HashMap<String, String>,
}

impl SessionMessage {
    /// Create a new session message
    pub fn new(session_id: String, message: Message) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            message,
            parent_id: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            metadata: HashMap::new(),
        }
    }

    /// Set parent message ID
    pub fn with_parent_id(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_message_creation() {
        let msg = SessionMessage::new(
            "session-123".to_string(),
            Message::user("Hello"),
        );

        assert_eq!(msg.session_id, "session-123");
        assert!(msg.parent_id.is_none());
        assert!(!msg.id.is_empty());
    }

    #[test]
    fn test_session_message_with_parent() {
        let msg = SessionMessage::new(
            "session-123".to_string(),
            Message::user("Reply"),
        ).with_parent_id("msg-456".to_string());

        assert_eq!(msg.parent_id, Some("msg-456".to_string()));
    }

    #[test]
    fn test_session_message_metadata() {
        let msg = SessionMessage::new(
            "session-123".to_string(),
            Message::user("Test"),
        ).with_metadata("user_id", "user-1");

        assert_eq!(msg.metadata.get("user_id"), Some(&"user-1".to_string()));
    }
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p vol-session
```

Expected: Compiles successfully

- [ ] **Step 3: 运行测试**

```bash
cargo test -p vol-session --lib message
```

Expected: 3 tests pass

- [ ] **Step 4: 提交**

```bash
git add crates/vol-session/src/message.rs
git commit -m "feat: add SessionMessage to vol-session"
```

---

## Task 3: 移动 Session 和 Store traits 到 vol-session

**Files:**
- Create: `crates/vol-session/src/session.rs`
- Create: `crates/vol-session/src/store.rs`
- Create: `crates/vol-session/src/memory_store.rs`

- [ ] **Step 1: 复制 store.rs**

从 `crates/vol-llm-agent/src/session/store.rs` 复制到 `crates/vol-session/src/store.rs`：

```rust
//! Session and Message store traits.

use async_trait::async_trait;
use vol_llm_core::Result;
use super::message::SessionMessage;
use super::session::Session;

/// Session storage interface
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: Session) -> Result<()>;
    async fn get(&self, session_id: &str) -> Result<Option<Session>>;
    async fn delete(&self, session_id: &str) -> Result<()>;
    async fn update(&self, session: Session) -> Result<()>;
}

/// Message storage interface
#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn save(&self, message: SessionMessage) -> Result<()>;
    async fn get_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>>;
    async fn get_before(&self, session_id: &str, before: i64, limit: usize) -> Result<Vec<SessionMessage>>;
    async fn delete_session(&self, session_id: &str) -> Result<()>;
    async fn update(&self, id: &str, message: SessionMessage) -> Result<()>;
    async fn get_count(&self, session_id: &str) -> Result<usize>;
    async fn cleanup_expired(&self, before: i64) -> Result<()>;
}
```

- [ ] **Step 2: 复制 session.rs**

从 `crates/vol-llm-agent/src/session/session.rs` 复制到 `crates/vol-session/src/session.rs`：

```rust
//! Session management.

use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::Result;
use super::message::SessionMessage;
use super::store::{SessionStore, MessageStore};

/// Session management
pub struct Session {
    pub id: String,
    pub created_at: i64,
    pub metadata: HashMap<String, String>,
    session_store: Arc<dyn SessionStore>,
    message_store: Arc<dyn MessageStore>,
}

impl Session {
    pub fn new(
        id: String,
        session_store: Arc<dyn SessionStore>,
        message_store: Arc<dyn MessageStore>,
    ) -> Self {
        Self {
            id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            metadata: HashMap::new(),
            session_store,
            message_store,
        }
    }

    pub async fn get_messages(&self, limit: usize) -> Result<Vec<SessionMessage>> {
        self.message_store.get_by_session(&self.id, limit).await
    }

    pub async fn add_message(&self, message: SessionMessage) -> Result<()> {
        self.message_store.save(message).await
    }

    pub async fn get_or_create_parent(&self, parent_id: &str) -> Option<Session> {
        self.session_store.get(parent_id).await.ok().flatten()
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            created_at: self.created_at,
            metadata: self.metadata.clone(),
            session_store: self.session_store.clone(),
            message_store: self.message_store.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;
    use crate::memory_store::{InMemorySessionStore, InMemoryMessageStore};

    #[tokio::test]
    async fn test_session_get_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            message_store.clone(),
        );

        let msg = SessionMessage::new(
            "session-1".to_string(),
            Message::user("Hello"),
        );
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages(10).await.unwrap();
        assert_eq!(messages.len(), 1);
    }
}
```

- [ ] **Step 3: 复制 memory_store.rs**

从 `crates/vol-llm-agent/src/session/memory_store.rs` 复制到 `crates/vol-session/src/memory_store.rs`

- [ ] **Step 4: 验证编译**

```bash
cargo check -p vol-session
```

Expected: Compiles successfully

- [ ] **Step 5: 提交**

```bash
git add crates/vol-session/src/{session,store,memory_store}.rs
git commit -m "feat: move Session and Store traits to vol-session"
```

---

## Task 4: 实现 FileMessageStore

**Files:**
- Create: `crates/vol-session/src/file_store.rs`
- Test: `crates/vol-session/tests/file_store_test.rs`

- [ ] **Step 1: 创建 FileMessageStore**

```rust
//! JSONL file-based message store.

use std::path::{Path, PathBuf};
use std::sync::RwLock;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::fs::{self, File, OpenOptions};
use serde::{Deserialize, Serialize};
use vol_llm_core::Result;
use crate::message::SessionMessage;
use crate::store::MessageStore;

/// JSONL file entry
#[derive(Serialize, Deserialize)]
struct JsonlEntry {
    event: String,
    data: serde_json::Value,
    session_id: String,
    timestamp: i64,
}

/// File-based message store using JSONL format
pub struct FileMessageStore {
    base_path: PathBuf,
    session_id: String,
    file_path: PathBuf,
}

impl FileMessageStore {
    /// Create a new file message store
    pub fn new(base_path: &str, session_id: &str) -> Result<Self> {
        let base_path = PathBuf::from(base_path);
        let session_dir = base_path.join("sessions");
        let file_path = session_dir.join(format!("{}.jsonl", session_id));

        Ok(Self {
            base_path,
            session_id: session_id.to_string(),
            file_path,
        })
    }

    /// Ensure the session directory exists
    async fn ensure_dir(&self) -> Result<()> {
        let session_dir = self.base_path.join("sessions");
        fs::create_dir_all(&session_dir).await?;
        Ok(())
    }

    /// Append a line to the JSONL file
    async fn append_line(&self, line: &str) -> Result<()> {
        self.ensure_dir().await?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?;
        
        writeln!(file, "{}", line).await?;
        file.sync_all().await?;
        Ok(())
    }

    /// Read all lines from the JSONL file
    async fn read_all_lines(&self) -> Result<Vec<String>> {
        if !self.file_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.file_path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut result = Vec::new();

        while let Some(line) = lines.next_line().await? {
            result.push(line);
        }

        Ok(result)
    }
}

#[async_trait]
impl MessageStore for FileMessageStore {
    async fn save(&self, message: SessionMessage) -> Result<()> {
        let entry = JsonlEntry {
            event: "SessionMessage".to_string(),
            data: serde_json::json!({
                "id": message.id,
                "session_id": message.session_id,
                "message": message.message,
                "parent_id": message.parent_id,
                "created_at": message.created_at,
                "metadata": message.metadata,
            }),
            session_id: message.session_id.clone(),
            timestamp: message.created_at,
        };

        let line = serde_json::to_string(&entry)?;
        self.append_line(&line).await?;
        Ok(())
    }

    async fn get_by_session(&self, _session_id: &str, limit: usize) -> Result<Vec<SessionMessage>> {
        let lines = self.read_all_lines().await?;
        let mut messages = Vec::new();

        for line in lines.into_iter().take(limit) {
            if let Ok(entry) = serde_json::from_str::<JsonlEntry>(&line) {
                // Parse back to SessionMessage
                // For now, skip detailed parsing - implement as needed
            }
        }

        Ok(messages)
    }

    async fn get_before(&self, session_id: &str, before: i64, limit: usize) -> Result<Vec<SessionMessage>> {
        // TODO: Implement pagination
        unimplemented!()
    }

    async fn delete_session(&self, _session_id: &str) -> Result<()> {
        if self.file_path.exists() {
            fs::remove_file(&self.file_path).await?;
        }
        Ok(())
    }

    async fn update(&self, _id: &str, _message: SessionMessage) -> Result<()> {
        // JSONL is append-only - updates require rewriting the file
        unimplemented!()
    }

    async fn get_count(&self, _session_id: &str) -> Result<usize> {
        let lines = self.read_all_lines().await?;
        Ok(lines.len())
    }

    async fn cleanup_expired(&self, before: i64) -> Result<()> {
        // TODO: Implement cleanup
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_file_message_store_save() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let store = FileMessageStore::new(tmp_dir.path().to_str().unwrap(), "test-1").unwrap();

        let msg = SessionMessage::new(
            "test-1".to_string(),
            Message::user("Hello"),
        );

        store.save(msg).await.unwrap();

        // Verify file exists and has one line
        let file_path = tmp_dir.path().join("sessions").join("test-1.jsonl");
        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content.lines().count(), 1);
    }
}
```

- [ ] **Step 2: 添加 tempfile 依赖到 Cargo.toml**

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p vol-session
```

- [ ] **Step 4: 运行测试**

```bash
cargo test -p vol-session --lib file_store
```

- [ ] **Step 5: 提交**

```bash
git add crates/vol-session/src/file_store.rs
git commit -m "feat: implement FileMessageStore with JSONL format"
```

---

## Task 5: 实现 SessionListener

**Files:**
- Create: `crates/vol-session/src/listener.rs`
- Test: `crates/vol-session/tests/listener_test.rs`

- [ ] **Step 1: 创建 SessionListener**

```rust
//! Session event listener for recording key events.

use std::sync::Arc;
use tokio::sync::broadcast;
use vol_llm_core::Result;
use crate::store::MessageStore;
use crate::message::SessionMessage;
use vol_llm_core::Message as CoreMessage;

// Import AgentStreamEvent from vol-llm-agent
// We'll need to define a minimal event type or use a trait
use vol_llm_agent::AgentStreamEvent;
use vol_tracing::TracedEvent;

/// SessionListener subscribes to event bus and records key events
pub struct SessionListener {
    event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
    store: Arc<dyn MessageStore>,
    session_id: String,
}

impl SessionListener {
    /// Create a new SessionListener
    pub fn new(
        event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
        store: Arc<dyn MessageStore>,
        session_id: String,
    ) -> Self {
        Self {
            event_rx,
            store,
            session_id,
        }
    }

    /// Check if an event should be recorded
    fn should_record(event: &AgentStreamEvent) -> bool {
        matches!(
            event,
            AgentStreamEvent::ThinkingComplete { .. }
                | AgentStreamEvent::ToolCallBegin { .. }
                | AgentStreamEvent::ToolCallComplete { .. }
                | AgentStreamEvent::IterationComplete { .. }
        )
    }

    /// Convert AgentStreamEvent to SessionMessage
    fn event_to_message(event: &AgentStreamEvent) -> Option<SessionMessage> {
        match event {
            AgentStreamEvent::ThinkingComplete { thinking } => {
                Some(SessionMessage::new(
                    "session".to_string(), // Will be overridden
                    CoreMessage::assistant(thinking.clone()),
                ))
            }
            AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
                // Tool calls are recorded as metadata, not as conversation messages
                None
            }
            AgentStreamEvent::ToolCallComplete { tool_name, result } => {
                // Tool results are recorded as system messages
                Some(SessionMessage::new(
                    "session".to_string(),
                    CoreMessage::system(format!("[Tool: {}] {}", tool_name, result)),
                ))
            }
            AgentStreamEvent::IterationComplete { final_answer, .. } => {
                if let Some(answer) = final_answer {
                    Some(SessionMessage::new(
                        "session".to_string(),
                        CoreMessage::assistant(answer.clone()),
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Run the listener loop
    pub async fn run(mut self) -> Result<()> {
        use tokio::sync::broadcast::error::RecvError;

        loop {
            match self.event_rx.recv().await {
                Ok(traced_event) => {
                    let event = &traced_event.data;
                    
                    if Self::should_record(event) {
                        if let Some(mut message) = Self::event_to_message(event) {
                            message.session_id = self.session_id.clone();
                            
                            if let Err(e) = self.store.save(message).await {
                                tracing::warn!(error = %e, "Failed to save session message");
                            }
                        }
                    }
                }
                Err(RecvError::Closed) => {
                    // Channel closed - exit gracefully
                    tracing::info!("SessionListener: channel closed, exiting");
                    break;
                }
                Err(RecvError::Lagged(n)) => {
                    // Lagged - log warning and continue
                    tracing::warn!(lagged = n, "SessionListener: lagged behind");
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_session_listener_records_events() {
        let (event_tx, event_rx) = broadcast::channel(100);
        
        let tmp_dir = tempfile::tempdir().unwrap();
        let store: Arc<dyn MessageStore> = Arc::new(
            FileMessageStore::new(tmp_dir.path().to_str().unwrap(), "test-1").unwrap()
        );

        let listener = SessionListener::new(event_rx, store, "test-1".to_string());

        // Spawn listener
        let handle = tokio::spawn(listener.run());

        // Send test events
        let thinking_event = AgentStreamEvent::ThinkingComplete {
            thinking: "Let me think...".to_string(),
        };
        event_tx.send(TracedEvent::new(thinking_event)).unwrap();

        // Wait for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Close channel to stop listener
        drop(event_tx);

        // Wait for listener to exit
        handle.await.unwrap().unwrap();

        // Verify file was created
        let file_path = tmp_dir.path().join("sessions").join("test-1.jsonl");
        assert!(file_path.exists());
    }
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p vol-session
```

- [ ] **Step 3: 运行测试**

```bash
cargo test -p vol-session --lib listener
```

- [ ] **Step 4: 提交**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "feat: implement SessionListener for event recording"
```

---

## Task 6: 更新 vol-llm-agent 依赖和导出

**Files:**
- Modify: `crates/vol-llm-agent/Cargo.toml`
- Modify: `crates/vol-llm-agent/src/lib.rs`
- Modify: `crates/vol-llm-agent/src/session/mod.rs`

- [ ] **Step 1: 添加 vol-session 依赖**

```toml
[dependencies]
# ... existing dependencies ...
vol-session = { path = "../vol-session" }
```

- [ ] **Step 2: 更新 lib.rs 导出**

```rust
// Re-export vol-session types
pub use vol_session::{
    Session, SessionMessage, SessionStore, MessageStore,
    InMemorySessionStore, InMemoryMessageStore,
    FileMessageStore, SessionListener,
};
```

- [ ] **Step 3: 更新 session/mod.rs**

```rust
//! Re-export vol-session for backwards compatibility.

pub use vol_session::{
    Session, SessionMessage, SessionStore, MessageStore,
    InMemorySessionStore, InMemoryMessageStore,
    FileMessageStore,
};
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p vol-llm-agent
```

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agent/{Cargo.toml,src/lib.rs,src/session/mod.rs}
git commit -m "chore: update vol-llm-agent to use vol-session"
```

---

## Task 7: 集成 SessionListener 到 ReActAgent

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: 在 run() 中创建 SessionListener**

在 `RunContext::new()` 调用后添加：

```rust
// === Phase 2.5: Spawn SessionListener for session recording ===
let session_listener = crate::session::SessionListener::new(
    run_ctx.event_tx.subscribe(),
    Arc::new(FileMessageStore::new(
        config.log_base_path.join(&config.agent_id).to_str().unwrap(),
        &session.id,
    )?),
    session.id.clone(),
);
tokio::spawn(session_listener.run());
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p vol-llm-agent
```

- [ ] **Step 3: 运行现有测试**

```bash
cargo test -p vol-llm-agent --lib
```

- [ ] **Step 4: 提交**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: integrate SessionListener into ReActAgent"
```

---

## Task 8: 添加集成测试

**Files:**
- Create: `crates/vol-session/tests/integration_test.rs`

- [ ] **Step 1: 创建集成测试**

```rust
//! SessionListener integration test.

use vol_session::{SessionListener, FileMessageStore, SessionMessage};
use vol_llm_agent::AgentStreamEvent;
use vol_tracing::TracedEvent;
use tokio::sync::broadcast;
use std::sync::Arc;
use vol_llm_core::Message;

#[tokio::test]
async fn test_session_listener_full_workflow() {
    let (event_tx, event_rx) = broadcast::channel(100);
    
    let tmp_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn vol_session::MessageStore> = Arc::new(
        FileMessageStore::new(tmp_dir.path().to_str().unwrap(), "session-1").unwrap()
    );

    let listener = SessionListener::new(event_rx, store, "session-1".to_string());
    let handle = tokio::spawn(listener.run());

    // Simulate full conversation workflow
    // 1. User message (not recorded by listener - comes from input)
    // 2. Thinking
    event_tx.send(TracedEvent::new(AgentStreamEvent::ThinkingComplete {
        thinking: "Let me search for BTC volatility...".to_string(),
    })).unwrap();

    // 3. Tool call
    event_tx.send(TracedEvent::new(AgentStreamEvent::ToolCallBegin {
        tool_name: "volatility_index".to_string(),
        arguments: r#"{"symbol": "BTC"}"#.to_string(),
    })).unwrap();

    event_tx.send(TracedEvent::new(AgentStreamEvent::ToolCallComplete {
        tool_name: "volatility_index".to_string(),
        result: "Index: btc_usd | Volatility: 42.98%".to_string(),
    })).unwrap();

    // 4. Iteration complete with final answer
    event_tx.send(TracedEvent::new(AgentStreamEvent::IterationComplete {
        iteration: 1,
        tool_calls: vec![],
        final_answer: Some("BTC 当前波动率为 42.98%...".to_string()),
    })).unwrap();

    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Close channel
    drop(event_tx);
    handle.await.unwrap().unwrap();

    // Verify JSONL file content
    let file_path = tmp_dir.path().join("sessions").join("session-1.jsonl");
    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();

    // Should have: Thinking, ToolCallComplete, IterationComplete (final_answer)
    // ToolCallBegin is not recorded as a message
    assert!(lines.len() >= 3, "Expected at least 3 lines, got {}", lines.len());
}
```

- [ ] **Step 2: 运行测试**

```bash
cargo test -p vol-session --test integration_test -- --nocapture
```

- [ ] **Step 3: 提交**

```bash
git add crates/vol-session/tests/integration_test.rs
git commit -m "test: add integration test for SessionListener"
```

---

## Task 9: 验证和清理

- [ ] **Step 1: 运行所有 vol-session 测试**

```bash
cargo test -p vol-session
```

Expected: All tests pass

- [ ] **Step 2: 运行所有 vol-llm-agent 测试**

```bash
cargo test -p vol-llm-agent
```

Expected: All tests pass

- [ ] **Step 3: 验证 workspace 编译**

```bash
cargo build --workspace
```

Expected: Compiles successfully

- [ ] **Step 4: 清理 unused code**

检查 `crates/vol-llm-agent/src/session/` 中的旧文件是否可以删除或简化为 re-export。

- [ ] **Step 5: 更新文档**

在 `crates/vol-session/README.md` 中添加完整的使用示例。

- [ ] **Step 6: 提交最终清理**

```bash
git commit -am "chore: final cleanup and documentation"
```

---

## 验收标准

完成所有 Tasks 后：

- [ ] `vol-session` 包独立编译
- [ ] JSONL 文件格式正确，可读可解析
- [ ] SessionListener 正确过滤并记录 5 种关键事件
- [ ] 通过 session 文件可以完整还原对话
- [ ] 现有测试全部通过
- [ ] 添加集成测试验证端到端流程
