# Agent Memory System Design

> **Problem:** Agents currently have no cross-session memory. Each run starts with a blank slate, unable to remember user preferences, project facts, past experiences, or previous decisions. This means users must repeat context every session.

> **Goal:** Design `vol-llm-memory` — a new crate providing layered memory abstractions so agents can store, retrieve, and inject relevant memories across sessions. Storage and retrieval implementations are swappable; the framework defines the contract, not the algorithm.

## Architecture

### Layered Design

```
┌─────────────────────────────────────────────┐
│              MemoryManager                   │
│   (orchestration: add, search, inject)       │
│                                              │
│  ┌────────────────┐  ┌──────────────────┐   │
│  │ MemoryStore    │  │ MemoryRetriever  │   │
│  │ (CRUD)         │  │ (relevance)      │   │
│  └────────────────┘  └──────────────────┘   │
│         │                      │             │
│  InMemoryStore         EmbeddingRetriever    │
│  FileStore             KeywordRetriever      │
│  VecStore              CompositeRetriever    │
└─────────────────────────────────────────────┘
```

**Four layers:**
1. **MemoryItem** — Pure data type, the atomic unit of memory
2. **MemoryStore** — Persistence trait (CRUD), no retrieval logic
3. **MemoryRetriever** — Relevance search trait, no persistence logic
4. **MemoryManager** — Orchestrator combining Store + Retriever for agent use

This separation means Store and Retriever implementations can evolve independently — e.g., a `FileStore` paired with an `EmbeddingRetriever` for semantic search over file-backed memories.

## Core Types

### `MemoryItem`

```rust
pub struct MemoryItem {
    pub id: String,            // UUID
    pub kind: MemoryKind,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub importance: f32,        // 0.0-1.0, higher = retain longer
}
```

`importance` drives retention/eviction policies (future). `tags` enable filtering without full semantic search.

### `MemoryKind`

```rust
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
```

### `MemoryFilter`

Composable filter for list/remove operations:

```rust
pub struct MemoryFilter {
    pub kinds: Option<Vec<MemoryKind>>,
    pub tags: Option<Vec<String>>,       // match any
    pub created_before: Option<DateTime<Utc>>,
    pub created_after: Option<DateTime<Utc>>,
    pub min_importance: Option<f32>,
}
```

## Traits

### `MemoryStore` — Persistence

```rust
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

### `MemoryRetriever` — Relevance Search

```rust
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

### Built-in Store — `InMemoryStore`

```rust
pub struct InMemoryStore {
    items: RwLock<HashMap<String, MemoryItem>>,
}
```

Thread-safe, non-persistent. Sufficient for testing and simple use cases. Future implementations: `FileStore` (JSON files), `VecStore` (vector DB backed retrieval).

## MemoryManager

Combines Store + Retriever into a single agent-facing API:

```rust
pub struct MemoryManager {
    store: Arc<dyn MemoryStore>,
    retriever: Arc<dyn MemoryRetriever>,
}

impl MemoryManager {
    // CRUD passthrough
    pub async fn add(&self, item: MemoryItem) -> Result<String>;
    pub async fn get(&self, id: &str) -> Result<Option<MemoryItem>>;
    pub async fn remove(&self, id: &str) -> Result<bool>;

    // Search
    pub async fn search(&self, query: &str, k: usize) -> Result<Vec<MemoryItem>>;

    /// Format retrieved memories as prompt-injectable text
    pub async fn inject_context(&self, query: &str, max_items: usize) -> Result<String>;

    /// Extract and store memories from a completed session
    async fn summarize_session(&self, messages: &[Message]) -> Result<Vec<MemoryItem>>;
}
```

## Integration Points

### With ReActAgent

`MemoryManager` is optional in `AgentConfig`:

```rust
pub struct AgentConfig {
    // ... existing fields ...
    pub memory: Option<Arc<MemoryManager>>,
}
```

When present, the agent loop:
1. On startup: `inject_context(user_input, k)` → prepend to system prompt
2. On shutdown (optional): `summarize_session(messages)` → extract and store memories

### With Existing RAG Module

RAG (`vol-llm-agent/src/rag/`) handles **external document retrieval** — searching project docs, codebases, knowledge bases. Memory handles **agent's own accumulated experience** — what it has learned from past interactions. They are complementary:

| | RAG | Memory |
|---|---|---|
| Source | External documents | Agent's own experience |
| Purpose | Ground responses in facts | Remember preferences, lessons |
| Retrieval | EmbeddingStore (vector) | MemoryRetriever (pluggable) |

## Data Flow

```
Session 1: User asks about project
  → Agent answers, learns user prefers Rust
  → Agent extracts & stores: MemoryItem { kind: UserPreference, content: "...", tags: ["rust"] }

Session 2: User asks about database
  → Agent startup: inject_context("database") 
  → Retrieves: MemoryItem { kind: ProjectFact, content: "Uses TDengine" }
  → Prepended to system prompt: "Remember: This project uses TDengine."
  → Agent answers with correct context
```

## What's NOT in Scope

- Memory extraction via LLM (no "summarize this conversation" implementation yet — `summarize_session` is a stub)
- Memory eviction/aging policies (importance field reserved for future)
- Memory conflict resolution (e.g., user changed their mind about something)
- TUI or HTTP UI for browsing/editing memories

## Crate Structure

```
crates/vol-llm-memory/
├── Cargo.toml
├── src/
│   ├── lib.rs            # Re-exports
│   ├── item.rs           # MemoryItem, MemoryKind, MemoryFilter
│   ├── store.rs          # MemoryStore trait
│   ├── retriever.rs      # MemoryRetriever trait
│   ├── manager.rs        # MemoryManager
│   └── memory_store.rs   # InMemoryStore impl
├── tests/
│   └── memory_test.rs    # Integration test
└── examples/
    └── memory_example.rs # Basic usage
```

## Dependencies

- `async-trait` — for async trait methods
- `tokio` — for async runtime (RwLock, etc.)
- `serde` / `serde_json` — for serialization (future FileStore)
- `uuid` — for memory IDs
- `chrono` — for timestamps
