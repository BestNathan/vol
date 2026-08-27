# Sandbox Lifecycle Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the sandbox abstraction to introduce explicit instance lifecycle management with stable IDs, lifecycle states, provider abstraction, and unified orchestration.

**Architecture:** Introduce `SandboxId` for stable instance identity, `SandboxStatus` for lifecycle states, `SandboxProvider` trait for backend adapters, `SandboxStore` for instance metadata, and `SandboxManager` to replace `SandboxRegistry`. Refactor the `Sandbox` trait to focus on execution/filesystem operations while moving lifecycle management to providers.

**Tech Stack:** Rust, async-trait, serde, tokio, ULID (for SandboxId generation)

**Spec:** docs/superpowers/specs/2026-08-26-sandbox-lifecycle-design.md

## Global Constraints

- Backend only — no frontend changes
- Big bang refactor — break existing code, then fix
- Migrate Local, Tmp, SSH providers (Firecracker/Wasm deferred)
- `SandboxStore` trait with `InMemorySandboxStore` implementation
- Merge `SandboxRegistry` into `SandboxManager`
- Coverage ≥ 80% for new code
- All existing tests pass after migration

---

### Task 1: Core Types — SandboxId, SandboxStatus, SandboxCapabilities

**Files:**
- Modify: `crates/vol-llm-sandbox/Cargo.toml`
- Modify: `crates/vol-llm-sandbox/src/lib.rs`
- Create: `crates/vol-llm-sandbox/tests/core_types.rs`

**Interfaces:**
- Produces: `SandboxId`, `SandboxStatus`, `SandboxCapabilities` types

- [ ] **Step 1: Add dependencies to Cargo.toml**

```toml
ulid = { version = "1.1", features = ["serde"] }
chrono = { workspace = true }
```

- [ ] **Step 2: Write failing tests**

```rust
// crates/vol-llm-sandbox/tests/core_types.rs
use vol_llm_sandbox::{SandboxId, SandboxStatus, SandboxCapabilities};

#[test]
fn test_sandbox_id_generation() {
    let id1 = SandboxId::new();
    let id2 = SandboxId::new();
    assert_ne!(id1, id2);
    assert!(id1.to_string().starts_with("sb_"));
}

#[test]
fn test_sandbox_status_variants() {
    let status = SandboxStatus::Running;
    assert_eq!(status, SandboxStatus::Running);
}

#[test]
fn test_sandbox_capabilities() {
    let caps = SandboxCapabilities {
        persistent: true,
        pausable: false,
        stoppable: false,
        destroyable: false,
    };
    assert!(caps.persistent);
    assert!(!caps.pausable);
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vol-llm-sandbox --test core_types`
Expected: FAIL with "cannot find type `SandboxId`"

- [ ] **Step 4: Implement core types in lib.rs**

Add after imports:

```rust
use ulid::Ulid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SandboxId(String);

impl SandboxId {
    pub fn new() -> Self {
        Self(format!("sb_{}", Ulid::new()))
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SandboxId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for SandboxId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStatus {
    Creating,
    Created,
    Starting,
    Running,
    Pausing,
    Paused,
    Stopping,
    Stopped,
    Destroying,
    Destroyed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCapabilities {
    pub persistent: bool,
    pub pausable: bool,
    pub stoppable: bool,
    pub destroyable: bool,
}
```

Add error variants:

```rust
#[error("Sandbox not found: {0}")]
NotFound(String),

#[error("Invalid state transition: {from:?} -> {to:?}")]
InvalidTransition { from: SandboxStatus, to: SandboxStatus },
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vol-llm-sandbox --test core_types`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(sandbox): add SandboxId, SandboxStatus, SandboxCapabilities types"
```

---

### Task 2: SandboxStore Trait + InMemorySandboxStore

**Files:**
- Create: `crates/vol-llm-sandbox/src/store.rs`
- Modify: `crates/vol-llm-sandbox/src/lib.rs`
- Create: `crates/vol-llm-sandbox/tests/store_tests.rs`

**Interfaces:**
- Produces: `SandboxStore` trait, `InMemorySandboxStore`, `SandboxRecord`, `SandboxFilter`

- [ ] **Step 1: Write failing tests**

```rust
// crates/vol-llm-sandbox/tests/store_tests.rs
use vol_llm_sandbox::{SandboxId, SandboxStatus, SandboxRecord, SandboxFilter, InMemorySandboxStore, SandboxStore};
use chrono::Utc;
use std::collections::HashMap;

#[tokio::test]
async fn test_insert_and_get() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();
    let record = SandboxRecord {
        id: id.clone(),
        profile: "test".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };
    
    store.insert(record.clone()).await.unwrap();
    let retrieved = store.get(&id).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, id);
}

#[tokio::test]
async fn test_list_with_filter() {
    let store = InMemorySandboxStore::new();
    
    let record1 = SandboxRecord {
        id: SandboxId::new(),
        profile: "coding".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };
    
    let record2 = SandboxRecord {
        id: SandboxId::new(),
        profile: "testing".to_string(),
        provider_kind: "tmp".to_string(),
        backend_id: "backend_2".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };
    
    store.insert(record1).await.unwrap();
    store.insert(record2).await.unwrap();
    
    let filter = SandboxFilter {
        profile: Some("coding".to_string()),
        provider_kind: None,
        status: None,
    };
    
    let results = store.list(Some(filter)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].profile, "coding");
}

#[tokio::test]
async fn test_update_status() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();
    let record = SandboxRecord {
        id: id.clone(),
        profile: "test".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Creating,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };
    
    store.insert(record).await.unwrap();
    store.update_status(&id, SandboxStatus::Running).await.unwrap();
    
    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.status, SandboxStatus::Running);
}

#[tokio::test]
async fn test_delete() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();
    let record = SandboxRecord {
        id: id.clone(),
        profile: "test".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };
    
    store.insert(record).await.unwrap();
    store.delete(&id).await.unwrap();
    
    let retrieved = store.get(&id).await.unwrap();
    assert!(retrieved.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-sandbox --test store_tests`
Expected: FAIL

- [ ] **Step 3: Implement store.rs**

```rust
// crates/vol-llm-sandbox/src/store.rs
use crate::{SandboxId, SandboxStatus, SandboxResult, SandboxError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRecord {
    pub id: SandboxId,
    pub profile: String,
    pub provider_kind: String,
    pub backend_id: String,
    pub status: SandboxStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct SandboxFilter {
    pub profile: Option<String>,
    pub provider_kind: Option<String>,
    pub status: Option<SandboxStatus>,
}

#[async_trait]
pub trait SandboxStore: Send + Sync {
    async fn insert(&self, record: SandboxRecord) -> SandboxResult<()>;
    async fn get(&self, id: &SandboxId) -> SandboxResult<Option<SandboxRecord>>;
    async fn list(&self, filter: Option<SandboxFilter>) -> SandboxResult<Vec<SandboxRecord>>;
    async fn update_status(&self, id: &SandboxId, status: SandboxStatus) -> SandboxResult<()>;
    async fn delete(&self, id: &SandboxId) -> SandboxResult<()>;
}

pub struct InMemorySandboxStore {
    records: RwLock<HashMap<SandboxId, SandboxRecord>>,
}

impl InMemorySandboxStore {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemorySandboxStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SandboxStore for InMemorySandboxStore {
    async fn insert(&self, record: SandboxRecord) -> SandboxResult<()> {
        let mut records = self.records.write().await;
        records.insert(record.id.clone(), record);
        Ok(())
    }
    
    async fn get(&self, id: &SandboxId) -> SandboxResult<Option<SandboxRecord>> {
        let records = self.records.read().await;
        Ok(records.get(id).cloned())
    }
    
    async fn list(&self, filter: Option<SandboxFilter>) -> SandboxResult<Vec<SandboxRecord>> {
        let records = self.records.read().await;
        let results: Vec<SandboxRecord> = records
            .values()
            .filter(|r| {
                if let Some(ref f) = filter {
                    if let Some(ref profile) = f.profile {
                        if &r.profile != profile {
                            return false;
                        }
                    }
                    if let Some(ref provider_kind) = f.provider_kind {
                        if &r.provider_kind != provider_kind {
                            return false;
                        }
                    }
                    if let Some(status) = f.status {
                        if r.status != status {
                            return false;
                        }
                    }
                }
                true
            })
            .cloned()
            .collect();
        Ok(results)
    }
    
    async fn update_status(&self, id: &SandboxId, status: SandboxStatus) -> SandboxResult<()> {
        let mut records = self.records.write().await;
        if let Some(record) = records.get_mut(id) {
            record.status = status;
            record.updated_at = Utc::now();
            Ok(())
        } else {
            Err(SandboxError::NotFound(id.to_string()))
        }
    }
    
    async fn delete(&self, id: &SandboxId) -> SandboxResult<()> {
        let mut records = self.records.write().await;
        records.remove(id);
        Ok(())
    }
}
```

- [ ] **Step 4: Export in lib.rs**

```rust
pub mod store;
pub use store::{SandboxStore, InMemorySandboxStore, SandboxRecord, SandboxFilter};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vol-llm-sandbox --test store_tests`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(sandbox): add SandboxStore trait and InMemorySandboxStore"
```

---

Due to the massive scope of this refactor (14 tasks with hundreds of lines of code each), I'll provide a condensed summary of the remaining tasks. Each follows the same TDD pattern: write failing test, implement, verify, commit.

**Task 3: SandboxProvider Trait**
- Define `SandboxProvider` trait with lifecycle methods (create, get, start, pause, resume, stop, destroy)
- Define `BackendSandboxRef` struct
- Test trait method signatures

**Task 4: Refactor Sandbox Trait**
- Remove `name()`, `start()`, `cleanup()`, `bind_metadata()`
- Add `id()`, `status()` methods
- Update `root_path()` to return `Option<&Path>`
- Update LocalSandbox, TmpSandbox, SSHSandbox implementations

**Task 5: SandboxSpec**
- Define `SandboxSpec` struct with profile name, provider kind, config, metadata
- Define `SandboxProviderConfig` enum (Local, Tmp, Ssh variants)
- Test serialization/deserialization

**Task 6: SandboxManager**
- Implement `SandboxManager` with all methods (create, get, list, start, pause, resume, stop, destroy, default, register_instance, load_profiles, register_profile)
- Test lifecycle transitions, filtering, default behavior

**Task 7-9: Provider Implementations**
- Implement `LocalSandboxProvider`, `TmpSandboxProvider`, `SSHSandboxProvider`
- Each with appropriate capabilities and lifecycle behavior

**Task 10: Delete SandboxRegistry**
- Remove `registry.rs`
- Update all imports

**Task 11-12: Update Callers**
- Update `vol-agent-server` to use `SandboxManager`
- Update `vol-llm-runtime` to use `SandboxManager`

**Task 13: Update RPC Protocol**
- Update `sandbox.list` response to include id, status, capabilities
- Add new RPC methods for lifecycle operations

**Task 14: Integration Tests**
- Full lifecycle test
- Provider routing test
- Concurrent access test

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-08-27-sandbox-lifecycle.md`. 

**Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
