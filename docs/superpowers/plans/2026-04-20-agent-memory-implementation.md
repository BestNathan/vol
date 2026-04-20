# Agent Memory System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `vol-llm-memory` crate providing layered memory abstractions so agents can store, retrieve, and inject relevant memories across sessions.

**Architecture:** Four-layer design: MemoryItem (data) → MemoryStore (CRUD trait) → MemoryRetriever (relevance search trait) → MemoryManager (orchestrator combining Store + Retriever). Store and Retriever implementations are swappable.

**Tech Stack:** async-trait, tokio, serde, serde_json, uuid, chrono

---

## Context

**Problem:** Agents have no cross-session memory. Each run starts blank, unable to remember user preferences, project facts, past experiences.

**Solution:** New `vol-llm-memory` crate with trait-based abstractions. `InMemoryStore` as the first built-in Store implementation. `KeywordRetriever` as the first built-in Retriever (simple text matching, no embeddings yet). `MemoryManager` orchestrates both for agent use.

**Key design decisions:**
1. Store and Retriever are separate traits — they evolve independently
2. `MemoryItem` has an `importance: f32` field reserved for future eviction policies
3. `MemoryKind` enum has 4 variants: UserPreference, ProjectFact, Experience, ConversationSummary
4. `MemoryFilter` enables tag/kind/time-based filtering without full retrieval
5. `summarize_session` in MemoryManager is a stub — LLM extraction is NOT in scope

See [docs/superpowers/specs/2026-04-20-agent-memory-system-design.md](docs/superpowers/specs/2026-04-20-agent-memory-system-design.md) for full design spec.

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `crates/vol-llm-memory/Cargo.toml` | Create | Crate manifest |
| `crates/vol-llm-memory/src/lib.rs` | Create | Re-exports |
| `crates/vol-llm-memory/src/item.rs` | Create | MemoryItem, MemoryKind, MemoryFilter |
| `crates/vol-llm-memory/src/store.rs` | Create | MemoryStore trait |
| `crates/vol-llm-memory/src/retriever.rs` | Create | MemoryRetriever trait |
| `crates/vol-llm-memory/src/memory_store.rs` | Create | InMemoryStore impl |
| `crates/vol-llm-memory/src/retrievers/keyword.rs` | Create | KeywordRetriever impl |
| `crates/vol-llm-memory/src/retrievers/mod.rs` | Create | Retriever module |
| `crates/vol-llm-memory/src/manager.rs` | Create | MemoryManager orchestrator |
| `crates/vol-llm-memory/tests/memory_test.rs` | Create | Integration tests |
| `Cargo.toml` | Modify | Add workspace member + deps |

---

### Task 1: Create vol-llm-memory Crate Skeleton

**Files:**
- Create: `crates/vol-llm-memory/Cargo.toml`
- Create: `crates/vol-llm-memory/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-memory"
version.workspace = true
edition.workspace = true

[dependencies]
async-trait = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
uuid = { workspace = true, features = ["v4"] }
chrono = { workspace = true }
vol-llm-core = { workspace = true }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create lib.rs**

```rust
//! vol-llm-memory: Layered memory abstractions for cross-session agent memory.
//!
//! # Architecture
//!
//! ```text
//! MemoryItem (data) → MemoryStore (CRUD trait) → MemoryRetriever (search trait) → MemoryManager (orchestrator)
//! ```
//!
//! # Quick Start
//!
//! ```rust
//! use vol_llm_memory::{MemoryManager, MemoryItem, MemoryKind, InMemoryStore, KeywordRetriever};
//!
//! #[tokio::main]
//! async fn main() {
//!     let store = InMemoryStore::new();
//!     let retriever = KeywordRetriever::new(Box::new(store));
//!     let manager = MemoryManager::new(Box::new(store), Box::new(retriever));
//!
//!     let item = MemoryItem::new(MemoryKind::UserPreference, "User prefers Rust");
//!     manager.add(item).await.unwrap();
//!
//!     let results = manager.search("Rust", 5).await.unwrap();
//!     assert_eq!(results.len(), 1);
//! }
//! ```

mod item;
mod manager;
mod memory_store;
mod retriever;
mod retrievers;
mod store;

pub use item::{MemoryFilter, MemoryItem, MemoryKind};
pub use manager::MemoryManager;
pub use memory_store::InMemoryStore;
pub use retriever::MemoryRetriever;
pub use retrievers::keyword::KeywordRetriever;
pub use store::MemoryStore;

/// Result type for memory operations
pub type Result<T> = std::result::Result<T, MemoryError>;

/// Error type for memory operations
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("Memory not found: {0}")]
    NotFound(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Retrieval error: {0}")]
    Retrieval(String),
}
```

- [ ] **Step 3: Create empty module files**

Create these files so `cargo check` passes:

`crates/vol-llm-memory/src/item.rs` — will be filled in Task 2.
`crates/vol-llm-memory/src/store.rs` — will be filled in Task 3.
`crates/vol-llm-memory/src/retriever.rs` — will be filled in Task 4.
`crates/vol-llm-memory/src/memory_store.rs` — will be filled in Task 5.
`crates/vol-llm-memory/src/retrievers/mod.rs` — `pub mod keyword;`
`crates/vol-llm-memory/src/retrievers/keyword.rs` — will be filled in Task 4.
`crates/vol-llm-memory/src/manager.rs` — will be filled in Task 6.

- [ ] **Step 4: Add workspace member and dependencies**

Add `"crates/vol-llm-memory"` to `members` array in root `Cargo.toml`.

Add to `[workspace.dependencies]`:
```toml
vol-llm-memory = { path = "crates/vol-llm-memory" }
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-memory
```

Expected: Compiles with errors for empty module files (we'll fill them in subsequent tasks).

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-memory/ Cargo.toml
git commit -m "feat: create vol-llm-memory crate skeleton"
```

---

### Task 2: Implement MemoryItem, MemoryKind, MemoryFilter

**Files:**
- Modify: `crates/vol-llm-memory/src/item.rs`

- [ ] **Step 1: Write item.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Kind of memory item, categorizing what type of experience it represents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryKind {
    /// User preferences, communication style, code style
    UserPreference,
    /// Project architecture, tech stack, constraints
    ProjectFact,
    /// Tool success/failure patterns, gotchas, tips
    Experience,
    /// Past session summaries and key decisions
    ConversationSummary,
}

impl std::fmt::Display for MemoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryKind::UserPreference => write!(f, "UserPreference"),
            MemoryKind::ProjectFact => write!(f, "ProjectFact"),
            MemoryKind::Experience => write!(f, "Experience"),
            MemoryKind::ConversationSummary => write!(f, "ConversationSummary"),
        }
    }
}

/// Atomic unit of memory — a single piece of information the agent has accumulated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub kind: MemoryKind,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub importance: f32,
}

impl MemoryItem {
    pub fn new(kind: MemoryKind, content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            content: content.into(),
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            importance: 0.5,
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }
}

/// Composable filter for list/remove operations.
#[derive(Debug, Clone, Default)]
pub struct MemoryFilter {
    pub kinds: Option<Vec<MemoryKind>>,
    pub tags: Option<Vec<String>>,
    pub created_before: Option<DateTime<Utc>>,
    pub created_after: Option<DateTime<Utc>>,
    pub min_importance: Option<f32>,
}

impl MemoryFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kinds(mut self, kinds: Vec<MemoryKind>) -> Self {
        self.kinds = Some(kinds);
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn created_after(mut self, dt: DateTime<Utc>) -> Self {
        self.created_after = Some(dt);
        self
    }

    pub fn created_before(mut self, dt: DateTime<Utc>) -> Self {
        self.created_before = Some(dt);
        self
    }

    pub fn min_importance(mut self, min: f32) -> Self {
        self.min_importance = Some(min);
        self
    }

    pub fn matches(&self, item: &MemoryItem) -> bool {
        if let Some(ref kinds) = self.kinds {
            if !kinds.contains(&item.kind) {
                return false;
            }
        }
        if let Some(ref filter_tags) = self.tags {
            if filter_tags.is_empty() || item.tags.is_empty() {
                return false;
            }
            if !filter_tags.iter().any(|t| item.tags.contains(t)) {
                return false;
            }
        }
        if let Some(ref before) = self.created_before {
            if item.created_at >= *before {
                return false;
            }
        }
        if let Some(ref after) = self.created_after {
            if item.created_at <= *after {
                return false;
            }
        }
        if let Some(min_imp) = self.min_importance {
            if item.importance < min_imp {
                return false;
            }
        }
        true
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-memory
```

Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-memory/src/item.rs
git commit -m "feat: add MemoryItem, MemoryKind, MemoryFilter types"
```

---

### Task 3: Implement MemoryStore Trait

**Files:**
- Modify: `crates/vol-llm-memory/src/store.rs`

- [ ] **Step 1: Write store.rs**

```rust
use async_trait::async_trait;

use crate::item::{MemoryFilter, MemoryItem};
use crate::{MemoryError, Result};

/// Persistence trait for memory CRUD operations.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn add(&self, item: MemoryItem) -> Result<String>;
    async fn get(&self, id: &str) -> Result<Option<MemoryItem>>;
    async fn remove(&self, id: &str) -> Result<bool>;
    async fn update(&self, item: MemoryItem) -> Result<()>;
    async fn list(&self, filter: MemoryFilter) -> Result<Vec<MemoryItem>>;
    async fn remove_many(&self, filter: MemoryFilter) -> Result<usize>;
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-memory
```

Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-memory/src/store.rs
git commit -m "feat: add MemoryStore persistence trait"
```

---

### Task 4: Implement MemoryRetriever Trait + KeywordRetriever

**Files:**
- Modify: `crates/vol-llm-memory/src/retriever.rs`
- Modify: `crates/vol-llm-memory/src/retrievers/mod.rs`
- Modify: `crates/vol-llm-memory/src/retrievers/keyword.rs`

- [ ] **Step 1: Write retriever.rs**

```rust
use async_trait::async_trait;

use crate::item::{MemoryFilter, MemoryItem};
use crate::{MemoryError, Result};

/// Relevance search trait for retrieving memories.
#[async_trait]
pub trait MemoryRetriever: Send + Sync {
    async fn retrieve(&self, query: &str, k: usize) -> Result<Vec<MemoryItem>>;

    async fn retrieve_with_filter(
        &self,
        query: &str,
        k: usize,
        filter: MemoryFilter,
    ) -> Result<Vec<MemoryItem>>;
}
```

- [ ] **Step 2: Write retrievers/mod.rs**

```rust
pub mod keyword;
```

- [ ] **Step 3: Write retrievers/keyword.rs**

```rust
use async_trait::async_trait;

use crate::item::{MemoryFilter, MemoryItem};
use crate::retriever::MemoryRetriever;
use crate::store::MemoryStore;
use crate::Result;

/// Simple keyword-based retriever.
pub struct KeywordRetriever {
    store: Box<dyn MemoryStore>,
}

impl KeywordRetriever {
    pub fn new(store: Box<dyn MemoryStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl MemoryRetriever for KeywordRetriever {
    async fn retrieve(&self, query: &str, k: usize) -> Result<Vec<MemoryItem>> {
        self.retrieve_with_filter(query, k, MemoryFilter::default()).await
    }

    async fn retrieve_with_filter(
        &self,
        query: &str,
        k: usize,
        filter: MemoryFilter,
    ) -> Result<Vec<MemoryItem>> {
        let all = self.store.list(filter).await?;
        let query_terms: Vec<&str> = query.split_whitespace().map(|s| s.to_lowercase()).collect();

        let mut scored: Vec<(f32, MemoryItem)> = all
            .into_iter()
            .map(|item| {
                let score = score_item(&item.content, &query_terms);
                (score, item)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let results: Vec<MemoryItem> = scored
            .into_iter()
            .filter(|(score, _)| *score > 0.0)
            .take(k)
            .map(|(_, item)| item)
            .collect();

        Ok(results)
    }
}

fn score_item(content: &str, query_terms: &[&str]) -> f32 {
    let content_lower = content.to_lowercase();
    let mut score = 0.0;
    for term in query_terms {
        let matches = content_lower.matches(*term).count();
        if matches > 0 {
            score += (matches as f32) / (content.len().max(1) as f32 / 100.0);
        }
    }
    score
}
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-memory
```

Expected: Compiles successfully.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-memory/src/retriever.rs crates/vol-llm-memory/src/retrievers/mod.rs crates/vol-llm-memory/src/retrievers/keyword.rs
git commit -m "feat: add MemoryRetriever trait and KeywordRetriever impl"
```

---

### Task 5: Implement InMemoryStore

**Files:**
- Modify: `crates/vol-llm-memory/src/memory_store.rs`

- [ ] **Step 1: Write memory_store.rs**

```rust
use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::item::{MemoryFilter, MemoryItem};
use crate::store::MemoryStore;
use crate::{MemoryError, Result};

/// Thread-safe, non-persistent in-memory store.
#[derive(Clone)]
pub struct InMemoryStore {
    items: Arc<RwLock<HashMap<String, MemoryItem>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            items: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn add(&self, item: MemoryItem) -> Result<String> {
        let id = item.id.clone();
        self.items.write().await.insert(id.clone(), item);
        Ok(id)
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryItem>> {
        Ok(self.items.read().await.get(id).cloned())
    }

    async fn remove(&self, id: &str) -> Result<bool> {
        Ok(self.items.write().await.remove(id).is_some())
    }

    async fn update(&self, item: MemoryItem) -> Result<()> {
        let id = item.id.clone();
        let mut items = self.items.write().await;
        if items.contains_key(&id) {
            items.insert(id, item);
            Ok(())
        } else {
            Err(MemoryError::NotFound(format!(
                "Memory item with id '{}' not found",
                id
            )))
        }
    }

    async fn list(&self, filter: MemoryFilter) -> Result<Vec<MemoryItem>> {
        let items = self.items.read().await;
        Ok(items.values().filter(|item| filter.matches(item)).cloned().collect())
    }

    async fn remove_many(&self, filter: MemoryFilter) -> Result<usize> {
        let mut items = self.items.write().await;
        let ids_to_remove: Vec<String> = items
            .iter()
            .filter(|(_, item)| filter.matches(item))
            .map(|(id, _)| id.clone())
            .collect();
        let count = ids_to_remove.len();
        for id in ids_to_remove {
            items.remove(&id);
        }
        Ok(count)
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-memory
```

Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-memory/src/memory_store.rs
git commit -m "feat: add InMemoryStore implementation"
```

---

### Task 6: Implement MemoryManager

**Files:**
- Modify: `crates/vol-llm-memory/src/manager.rs`

- [ ] **Step 1: Write manager.rs**

```rust
use crate::item::{MemoryFilter, MemoryItem};
use crate::retriever::MemoryRetriever;
use crate::store::MemoryStore;
use crate::Result;

/// Orchestrator combining Store + Retriever into a single agent-facing API.
pub struct MemoryManager {
    store: Box<dyn MemoryStore>,
    retriever: Box<dyn MemoryRetriever>,
}

impl MemoryManager {
    pub fn new(store: Box<dyn MemoryStore>, retriever: Box<dyn MemoryRetriever>) -> Self {
        Self { store, retriever }
    }

    pub async fn add(&self, item: MemoryItem) -> Result<String> {
        self.store.add(item).await
    }

    pub async fn get(&self, id: &str) -> Result<Option<MemoryItem>> {
        self.store.get(id).await
    }

    pub async fn remove(&self, id: &str) -> Result<bool> {
        self.store.remove(id).await
    }

    pub async fn search(&self, query: &str, k: usize) -> Result<Vec<MemoryItem>> {
        self.retriever.retrieve(query, k).await
    }

    pub async fn search_with_filter(
        &self,
        query: &str,
        k: usize,
        filter: MemoryFilter,
    ) -> Result<Vec<MemoryItem>> {
        self.retriever.retrieve_with_filter(query, k, filter).await
    }

    /// Format retrieved memories as prompt-injectable text.
    pub async fn inject_context(&self, query: &str, max_items: usize) -> Result<String> {
        let memories = self.search(query, max_items).await?;
        if memories.is_empty() {
            return Ok(String::new());
        }

        let mut output = String::from("Relevant memories:\n");
        for (i, mem) in memories.iter().enumerate() {
            output.push_str(&format!("{}. [{}] {}\n", i + 1, mem.kind, mem.content));
            if !mem.tags.is_empty() {
                output.push_str(&format!("   Tags: {}\n", mem.tags.join(", ")));
            }
        }
        Ok(output)
    }

    pub async fn list(&self, filter: MemoryFilter) -> Result<Vec<MemoryItem>> {
        self.store.list(filter).await
    }

    pub async fn remove_many(&self, filter: MemoryFilter) -> Result<usize> {
        self.store.remove_many(filter).await
    }

    /// Stub: Extract and store memories from a completed session.
    /// TODO: Implement LLM-based extraction when memory extraction is in scope.
    #[allow(dead_code)]
    pub async fn summarize_session(
        &self,
        _messages: &[vol_llm_core::Message],
    ) -> Result<Vec<MemoryItem>> {
        Ok(Vec::new())
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-memory
```

Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-memory/src/manager.rs
git commit -m "feat: add MemoryManager orchestrator"
```

---

### Task 7: Integration Tests

**Files:**
- Create: `crates/vol-llm-memory/tests/memory_test.rs`

- [ ] **Step 1: Write memory_test.rs**

```rust
use vol_llm_memory::{
    InMemoryStore, KeywordRetriever, MemoryFilter, MemoryItem, MemoryKind, MemoryManager,
    MemoryRetriever, MemoryStore,
};

#[tokio::test]
async fn test_add_and_get_memory() {
    let store = InMemoryStore::new();
    let item = MemoryItem::new(MemoryKind::UserPreference, "User prefers Rust");
    let id = store.add(item).await.unwrap();
    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.content, "User prefers Rust");
    assert_eq!(retrieved.kind, MemoryKind::UserPreference);
}

#[tokio::test]
async fn test_remove_memory() {
    let store = InMemoryStore::new();
    let item = MemoryItem::new(MemoryKind::ProjectFact, "Uses TDengine");
    let id = store.add(item).await.unwrap();
    assert!(store.remove(&id).await.unwrap());
    assert!(store.get(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_update_memory() {
    let store = InMemoryStore::new();
    let item = MemoryItem::new(MemoryKind::Experience, "Old content");
    let id = store.add(item).await.unwrap();
    let mut updated = store.get(&id).await.unwrap().unwrap();
    updated.content = "New content".to_string();
    store.update(updated).await.unwrap();
    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.content, "New content");
}

#[tokio::test]
async fn test_update_nonexistent_memory() {
    let store = InMemoryStore::new();
    let item = MemoryItem::new(MemoryKind::Experience, "test");
    let result = store.update(item).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_with_filter() {
    let store = InMemoryStore::new();
    store.add(MemoryItem::new(MemoryKind::UserPreference, "Prefers Rust").with_tags(vec!["rust".to_string()])).await;
    store.add(MemoryItem::new(MemoryKind::ProjectFact, "Uses TDengine").with_tags(vec!["database".to_string()])).await;
    let filter = MemoryFilter::new().kinds(vec![MemoryKind::UserPreference]);
    let results = store.list(filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, MemoryKind::UserPreference);
}

#[tokio::test]
async fn test_remove_many() {
    let store = InMemoryStore::new();
    store.add(MemoryItem::new(MemoryKind::Experience, "exp1")).await;
    store.add(MemoryItem::new(MemoryKind::Experience, "exp2")).await;
    store.add(MemoryItem::new(MemoryKind::UserPreference, "pref1")).await;
    let filter = MemoryFilter::new().kinds(vec![MemoryKind::Experience]);
    let removed = store.remove_many(filter).await.unwrap();
    assert_eq!(removed, 2);
    let all = store.list(MemoryFilter::new()).await.unwrap();
    assert_eq!(all.len(), 1);
}

#[tokio::test]
async fn test_keyword_retriever() {
    let store = InMemoryStore::new();
    store.add(MemoryItem::new(MemoryKind::ProjectFact, "This project uses TDengine database")).await;
    store.add(MemoryItem::new(MemoryKind::UserPreference, "User likes Rust programming")).await;
    let retriever = KeywordRetriever::new(Box::new(store));
    let results = retriever.retrieve("TDengine database", 5).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, MemoryKind::ProjectFact);
}

#[tokio::test]
async fn test_keyword_retriever_with_filter() {
    let store = InMemoryStore::new();
    store.add(MemoryItem::new(MemoryKind::ProjectFact, "Uses Rust")).await;
    store.add(MemoryItem::new(MemoryKind::UserPreference, "Rust is great")).await;
    let retriever = KeywordRetriever::new(Box::new(store));
    let filter = MemoryFilter::new().kinds(vec![MemoryKind::UserPreference]);
    let results = retriever.retrieve_with_filter("Rust", 5, filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, MemoryKind::UserPreference);
}

#[tokio::test]
async fn test_memory_manager_add_search() {
    let store = InMemoryStore::new();
    let retriever = KeywordRetriever::new(Box::new(store.clone()));
    let manager = MemoryManager::new(Box::new(store), retriever);
    let item = MemoryItem::new(MemoryKind::UserPreference, "User prefers Rust for backend development")
        .with_tags(vec!["rust".to_string(), "backend".to_string()]);
    manager.add(item).await.unwrap();
    let results = manager.search("Rust backend", 5).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_memory_manager_inject_context() {
    let store = InMemoryStore::new();
    let retriever = KeywordRetriever::new(Box::new(store.clone()));
    let manager = MemoryManager::new(Box::new(store), retriever);
    manager.add(MemoryItem::new(MemoryKind::ProjectFact, "Uses TDengine")).await;
    let injected = manager.inject_context("TDengine", 5).await.unwrap();
    assert!(injected.contains("Uses TDengine"));
    assert!(injected.contains("ProjectFact"));
}

#[tokio::test]
async fn test_memory_manager_inject_context_empty() {
    let store = InMemoryStore::new();
    let retriever = KeywordRetriever::new(Box::new(store.clone()));
    let manager = MemoryManager::new(Box::new(store), retriever);
    let injected = manager.inject_context("nonexistent", 5).await.unwrap();
    assert!(injected.is_empty());
}

#[tokio::test]
async fn test_memory_filter_matches() {
    let item = MemoryItem::new(MemoryKind::Experience, "test content")
        .with_tags(vec!["rust".to_string()])
        .with_importance(0.8);

    assert!(MemoryFilter::new().kinds(vec![MemoryKind::Experience]).matches(&item));
    assert!(!MemoryFilter::new().kinds(vec![MemoryKind::UserPreference]).matches(&item));
    assert!(MemoryFilter::new().tags(vec!["rust".to_string()]).matches(&item));
    assert!(!MemoryFilter::new().tags(vec!["python".to_string()]).matches(&item));
    assert!(MemoryFilter::new().min_importance(0.5).matches(&item));
    assert!(!MemoryFilter::new().min_importance(0.9).matches(&item));
}

#[tokio::test]
async fn test_memory_item_builder() {
    let item = MemoryItem::new(MemoryKind::ConversationSummary, "Good session")
        .with_tags(vec!["productive".to_string()])
        .with_importance(0.9);
    assert_eq!(item.tags, vec!["productive"]);
    assert!((item.importance - 0.9).abs() < f32::EPSILON);
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vol-llm-memory
```

Expected: All 13 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-memory/tests/memory_test.rs
git commit -m "test: add integration tests for vol-llm-memory"
```

---

### Task 8: Full Workspace Verification

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run all existing tests**

```bash
cargo test --workspace --lib
```

Expected: All existing tests pass.

---

## Summary of Changes

| Crate | Files Changed | Purpose |
|-------|---------------|---------|
| `vol-llm-memory` | **new** | Memory system crate |
| `Cargo.toml` (root) | Modify | Add workspace member + dependency |

### vol-llm-memory Internal Structure

| File | Purpose |
|------|---------|
| `src/item.rs` | MemoryItem, MemoryKind, MemoryFilter |
| `src/store.rs` | MemoryStore trait |
| `src/retriever.rs` | MemoryRetriever trait |
| `src/retrievers/keyword.rs` | KeywordRetriever impl |
| `src/retrievers/mod.rs` | Retriever module |
| `src/memory_store.rs` | InMemoryStore impl |
| `src/manager.rs` | MemoryManager orchestrator |
| `tests/memory_test.rs` | Integration tests (13 tests) |
