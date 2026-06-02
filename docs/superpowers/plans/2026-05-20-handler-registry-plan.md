# Handler Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 7 hardcoded handler fields on `AgentServerCore` with a trait-based `HandlerRegistry` supporting both built-in and external handler registration.

**Architecture:** Create a `DomainHandler` trait and `HandlerRegistry` struct in `domain/`. Convert all 7 existing handlers to implement the trait. Replace the hardcoded handler fields and match-based dispatch in `AgentServerCore` with a single `HandlerRegistry`. Builder gains a `register_handler()` method for external handlers.

**Tech Stack:** Rust, async_trait, tokio, existing `AgentServerMessage`/`Operation`/`ProtocolError` types from `agent_server_protocol`.

---

### Task 1: Create `DomainHandler` trait

**Files:**
- Create: `crates/vol-llm-agent-channel/src/domain/handler.rs`

- [ ] **Step 1: Write the trait**

```rust
//! Domain handler trait and type aliases.

use async_trait::async_trait;
use std::sync::Arc;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, ProtocolError,
};

/// Trait for domain handlers registered into AgentServerCore.
#[async_trait]
pub trait DomainHandler: Send + Sync + 'static {
    /// Unique name for debugging and logging.
    fn name(&self) -> &str;

    /// Operations this handler exclusively owns.
    /// Return an empty vec for handlers using string-based routing only.
    fn operations(&self) -> Vec<Operation>;

    /// Handle a message. The operation is embedded in `message.operation`.
    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError>;
}

/// Type alias for a registered handler.
pub type HandlerRef = Arc<dyn DomainHandler>;
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/handler.rs
git commit -m "feat: add DomainHandler trait for pluggable handlers"
```

---

### Task 2: Create `HandlerRegistry`

**Files:**
- Create: `crates/vol-llm-agent-channel/src/domain/registry.rs`

- [ ] **Step 1: Write the registry**

```rust
//! Handler registry with operation-based dispatch.

use std::collections::HashMap;
use std::sync::Arc;

use crate::agent_server_protocol::{AgentServerMessage, Operation, ProtocolError};
use crate::domain::handler::{DomainHandler, HandlerRef};

/// Registry of domain handlers, dispatched by method name string.
pub struct HandlerRegistry {
    handlers: Vec<HandlerRef>,
    /// method_name → handler index
    method_index: HashMap<String, usize>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            method_index: HashMap::new(),
        }
    }

    /// Register a handler with type-safe Operation declarations.
    pub fn register(&mut self, handler: HandlerRef) -> Result<(), String> {
        let idx = self.handlers.len();
        for op in &handler.operations() {
            let method = op.method_name().to_string();
            if self.method_index.contains_key(&method) {
                return Err(format!(
                    "method '{}' already claimed by handler '{}'",
                    method,
                    self.handlers[self.method_index[&method]].name()
                ));
            }
        }
        for op in handler.operations() {
            self.method_index.insert(op.method_name().to_string(), idx);
        }
        self.handlers.push(handler);
        Ok(())
    }

    /// Register a custom handler with explicit method name strings.
    pub fn register_custom(
        &mut self,
        handler: HandlerRef,
        methods: &[&str],
    ) -> Result<(), String> {
        let idx = self.handlers.len();
        for method in methods {
            if self.method_index.contains_key(*method) {
                return Err(format!(
                    "method '{}' already registered",
                    method
                ));
            }
            self.method_index.insert(method.to_string(), idx);
        }
        self.handlers.push(handler);
        Ok(())
    }

    /// Dispatch a message to the appropriate handler.
    pub async fn dispatch(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let method = message.operation.method_name();
        if let Some(idx) = self.method_index.get(method) {
            return self.handlers[*idx].handle(message).await;
        }
        Err(ProtocolError::UnknownMethod(method.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_server_protocol::{
        AgentServerMessage, FileOperation, FilePayload, MessageKind, Operation, Payload,
    };
    use async_trait::async_trait;

    struct TestHandler {
        name: &'static str,
        ops: Vec<Operation>,
    }

    #[async_trait]
    impl DomainHandler for TestHandler {
        fn name(&self) -> &str { self.name }
        fn operations(&self) -> Vec<Operation> { self.ops.clone() }
        async fn handle(
            &self,
            msg: AgentServerMessage,
        ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
            Ok(vec![AgentServerMessage::new_result(
                msg.message_id,
                Operation::File(FileOperation::Read),
                Payload::File(FilePayload::ReadResult {
                    content: format!("handled by {}", self.name),
                    metadata: serde_json::json!({}),
                }),
            )])
        }
    }

    #[tokio::test]
    async fn test_register_and_dispatch() {
        let mut registry = HandlerRegistry::new();
        let handler = Arc::new(TestHandler {
            name: "test",
            ops: vec![
                Operation::File(FileOperation::List),
                Operation::File(FileOperation::Read),
            ],
        });
        registry.register(handler).unwrap();

        let msg = AgentServerMessage {
            message_id: "1".to_string(),
            kind: MessageKind::Command,
            operation: Operation::File(FileOperation::List),
            payload: Payload::File(FilePayload::List { path: ".".into() }),
            meta: Default::default(),
        };

        let results = registry.dispatch(msg).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, MessageKind::Result);
    }

    #[tokio::test]
    async fn test_duplicate_operation_rejected() {
        let mut registry = HandlerRegistry::new();
        let h1 = Arc::new(TestHandler {
            name: "first",
            ops: vec![Operation::File(FileOperation::List)],
        });
        let h2 = Arc::new(TestHandler {
            name: "second",
            ops: vec![Operation::File(FileOperation::List)],
        });
        registry.register(h1).unwrap();
        let err = registry.register(h2).unwrap_err();
        assert!(err.contains("already claimed"));
    }

    #[tokio::test]
    async fn test_unknown_method_returns_error() {
        let registry = HandlerRegistry::new();
        let msg = AgentServerMessage {
            message_id: "1".to_string(),
            kind: MessageKind::Command,
            operation: Operation::File(FileOperation::List),
            payload: Payload::File(FilePayload::List { path: ".".into() }),
            meta: Default::default(),
        };
        let err = registry.dispatch(msg).await.unwrap_err();
        assert!(matches!(err, ProtocolError::UnknownMethod(_)));
    }

    #[tokio::test]
    async fn test_register_custom() {
        let mut registry = HandlerRegistry::new();
        let handler = Arc::new(TestHandler {
            name: "custom",
            ops: vec![],
        });
        registry.register_custom(handler, &["custom.op"]).unwrap();

        // Custom operation must be dispatched via a method name that an Operation
        // variant maps to, or via an Operation we invent. For now, we test that
        // registration succeeds and the string index is populated.
        assert!(registry.method_index.contains_key("custom.op"));
    }
}
```

- [ ] **Step 2: Verify it compiles and tests pass**

Run: `cargo test -p vol-llm-agent-channel -- domain::registry 2>&1`
Expected: All 4 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/registry.rs
git commit -m "feat: add HandlerRegistry with operation-based dispatch"
```

---

### Task 3: Convert `LogHandler` to implement `DomainHandler`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/log.rs`

**Why LogHandler first:** It's the simplest handler — no constructor deps, no state, 2 operations. A clean smoke test for the trait.

- [ ] **Step 1: Rewrite `log.rs`**

Replace the entire file with:

```rust
use async_trait::async_trait;

use crate::agent_server_protocol::{
    AgentServerMessage, LogOperation, LogPayload, Operation, Payload, ProtocolError,
};
use crate::domain::handler::DomainHandler;

/// Handler for log-domain operations.
pub struct LogHandler;

#[async_trait]
impl DomainHandler for LogHandler {
    fn name(&self) -> &str {
        "log"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Log(LogOperation::List),
            Operation::Log(LogOperation::Read),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Log(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("log")),
        };
        match (op, message.payload) {
            (LogOperation::List, Payload::Log(LogPayload::List)) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Log(LogOperation::List),
                    Payload::Log(LogPayload::ListResult { runs: vec![] }),
                ),
            ]),
            (LogOperation::Read, Payload::Log(LogPayload::Read { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Log(LogOperation::Read),
                    Payload::Log(LogPayload::ReadResult { entries: vec![] }),
                ),
            ]),
            (LogOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("log.list")),
            (LogOperation::Read, _) => Err(ProtocolError::PayloadDecodeFailed("log.read")),
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS (with unused-import warnings until all old handler usage removed)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/log.rs
git commit -m "refactor: convert LogHandler to DomainHandler trait"
```

---

### Task 4: Convert `SystemHandler` to implement `DomainHandler`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/system.rs`

- [ ] **Step 1: Rewrite `system.rs`**

```rust
use async_trait::async_trait;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SystemOperation, SystemPayload,
};
use crate::domain::handler::DomainHandler;

/// Placeholder handler for system-domain operations.
pub struct SystemHandler;

#[async_trait]
impl DomainHandler for SystemHandler {
    fn name(&self) -> &str {
        "system"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![Operation::System(SystemOperation::Connected)]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::System(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("system")),
        };
        Ok(vec![AgentServerMessage::new_result(
            message.message_id,
            Operation::System(op),
            Payload::System(SystemPayload::Empty),
        )])
    }
}
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/system.rs
git commit -m "refactor: convert SystemHandler to DomainHandler trait"
```

---

### Task 5: Convert `FileHandler` to implement `DomainHandler`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/file.rs`

- [ ] **Step 1: Rewrite `file.rs`**

```rust
use std::path::PathBuf;

use async_trait::async_trait;

use crate::agent_server_protocol::{
    AgentServerMessage, FileOperation, FilePayload, Operation, Payload, ProtocolError,
};
use crate::domain::handler::DomainHandler;

/// Handler for file-domain operations.
pub struct FileHandler {
    working_dir: PathBuf,
}

impl FileHandler {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.working_dir.join(p)
        }
    }
}

#[async_trait]
impl DomainHandler for FileHandler {
    fn name(&self) -> &str {
        "file"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::File(FileOperation::List),
            Operation::File(FileOperation::Read),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::File(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("file")),
        };
        match (op, message.payload) {
            (FileOperation::List, Payload::File(FilePayload::List { path })) => {
                let resolved = self.resolve_path(&path);
                match std::fs::read_dir(&resolved) {
                    Ok(entries) => {
                        let mut list: Vec<serde_json::Value> = Vec::new();
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            list.push(serde_json::json!({
                                "name": name,
                                "is_dir": is_dir,
                                "size": size,
                            }));
                        }
                        list.sort_by(|a, b| {
                            let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                            let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                            b_dir.cmp(&a_dir).then_with(|| a["name"].as_str().cmp(&b["name"].as_str()))
                        });
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::File(FileOperation::List),
                            Payload::File(FilePayload::ListResult { entries: list }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::File(FileOperation::List),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "file_list_failed".to_string(),
                            message: format!("Failed to read directory: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (FileOperation::Read, Payload::File(FilePayload::Read { path })) => {
                let resolved = self.resolve_path(&path);
                match std::fs::read_to_string(&resolved) {
                    Ok(content) => Ok(vec![AgentServerMessage::new_result(
                        message.message_id,
                        Operation::File(FileOperation::Read),
                        Payload::File(FilePayload::ReadResult {
                            content,
                            metadata: serde_json::json!({}),
                        }),
                    )]),
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::File(FileOperation::Read),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "file_read_failed".to_string(),
                            message: format!("Failed to read file: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (FileOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("file.list")),
            (FileOperation::Read, _) => Err(ProtocolError::PayloadDecodeFailed("file.read")),
        }
    }
}
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/file.rs
git commit -m "refactor: convert FileHandler to DomainHandler trait"
```

---

### Task 6: Convert `SessionHandler` to implement `DomainHandler`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/session.rs`

- [ ] **Step 1: Rewrite `session.rs`**

```rust
use std::path::PathBuf;

use async_trait::async_trait;
use vol_session::file_store::FileSessionEntryStore;
use vol_session::SessionEntryStore;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SessionOperation, SessionPayload,
};
use crate::domain::handler::DomainHandler;

/// Handler for session-domain operations.
///
/// Scans all agent directories under agents_root to aggregate sessions.
pub struct SessionHandler {
    agents_root: PathBuf,
}

impl SessionHandler {
    pub fn new(agents_root: PathBuf) -> Self {
        Self { agents_root }
    }

    /// Get a session store for a specific agent.
    fn agent_store(&self, agent_id: &str) -> FileSessionEntryStore {
        FileSessionEntryStore::new(self.agents_root.join(agent_id).join("sessions"))
    }

    /// Find which agent owns a session by scanning all agent dirs.
    fn find_store_for_session(&self, session_id: &str) -> Result<FileSessionEntryStore, ProtocolError> {
        if self.agents_root.is_dir() {
            for entry in std::fs::read_dir(&self.agents_root).into_iter().flatten().flatten() {
                if entry.path().is_dir() {
                    if let Some(agent_id) = entry.file_name().to_str() {
                        let store = self.agent_store(agent_id);
                        if let Ok(summaries) = store.list_sessions() {
                            if summaries.iter().any(|s| s.session_id == session_id) {
                                return Ok(store);
                            }
                        }
                    }
                }
            }
        }
        Err(ProtocolError::PayloadDecodeFailed("session not found in any agent"))
    }
}

#[async_trait]
impl DomainHandler for SessionHandler {
    fn name(&self) -> &str {
        "session"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Session(SessionOperation::List),
            Operation::Session(SessionOperation::Resume),
            Operation::Session(SessionOperation::Entries),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Session(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("session")),
        };
        match (op, message.payload) {
            (SessionOperation::List, Payload::Session(SessionPayload::List)) => {
                let mut all_sessions: Vec<serde_json::Value> = Vec::new();

                if self.agents_root.is_dir() {
                    for entry in std::fs::read_dir(&self.agents_root).into_iter().flatten().flatten() {
                        if entry.path().is_dir() {
                            if let Some(agent_id) = entry.file_name().to_str() {
                                let store = self.agent_store(agent_id);
                                if let Ok(summaries) = store.list_sessions() {
                                    for s in summaries {
                                        all_sessions.push(serde_json::json!({
                                            "agent_id": agent_id,
                                            "session_id": s.session_id,
                                            "entry_count": s.entry_count,
                                            "created_at": s.created_at,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Session(SessionOperation::List),
                    Payload::Session(SessionPayload::ListResult { sessions: all_sessions }),
                )])
            }
            (SessionOperation::Resume, Payload::Session(SessionPayload::Resume { session_id, agent_id })) => {
                let store = match agent_id {
                    Some(id) => self.agent_store(&id),
                    None => self.find_store_for_session(&session_id)?,
                };
                match store.get_entries(&session_id).await {
                    Ok(entries) => {
                        let json_entries: Vec<serde_json::Value> = entries
                            .into_iter()
                            .filter_map(|e| serde_json::to_value(e).ok())
                            .collect();
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Session(SessionOperation::Resume),
                            Payload::Session(SessionPayload::ResumeResult {
                                session_id,
                                restored: true,
                                entries: json_entries,
                            }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Session(SessionOperation::Resume),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "session_resume_failed".to_string(),
                            message: format!("Failed to resume session: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (SessionOperation::Entries, Payload::Session(SessionPayload::Entries { session_id, agent_id })) => {
                let store = match agent_id {
                    Some(id) => self.agent_store(&id),
                    None => self.find_store_for_session(&session_id)?,
                };
                match store.get_entries(&session_id).await {
                    Ok(entries) => {
                        let json_entries: Vec<serde_json::Value> = entries
                            .into_iter()
                            .filter_map(|e| serde_json::to_value(e).ok())
                            .collect();
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Session(SessionOperation::Entries),
                            Payload::Session(SessionPayload::EntriesResult { entries: json_entries }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Session(SessionOperation::Entries),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "session_entries_failed".to_string(),
                            message: format!("Failed to get entries: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (SessionOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("session.list")),
            (SessionOperation::Resume, _) => Err(ProtocolError::PayloadDecodeFailed("session.resume")),
            (SessionOperation::Entries, _) => Err(ProtocolError::PayloadDecodeFailed("session.entries")),
        }
    }
}
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/session.rs
git commit -m "refactor: convert SessionHandler to DomainHandler trait"
```

---

### Task 7: Convert `SkillHandler` to implement `DomainHandler`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/skill.rs`

- [ ] **Step 1: Rewrite `skill.rs`**

```rust
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_skill::SkillLoader;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SkillOperation, SkillPayload,
};
use crate::domain::handler::DomainHandler;

/// Handler for skill-domain operations.
pub struct SkillHandler {
    skill_loader: Option<Arc<SkillLoader>>,
}

impl SkillHandler {
    pub fn new(skill_loader: Option<Arc<SkillLoader>>) -> Self {
        Self { skill_loader }
    }
}

#[async_trait]
impl DomainHandler for SkillHandler {
    fn name(&self) -> &str {
        "skill"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Skill(SkillOperation::List),
            Operation::Skill(SkillOperation::Get),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Skill(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("skill")),
        };
        match (op, message.payload) {
            (SkillOperation::List, Payload::Skill(SkillPayload::List)) => {
                let skills = match &self.skill_loader {
                    Some(loader) => {
                        let metadata = loader.list_metadata().await;
                        metadata.iter().map(|m| {
                            serde_json::json!({
                                "id": m.id,
                                "name": m.name,
                                "version": m.version,
                                "scope": m.scope.to_string(),
                                "description": m.description,
                                "triggers": m.triggers,
                            })
                        }).collect()
                    }
                    None => vec![],
                };
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Skill(SkillOperation::List),
                    Payload::Skill(SkillPayload::ListResult { skills }),
                )])
            }
            (SkillOperation::Get, Payload::Skill(SkillPayload::Get { name })) => {
                let skill = match &self.skill_loader {
                    Some(loader) => loader.get(&name).await.map(|s| serde_json::json!({
                        "name": s.name,
                        "version": s.version,
                        "scope": s.scope.to_string(),
                        "description": s.description,
                        "triggers": s.triggers,
                        "content": s.content,
                    })),
                    None => None,
                };
                match skill {
                    Some(skill) => Ok(vec![AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Skill(SkillOperation::Get),
                        Payload::Skill(SkillPayload::GetResult { skill, name }),
                    )]),
                    None => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Skill(SkillOperation::Get),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "skill_not_found".to_string(),
                            message: format!("Skill '{name}' not found"),
                            detail: None,
                            terminal: false,
                        },
                    )]),
                }
            }
            (SkillOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("skill.list")),
            (SkillOperation::Get, _) => Err(ProtocolError::PayloadDecodeFailed("skill.get")),
        }
    }
}
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/skill.rs
git commit -m "refactor: convert SkillHandler to DomainHandler trait"
```

---

### Task 8: Convert `McpHandler` to implement `DomainHandler`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/mcp.rs`

- [ ] **Step 1: Rewrite `mcp.rs`**

```rust
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_mcp::manager::McpManager;

use crate::agent_server_protocol::{
    AgentServerMessage, McpOperation, McpPayload, Operation, Payload, ProtocolError,
};
use crate::domain::handler::DomainHandler;

/// Handler for MCP-domain operations.
pub struct McpHandler {
    mcp_manager: Option<Arc<McpManager>>,
}

impl McpHandler {
    pub fn new(mcp_manager: Option<Arc<McpManager>>) -> Self {
        Self { mcp_manager }
    }

    fn mgr(&self) -> Result<&Arc<McpManager>, ProtocolError> {
        self.mcp_manager
            .as_ref()
            .ok_or(ProtocolError::PayloadDecodeFailed("mcp not configured"))
    }

    fn server_status_to_str(status: &vol_llm_mcp::manager::ServerStatus) -> String {
        match status {
            vol_llm_mcp::manager::ServerStatus::Connected => "connected".into(),
            vol_llm_mcp::manager::ServerStatus::Disconnected => "disconnected".into(),
            vol_llm_mcp::manager::ServerStatus::Connecting => "connecting".into(),
            vol_llm_mcp::manager::ServerStatus::Error(e) => format!("error: {e}"),
        }
    }
}

#[async_trait]
impl DomainHandler for McpHandler {
    fn name(&self) -> &str {
        "mcp"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Mcp(McpOperation::ListServers),
            Operation::Mcp(McpOperation::ListTools),
            Operation::Mcp(McpOperation::CallTool),
            Operation::Mcp(McpOperation::ListResources),
            Operation::Mcp(McpOperation::ListResourceTemplates),
            Operation::Mcp(McpOperation::ReadResource),
            Operation::Mcp(McpOperation::ListPrompts),
            Operation::Mcp(McpOperation::GetPrompt),
            Operation::Mcp(McpOperation::Reconnect),
            Operation::Mcp(McpOperation::ServerStatus),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Mcp(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("mcp")),
        };
        match (op, message.payload) {
            (McpOperation::ListServers, Payload::Mcp(McpPayload::ListServers)) => {
                let mgr = self.mgr()?;
                let status = mgr.server_status_async().await;
                let servers: Vec<serde_json::Value> = status.iter().map(|(name, s)| {
                    serde_json::json!({ "name": name, "status": Self::server_status_to_str(s) })
                }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListServers),
                    Payload::Mcp(McpPayload::ListServersResult { servers }),
                )])
            }
            (McpOperation::ListTools, Payload::Mcp(McpPayload::ListTools { server })) => {
                let mgr = self.mgr()?;
                let tools = mgr.list_all_tools().await;
                let tools_json: Vec<serde_json::Value> = tools.iter()
                    .filter(|(s, _)| server.as_ref().map_or(true, |f| s == f))
                    .map(|(s, t)| {
                        serde_json::json!({
                            "server": s, "name": t.name, "description": t.description,
                            "input_schema": t.input_schema,
                        })
                    }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListTools),
                    Payload::Mcp(McpPayload::ListToolsResult { tools: tools_json }),
                )])
            }
            (McpOperation::CallTool, Payload::Mcp(McpPayload::CallTool { server, tool_name, arguments })) => {
                let mgr = self.mgr()?;
                match mgr.call_tool(&server, &tool_name, arguments).await {
                    Ok(result) => Ok(vec![AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Mcp(McpOperation::CallTool),
                        Payload::Mcp(McpPayload::CallToolResult {
                            tool_name,
                            result: serde_json::json!(result),
                        }),
                    )]),
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Mcp(McpOperation::CallTool),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "mcp_call_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (McpOperation::ListResources, Payload::Mcp(McpPayload::ListResources { server })) => {
                let mgr = self.mgr()?;
                let resources = mgr.list_all_resources().await;
                let r_json: Vec<serde_json::Value> = resources.iter()
                    .filter(|(s, _)| server.as_ref().map_or(true, |f| s == f))
                    .map(|(s, r)| {
                        serde_json::json!({
                            "server": s, "name": r.name, "uri": r.uri,
                            "mime_type": r.mime_type, "description": r.description,
                        })
                    }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListResources),
                    Payload::Mcp(McpPayload::ListResourcesResult { resources: r_json }),
                )])
            }
            (McpOperation::ListResourceTemplates, Payload::Mcp(McpPayload::ListResourceTemplates { server })) => {
                let mgr = self.mgr()?;
                let templates = mgr.list_all_resource_templates().await;
                let t_json: Vec<serde_json::Value> = templates.iter()
                    .filter(|(s, _)| server.as_ref().map_or(true, |f| s == f))
                    .map(|(s, t)| {
                        serde_json::json!({
                            "server": s, "name": t.name, "uri_template": t.uri_template,
                            "description": t.description,
                        })
                    }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListResourceTemplates),
                    Payload::Mcp(McpPayload::ListResourceTemplatesResult { templates: t_json }),
                )])
            }
            (McpOperation::ReadResource, Payload::Mcp(McpPayload::ReadResource { uri })) => {
                let mgr = self.mgr()?;
                match mgr.read_resource(&uri).await {
                    Ok(content) => Ok(vec![AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Mcp(McpOperation::ReadResource),
                        Payload::Mcp(McpPayload::ReadResourceResult { uri, content }),
                    )]),
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Mcp(McpOperation::ReadResource),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "mcp_read_resource_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (McpOperation::ListPrompts, Payload::Mcp(McpPayload::ListPrompts { server })) => {
                let mgr = self.mgr()?;
                let prompts = mgr.list_all_prompts().await;
                let p_json: Vec<serde_json::Value> = prompts.iter()
                    .filter(|(s, _)| server.as_ref().map_or(true, |f| s == f))
                    .map(|(s, p)| {
                        let args = p.arguments.as_ref().map(|args| {
                            args.iter().map(|a| {
                                serde_json::json!({
                                    "name": a.name,
                                    "description": a.description,
                                    "required": a.required,
                                })
                            }).collect::<Vec<_>>()
                        });
                        serde_json::json!({
                            "server": s, "name": p.name, "description": p.description,
                            "arguments": args,
                        })
                    }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListPrompts),
                    Payload::Mcp(McpPayload::ListPromptsResult { prompts: p_json }),
                )])
            }
            (McpOperation::GetPrompt, Payload::Mcp(McpPayload::GetPrompt { name, arguments })) => {
                let mgr = self.mgr()?;
                match mgr.get_prompt(&name, arguments.map(|m| m.into_iter().collect())).await {
                    Ok((desc, messages)) => {
                        let msgs = messages.iter().map(|m| {
                            let content = serde_json::to_string(&m.content).unwrap_or_default();
                            let role = format!("{:?}", m.role);
                            serde_json::json!({ "role": role, "content": content })
                        }).collect::<Vec<_>>();
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Mcp(McpOperation::GetPrompt),
                            Payload::Mcp(McpPayload::GetPromptResult {
                                name,
                                prompt: serde_json::json!({
                                    "description": desc,
                                    "messages": msgs,
                                }),
                            }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Mcp(McpOperation::GetPrompt),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "mcp_get_prompt_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (McpOperation::Reconnect, Payload::Mcp(McpPayload::Reconnect { server })) => {
                let mgr = self.mgr()?;
                match mgr.reconnect(&server).await {
                    Ok(()) => {
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Mcp(McpOperation::Reconnect),
                            Payload::Mcp(McpPayload::ReconnectResult { reconnected: true }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Mcp(McpOperation::Reconnect),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "mcp_reconnect_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (McpOperation::ServerStatus, Payload::Mcp(McpPayload::ServerStatus { server: _ })) => {
                let mgr = self.mgr()?;
                let status = mgr.server_status_async().await;
                let servers: Vec<serde_json::Value> = status.iter().map(|(name, s)| {
                    serde_json::json!({ "name": name, "status": Self::server_status_to_str(s) })
                }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ServerStatus),
                    Payload::Mcp(McpPayload::ServerStatusResult {
                        server: "all".to_string(),
                        status: format!("{} servers", servers.len()),
                    }),
                )])
            }
            (McpOperation::ListServers, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_servers")),
            (McpOperation::ListTools, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_tools")),
            (McpOperation::CallTool, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.call_tool")),
            (McpOperation::ListResources, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_resources")),
            (McpOperation::ListResourceTemplates, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_resource_templates")),
            (McpOperation::ReadResource, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.read_resource")),
            (McpOperation::ListPrompts, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_prompts")),
            (McpOperation::GetPrompt, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.get_prompt")),
            (McpOperation::Reconnect, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.reconnect")),
            (McpOperation::ServerStatus, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.server_status")),
        }
    }
}
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/mcp.rs
git commit -m "refactor: convert McpHandler to DomainHandler trait"
```

---

### Task 9: Convert `AgentHandler` to implement `DomainHandler`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/agent.rs`

- [ ] **Step 1: Rewrite `agent.rs`**

```rust
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, Operation, Payload, ProtocolError,
};
use crate::connection::ConnectionHolder;
use crate::domain::handler::DomainHandler;
use crate::router::AgentRouter;

/// Handler for agent-domain operations.
pub struct AgentHandler {
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
}

impl AgentHandler {
    pub fn new(
        router: AgentRouter,
        holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
    ) -> Self {
        Self { router, holders }
    }
}

#[async_trait]
impl DomainHandler for AgentHandler {
    fn name(&self) -> &str {
        "agent"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Agent(AgentOperation::Submit),
            Operation::Agent(AgentOperation::Cancel),
            Operation::Agent(AgentOperation::Subscribe),
            Operation::Agent(AgentOperation::Unsubscribe),
            Operation::Agent(AgentOperation::Approve),
            Operation::Agent(AgentOperation::List),
            Operation::Agent(AgentOperation::Event),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Agent(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("agent")),
        };
        match (op, message.payload) {
            (AgentOperation::Submit, Payload::Agent(AgentPayload::Submit { .. })) => {
                let run_id = uuid::Uuid::new_v4().to_string();
                Ok(vec![
                    AgentServerMessage::new_ack(
                        message.message_id.clone(),
                        Operation::Agent(AgentOperation::Submit),
                        Payload::Agent(AgentPayload::SubmitAck {
                            run_id: run_id.clone(),
                            accepted: true,
                        }),
                    ),
                    AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Agent(AgentOperation::Submit),
                        Payload::Agent(AgentPayload::SubmitResult {
                            run_id,
                            response: serde_json::json!({ "output": "" }),
                        }),
                    ),
                ])
            }
            (AgentOperation::Cancel, Payload::Agent(AgentPayload::Cancel { run_id })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Cancel),
                    Payload::Agent(AgentPayload::CancelResult {
                        run_id,
                        cancelled: false,
                    }),
                ),
            ]),
            (AgentOperation::Subscribe, Payload::Agent(AgentPayload::Subscribe { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Subscribe),
                    Payload::Agent(AgentPayload::SubscribeResult {
                        subscription_id: uuid::Uuid::new_v4().to_string(),
                    }),
                ),
            ]),
            (AgentOperation::Unsubscribe, Payload::Agent(AgentPayload::Unsubscribe { subscription_id })) => {
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Unsubscribe),
                    Payload::Agent(AgentPayload::UnsubscribeResult {
                        subscription_id,
                        removed: true,
                    }),
                )])
            }
            (AgentOperation::Approve, Payload::Agent(AgentPayload::Approve { run_id, .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Approve),
                    Payload::Agent(AgentPayload::ApproveResult {
                        run_id,
                        accepted: true,
                    }),
                ),
            ]),
            (AgentOperation::List, _) => {
                let agents: Vec<serde_json::Value> = self
                    .holders
                    .lock()
                    .unwrap()
                    .keys()
                    .map(|k| serde_json::json!({ "id": k, "name": k }))
                    .collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::List),
                    Payload::Agent(AgentPayload::ListResult { agents }),
                )])
            }
            (AgentOperation::Event, Payload::Agent(AgentPayload::Event { run_id, event })) => Ok(vec![
                AgentServerMessage::new_event(
                    message.message_id,
                    Operation::Agent(AgentOperation::Event),
                    Payload::Agent(AgentPayload::Event { run_id, event }),
                ),
            ]),
            (AgentOperation::Submit, _) => Err(ProtocolError::PayloadDecodeFailed("agent.submit")),
            (AgentOperation::Cancel, _) => Err(ProtocolError::PayloadDecodeFailed("agent.cancel")),
            (AgentOperation::Subscribe, _) => Err(ProtocolError::PayloadDecodeFailed("agent.subscribe")),
            (AgentOperation::Unsubscribe, _) => Err(ProtocolError::PayloadDecodeFailed("agent.unsubscribe")),
            (AgentOperation::Approve, _) => Err(ProtocolError::PayloadDecodeFailed("agent.approve")),
            (AgentOperation::Event, _) => Err(ProtocolError::PayloadDecodeFailed("agent.event")),
        }
    }
}
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/agent.rs
git commit -m "refactor: convert AgentHandler to DomainHandler trait"
```

---

### Task 10: Update `domain/mod.rs` exports

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/mod.rs`

- [ ] **Step 1: Add new module declarations**

Replace the file with:

```rust
pub mod agent;
pub mod file;
pub mod handler;
pub mod log;
pub mod mcp;
pub mod registry;
pub mod session;
pub mod skill;
pub mod system;
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/mod.rs
git commit -m "feat: add handler and registry module declarations"
```

---

### Task 11: Update `AgentServerCore` to use `HandlerRegistry`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`

This is the core integration step. Three changes: struct fields, `handle()`, `builder::build()`, and `for_test()`.

- [ ] **Step 1: Replace struct handler fields with registry**

Replace lines 90-97 (the handler fields section of `AgentServerCore`):

```rust
    // === Domain handlers ===
    handler_registry: crate::domain::registry::HandlerRegistry,
```

- [ ] **Step 2: Replace `handle()` dispatch with registry dispatch**

Replace lines 219-233 (the `handle()` method):

```rust
    /// Handle an inbound `AgentServerMessage` by dispatching via the handler registry.
    pub async fn handle(
        &self,
        message: crate::agent_server_protocol::AgentServerMessage,
    ) -> Result<Vec<crate::agent_server_protocol::AgentServerMessage>, crate::agent_server_protocol::ProtocolError> {
        self.handler_registry.dispatch(message).await
    }
```

- [ ] **Step 3: Update import block to reflect new use pattern**

Replace the domain import block (lines 28-31):

The old import:

```rust
use crate::domain::{
    agent::AgentHandler, file::FileHandler, log::LogHandler, mcp::McpHandler,
    session::SessionHandler, skill::SkillHandler, system::SystemHandler,
};
```

Replace with:

```rust
use crate::domain::registry::HandlerRegistry;
use crate::domain::{
    agent::AgentHandler, file::FileHandler, log::LogHandler, mcp::McpHandler,
    session::SessionHandler, skill::SkillHandler, system::SystemHandler,
};
```

- [ ] **Step 4: Update builder `build()` to register handlers**

Replace lines 295-301 (the handler construction block in `build()`):

The old code:

```rust
        let agent = AgentHandler::new(router.clone(), Arc::clone(&holders));
        let file = FileHandler::new(working_dir.clone());
        let session = SessionHandler::new(agents_root);
        let mcp = McpHandler::new(Some(mcp_manager.clone()));
        let skill = SkillHandler::new(Some(skill_loader.clone()));
        let log = LogHandler;
        let system = SystemHandler;
```

Replace with:

```rust
        let mut handler_registry = HandlerRegistry::new();
        handler_registry
            .register(Arc::new(AgentHandler::new(router.clone(), Arc::clone(&holders))))
            .map_err(|e| format!("failed to register AgentHandler: {e}"))?;
        handler_registry
            .register(Arc::new(FileHandler::new(working_dir.clone())))
            .map_err(|e| format!("failed to register FileHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SessionHandler::new(agents_root)))
            .map_err(|e| format!("failed to register SessionHandler: {e}"))?;
        handler_registry
            .register(Arc::new(McpHandler::new(Some(mcp_manager.clone()))))
            .map_err(|e| format!("failed to register McpHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SkillHandler::new(Some(skill_loader.clone()))))
            .map_err(|e| format!("failed to register SkillHandler: {e}"))?;
        handler_registry
            .register(Arc::new(LogHandler))
            .map_err(|e| format!("failed to register LogHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SystemHandler))
            .map_err(|e| format!("failed to register SystemHandler: {e}"))?;

        // Register extra handlers from builder (external registration).
        for extra in self.extra_handlers {
            handler_registry
                .register(extra)
                .map_err(|e| format!("failed to register external handler: {e}"))?;
        }
```

- [ ] **Step 5: Update `AgentServerCore` struct literal in `build()` return**

Replace lines 303-319 (the `Ok(AgentServerCore { ... })` literal):

```rust
        Ok(AgentServerCore {
            working_dir,
            store_dir,
            mcp_manager,
            skill_loader,
            tool_registry,
            llm,
            router,
            holders,
            handler_registry,
        })
```

- [ ] **Step 6: Update `for_test()` to use registry**

Replace lines 406-412 (the handler construction in `for_test()`):

```rust
        let mut handler_registry = HandlerRegistry::new();
        handler_registry.register(Arc::new(AgentHandler::new(router.clone(), Arc::clone(&holders)))).ok();
        handler_registry.register(Arc::new(FileHandler::new(PathBuf::from(".")))).ok();
        handler_registry.register(Arc::new(SessionHandler::new(agents_root))).ok();
        handler_registry.register(Arc::new(McpHandler::new(None))).ok();
        handler_registry.register(Arc::new(SkillHandler::new(None))).ok();
        handler_registry.register(Arc::new(LogHandler)).ok();
        handler_registry.register(Arc::new(SystemHandler)).ok();
```

Replace the struct literal return lines 414-430 (the `AgentServerCore { ... }` in `for_test()`):

```rust
        AgentServerCore {
            working_dir: PathBuf::from("."),
            store_dir,
            mcp_manager: Arc::new(McpManager::new(vec![])),
            skill_loader: Arc::new(SkillLoader::new_empty()),
            tool_registry: Arc::new(ToolRegistry::new()),
            llm: Arc::new(TestLlm),
            router,
            holders,
            handler_registry,
        }
```

- [ ] **Step 7: Add `extra_handlers` field to `AgentServerCoreBuilder`**

Replace lines 240-243 (the builder struct):

```rust
pub struct AgentServerCoreBuilder {
    working_dir: Option<PathBuf>,
    store_dir: Option<PathBuf>,
    extra_handlers: Vec<Arc<dyn crate::domain::handler::DomainHandler>>,
}
```

Replace the `Default` impl (lines 245-252):

```rust
impl Default for AgentServerCoreBuilder {
    fn default() -> Self {
        Self {
            working_dir: None,
            store_dir: None,
            extra_handlers: Vec::new(),
        }
    }
}
```

- [ ] **Step 8: Add `register_handler()` method to builder**

After the `store_dir()` method (line 265), add:

```rust
    /// Register an external domain handler.
    pub fn register_handler(mut self, handler: Arc<dyn crate::domain::handler::DomainHandler>) -> Self {
        self.extra_handlers.push(handler);
        self
    }
```

- [ ] **Step 9: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 10: Commit**

```bash
git add crates/vol-llm-agent-channel/src/server_core.rs
git commit -m "refactor: replace hardcoded handler fields with HandlerRegistry"
```

---

### Task 12: Update `lib.rs` exports

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/lib.rs`

- [ ] **Step 1: Add public exports for new types**

After the existing `pub use server_core::AgentServerCore;` line, add:

```rust
pub use domain::handler::DomainHandler;
pub use domain::registry::HandlerRegistry;
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/lib.rs
git commit -m "feat: export DomainHandler and HandlerRegistry from lib"
```

---

### Task 13: Run full test suite

**Files:**
- All tests in `crates/vol-llm-agent-channel/`

- [ ] **Step 1: Run all tests**

Run: `cargo test -p vol-llm-agent-channel 2>&1`
Expected: All tests PASS, including existing integration tests and new registry unit tests.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p vol-llm-agent-channel 2>&1`
Expected: No new warnings introduced.

- [ ] **Step 3: Commit any fixes if needed**

```bash
git add -A && git diff --cached --stat
```

---

### Task 14: Build example to verify integration

**Files:**
- `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs`

- [ ] **Step 1: Check example compiles**

Run: `cargo check --example jsonrpc_agent_service -p vol-llm-agent-channel 2>&1`
Expected: PASS (no changes needed — example uses `core.handle()` indirectly via JSON-RPC server)

- [ ] **Step 2: Done — no commit needed**
