# vol-llm-context Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `vol-llm-context` crate providing structured context management with attention-zone-aware sorting, budget-driven compression, and trait-based extensibility for LLM agents.

**Architecture:** `ContextBlock` (messages + anchor) → `ContextContributor` trait (extension) → `ContextBuilder` (orchestrator). Contributors produce `Vec<Message>` directly. Budget-driven compression when middle-zone content exceeds token limits.

**Tech Stack:** async-trait, tokio, serde, serde_json, vol-llm-core, vol-llm-memory, vol-llm-skill

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `crates/vol-llm-context/Cargo.toml` | Create | Crate manifest |
| `crates/vol-llm-context/src/lib.rs` | Create | Re-exports |
| `crates/vol-llm-context/src/block.rs` | Create | `ContextBlock`, `AttentionAnchor`, `TokenBudget` |
| `crates/vol-llm-context/src/contributor.rs` | Create | `ContextContributor` trait |
| `crates/vol-llm-context/src/builder.rs` | Create | `ContextBuilder` with builder pattern |
| `crates/vol-llm-context/src/builtin/mod.rs` | Create | Built-in contributors module |
| `crates/vol-llm-context/src/builtin/role.rs` | Create | `RoleContributor` |
| `crates/vol-llm-context/src/builtin/task.rs` | Create | `TaskContributor` |
| `crates/vol-llm-context/src/builtin/rules.rs` | Create | `RulesContributor` |
| `crates/vol-llm-context/tests/context_test.rs` | Create | Integration tests |
| `Cargo.toml` | Modify | Add workspace member + dependency |

---

### Task 1: Create vol-llm-context Crate Skeleton

**Files:**
- Create: `crates/vol-llm-context/Cargo.toml`
- Create: `crates/vol-llm-context/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-context"
version.workspace = true
edition.workspace = true

[dependencies]
async-trait = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
vol-llm-core = { workspace = true }
vol-llm-memory = { workspace = true }
vol-llm-skill = { workspace = true }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create lib.rs**

```rust
//! vol-llm-context: Structured context management for LLM agents.
//!
//! Attention-zone-aware sorting, budget-driven compression, and trait-based extensibility.
//!
//! # Quick Start
//!
//! ```rust
//! use vol_llm_context::ContextBuilder;
//! use vol_llm_context::builtin::{RoleContributor, TaskContributor, RulesContributor};
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut builder = ContextBuilder::builder()
//!         .contributor(RoleContributor::new("You are a coding assistant"))
//!         .contributor(TaskContributor::new("Help users modify code"))
//!         .contributor(RulesContributor::new(vec!["Use snake_case"]))
//!         .token_limit(100_000)
//!         .build();
//!
//!     let output = builder.build().await;
//!     assert!(!output.messages.is_empty());
//! }
//! ```

pub mod block;
pub mod contributor;
pub mod builder;
pub mod builtin;

pub use block::{AttentionAnchor, ContextBlock, TokenBudget};
pub use builder::{ContextBuilder, ContextOutput};
pub use contributor::ContextContributor;
```

- [ ] **Step 3: Create empty module files**

Create these files so `cargo check` passes:

`crates/vol-llm-context/src/block.rs` — will be filled in Task 2.
`crates/vol-llm-context/src/contributor.rs` — will be filled in Task 3.
`crates/vol-llm-context/src/builder.rs` — will be filled in Task 4.
`crates/vol-llm-context/src/builtin/mod.rs` — `pub mod role; pub mod task; pub mod rules;`
`crates/vol-llm-context/src/builtin/role.rs` — will be filled in Task 5.
`crates/vol-llm-context/src/builtin/task.rs` — will be filled in Task 5.
`crates/vol-llm-context/src/builtin/rules.rs` — will be filled in Task 5.

- [ ] **Step 4: Add workspace member and dependency**

Add `"crates/vol-llm-context"` to `members` array in root `Cargo.toml`.

Add to `[workspace.dependencies]`:
```toml
vol-llm-context = { path = "crates/vol-llm-context" }
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-context
```

Expected: Compiles successfully.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-context/ Cargo.toml
git commit -m "feat: create vol-llm-context crate skeleton"
```

---

### Task 2: Implement Core Types (ContextBlock, AttentionAnchor, TokenBudget)

**Files:**
- Modify: `crates/vol-llm-context/src/block.rs`

- [ ] **Step 1: Write the tests first**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;

    #[test]
    fn test_attention_anchor_sorting() {
        let head = AttentionAnchor::Head(0);
        let middle = AttentionAnchor::Middle(0);
        let tail = AttentionAnchor::Tail(0);

        assert!(head < middle);
        assert!(middle < tail);
        assert!(AttentionAnchor::Head(0) < AttentionAnchor::Head(1));
    }

    #[test]
    fn test_context_block() {
        let block = ContextBlock {
            messages: vec![Message::system("hello".to_string())],
            anchor: AttentionAnchor::Middle(5),
        };
        assert_eq!(block.messages.len(), 1);
        assert!(matches!(block.anchor, AttentionAnchor::Middle(5)));
    }

    #[test]
    fn test_token_budget() {
        let budget = TokenBudget::new(1000);
        assert_eq!(budget.remaining(), 1000);
        assert!(!budget.is_exceeded());

        let budget = budget.with_used(800);
        assert_eq!(budget.remaining(), 200);
        assert!(!budget.is_exceeded());

        let budget = budget.with_used(1200);
        assert!(budget.is_exceeded());
    }

    #[test]
    fn test_token_estimate_messages() {
        let msgs = vec![
            Message::system("hello world".to_string()),
            Message::user("test message here".to_string()),
        ];
        let estimate = TokenBudget::estimate_tokens(&msgs);
        assert!(estimate > 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vol-llm-context --lib`
Expected: FAIL with unresolved imports.

- [ ] **Step 3: Implement block.rs**

```rust
use vol_llm_core::Message;

/// Attention zone with position value for sorting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttentionAnchor {
    Head(u32),
    Middle(u32),
    Tail(u32),
}

impl AttentionAnchor {
    fn zone_priority(&self) -> u8 {
        match self {
            AttentionAnchor::Head(_) => 0,
            AttentionAnchor::Middle(_) => 1,
            AttentionAnchor::Tail(_) => 2,
        }
    }

    fn position(&self) -> u32 {
        match self {
            AttentionAnchor::Head(p) | AttentionAnchor::Middle(p) | AttentionAnchor::Tail(p) => *p,
        }
    }
}

impl Ord for AttentionAnchor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.zone_priority()
            .cmp(&other.zone_priority())
            .then_with(|| self.position().cmp(&other.position()))
    }
}

impl PartialOrd for AttentionAnchor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A unit of context — contributor-formatted messages with an attention anchor.
#[derive(Debug, Clone)]
pub struct ContextBlock {
    pub messages: Vec<Message>,
    pub anchor: AttentionAnchor,
}

/// Token budget tracker for compression decisions.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    pub total: usize,
    pub used: usize,
}

impl TokenBudget {
    pub fn new(total: usize) -> Self {
        Self { total, used: 0 }
    }

    pub fn with_used(mut self, used: usize) -> Self {
        self.used = used;
        self
    }

    pub fn remaining(&self) -> usize {
        self.total.saturating_sub(self.used)
    }

    pub fn is_exceeded(&self) -> bool {
        self.used > self.total
    }

    /// Estimate token count from messages using a simple char/4 heuristic.
    pub fn estimate_tokens(messages: &[Message]) -> usize {
        messages.iter().map(|m| {
            m.content.as_ref().map_or(0, |c| c.len()) / 4
        }).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_anchor_sorting() {
        let head = AttentionAnchor::Head(0);
        let middle = AttentionAnchor::Middle(0);
        let tail = AttentionAnchor::Tail(0);

        assert!(head < middle);
        assert!(middle < tail);
        assert!(AttentionAnchor::Head(0) < AttentionAnchor::Head(1));
    }

    #[test]
    fn test_context_block() {
        let block = ContextBlock {
            messages: vec![Message::system("hello".to_string())],
            anchor: AttentionAnchor::Middle(5),
        };
        assert_eq!(block.messages.len(), 1);
        assert!(matches!(block.anchor, AttentionAnchor::Middle(5)));
    }

    #[test]
    fn test_token_budget() {
        let budget = TokenBudget::new(1000);
        assert_eq!(budget.remaining(), 1000);
        assert!(!budget.is_exceeded());

        let budget = budget.with_used(800);
        assert_eq!(budget.remaining(), 200);
        assert!(!budget.is_exceeded());

        let budget = budget.with_used(1200);
        assert!(budget.is_exceeded());
    }

    #[test]
    fn test_token_estimate_messages() {
        let msgs = vec![
            Message::system("hello world".to_string()),
            Message::user("test message here".to_string()),
        ];
        let estimate = TokenBudget::estimate_tokens(&msgs);
        assert!(estimate > 0);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vol-llm-context --lib`
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-context/src/block.rs
git commit -m "feat: add ContextBlock, AttentionAnchor, TokenBudget types"
```

---

### Task 3: Implement ContextContributor Trait

**Files:**
- Modify: `crates/vol-llm-context/src/contributor.rs`

- [ ] **Step 1: Write the test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{ContextBlock, AttentionAnchor};
    use vol_llm_core::Message;

    struct TestContributor {
        name: String,
        call_count: std::sync::atomic::AtomicUsize,
    }

    impl TestContributor {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                call_count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl ContextContributor for TestContributor {
        fn name(&self) -> &str { &self.name }

        async fn contribute(&self) -> Vec<ContextBlock> {
            self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            vec![]
        }

        fn compress(&mut self) {}
    }

    #[tokio::test]
    async fn test_contributor_name() {
        let c = TestContributor::new("test");
        assert_eq!(c.name(), "test");
    }

    #[tokio::test]
    async fn test_contributor_contribute() {
        let c = TestContributor::new("test");
        let blocks = c.contribute().await;
        assert!(blocks.is_empty());
        assert_eq!(c.call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vol-llm-context --lib`
Expected: FAIL — `ContextContributor` trait not defined.

- [ ] **Step 3: Implement contributor.rs**

```rust
use async_trait::async_trait;

use crate::block::{ContextBlock, TokenBudget};

/// Trait for context contributors that produce context blocks.
#[async_trait]
pub trait ContextContributor: Send + Sync {
    /// Unique name for this contributor.
    fn name(&self) -> &str;

    /// Contribute context blocks. Called after compress() to get reduced content.
    async fn contribute(&self) -> Vec<ContextBlock>;

    /// Compress internal state to reduce future output size.
    /// Default: no-op (not compressible).
    fn compress(&mut self) {}

    /// Estimate the token size of this contributor's contribution.
    /// Default: contribute() then estimate. Override for efficiency.
    async fn estimate_size(&self) -> usize {
        let blocks = self.contribute().await;
        let total_messages: Vec<_> = blocks.iter().flat_map(|b| &b.messages).collect();
        TokenBudget::estimate_tokens(&total_messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{ContextBlock, AttentionAnchor};
    use vol_llm_core::Message;

    struct TestContributor {
        name: String,
        call_count: std::sync::atomic::AtomicUsize,
    }

    impl TestContributor {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                call_count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl ContextContributor for TestContributor {
        fn name(&self) -> &str { &self.name }

        async fn contribute(&self) -> Vec<ContextBlock> {
            self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            vec![]
        }

        fn compress(&mut self) {}
    }

    #[tokio::test]
    async fn test_contributor_name() {
        let c = TestContributor::new("test");
        assert_eq!(c.name(), "test");
    }

    #[tokio::test]
    async fn test_contributor_contribute() {
        let c = TestContributor::new("test");
        let blocks = c.contribute().await;
        assert!(blocks.is_empty());
        assert_eq!(c.call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vol-llm-context --lib`
Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-context/src/contributor.rs
git commit -m "feat: add ContextContributor trait"
```

---

### Task 4: Implement ContextBuilder

**Files:**
- Modify: `crates/vol-llm-context/src/builder.rs`

- [ ] **Step 1: Write the tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{ContextBlock, AttentionAnchor, TokenBudget};
    use crate::builtin::{RoleContributor, TaskContributor, RulesContributor};
    use crate::contributor::ContextContributor;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_build_with_builtin_contributors() {
        let mut builder = ContextBuilder::builder()
            .contributor(RoleContributor::new("You are a helpful assistant"))
            .contributor(TaskContributor::new("Answer questions about code"))
            .contributor(RulesContributor::new(vec!["Use snake_case"]))
            .token_limit(100_000)
            .build();

        let output = builder.build().await;
        assert!(!output.messages.is_empty());
    }

    #[tokio::test]
    async fn test_build_with_custom_contributor() {
        struct MemoryContributor;

        #[async_trait::async_trait]
        impl ContextContributor for MemoryContributor {
            fn name(&self) -> &str { "memory" }

            async fn contribute(&self) -> Vec<ContextBlock> {
                vec![ContextBlock {
                    messages: vec![Message::system("User prefers Rust".to_string())],
                    anchor: AttentionAnchor::Middle(0),
                }]
            }
        }

        let mut builder = ContextBuilder::builder()
            .contributor(RoleContributor::new("Assistant"))
            .contributor(MemoryContributor)
            .token_limit(100_000)
            .build();

        let output = builder.build().await;
        assert!(output.messages.len() >= 2);
    }

    #[tokio::test]
    async fn test_build_empty() {
        let mut builder = ContextBuilder::builder()
            .token_limit(100_000)
            .build();

        let output = builder.build().await;
        assert!(output.messages.is_empty());
    }

    #[tokio::test]
    async fn test_build_respects_anchor_ordering() {
        struct HeadContributor;
        struct TailContributor;

        #[async_trait::async_trait]
        impl ContextContributor for HeadContributor {
            fn name(&self) -> &str { "head" }
            async fn contribute(&self) -> Vec<ContextBlock> {
                vec![ContextBlock {
                    messages: vec![Message::system("HEAD".to_string())],
                    anchor: AttentionAnchor::Head(0),
                }]
            }
        }

        #[async_trait::async_trait]
        impl ContextContributor for TailContributor {
            fn name(&self) -> &str { "tail" }
            async fn contribute(&self) -> Vec<ContextBlock> {
                vec![ContextBlock {
                    messages: vec![Message::user("TAIL".to_string())],
                    anchor: AttentionAnchor::Tail(0),
                }]
            }
        }

        let mut builder = ContextBuilder::builder()
            .contributor(TailContributor)
            .contributor(HeadContributor)
            .token_limit(100_000)
            .build();

        let output = builder.build().await;
        let head_idx = output.messages.iter().position(|m| {
            m.content.as_ref().map_or(false, |c| c.contains("HEAD"))
        }).unwrap();
        let tail_idx = output.messages.iter().position(|m| {
            m.content.as_ref().map_or(false, |c| c.contains("TAIL"))
        }).unwrap();
        assert!(head_idx < tail_idx);
    }

    #[tokio::test]
    async fn test_build_token_budget_drops_excess() {
        struct BigContributor {
            content: String,
        }

        #[async_trait::async_trait]
        impl ContextContributor for BigContributor {
            fn name(&self) -> &str { "big" }

            async fn contribute(&self) -> Vec<ContextBlock> {
                vec![ContextBlock {
                    messages: vec![Message::system(self.content.clone())],
                    anchor: AttentionAnchor::Middle(0),
                }]
            }

            fn compress(&mut self) {
                let half = self.content.len() / 2;
                self.content = self.content[..half].to_string();
            }
        }

        let mut builder = ContextBuilder::builder()
            .contributor(BigContributor {
                content: "A".repeat(10000),
            })
            .token_limit(1000)
            .build();

        let output = builder.build().await;
        let total_tokens = TokenBudget::estimate_tokens(&output.messages);
        assert!(total_tokens < 10000 / 4);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vol-llm-context --lib`
Expected: FAIL — `ContextBuilder` not defined.

- [ ] **Step 3: Implement builder.rs**

```rust
use vol_llm_core::Message;

use crate::block::{AttentionAnchor, ContextBlock, TokenBudget};
use crate::contributor::ContextContributor;

/// Output from ContextBuilder — ready-to-send LLM messages.
pub struct ContextOutput {
    pub messages: Vec<Message>,
}

/// Orchestrator that collects contributors, applies budget constraints,
/// and produces a sorted, formatted message list.
pub struct ContextBuilder {
    contributors: Vec<Box<dyn ContextContributor>>,
    token_limit: usize,
    built: bool,
}

impl ContextBuilder {
    pub fn new(token_limit: usize) -> Self {
        Self {
            contributors: Vec::new(),
            token_limit,
            built: false,
        }
    }

    pub fn add_contributor(&mut self, contributor: impl ContextContributor + 'static) {
        self.contributors.push(Box::new(contributor));
    }

    pub async fn build(&mut self) -> ContextOutput {
        if self.built {
            tracing::warn!("ContextBuilder::build called multiple times");
        }
        self.built = true;

        // Collect all blocks
        let mut all_blocks: Vec<ContextBlock> = Vec::new();
        for contributor in &self.contributors {
            let blocks = contributor.contribute().await;
            all_blocks.extend(blocks);
        }

        // Separate by zone
        let mut head_blocks: Vec<ContextBlock> = Vec::new();
        let mut middle_blocks: Vec<ContextBlock> = Vec::new();
        let mut tail_blocks: Vec<ContextBlock> = Vec::new();

        for block in all_blocks {
            match &block.anchor {
                AttentionAnchor::Head(_) => head_blocks.push(block),
                AttentionAnchor::Middle(_) => middle_blocks.push(block),
                AttentionAnchor::Tail(_) => tail_blocks.push(block),
            }
        }

        // Estimate head + tail sizes
        let head_messages: Vec<_> = head_blocks.iter().flat_map(|b| &b.messages).collect();
        let tail_messages: Vec<_> = tail_blocks.iter().flat_map(|b| &b.messages).collect();
        let head_size = TokenBudget::estimate_tokens(&head_messages);
        let tail_size = TokenBudget::estimate_tokens(&tail_messages);

        // Calculate middle budget
        let middle_budget = self.token_limit.saturating_sub(head_size).saturating_sub(tail_size);

        // Compress if over budget
        if !middle_blocks.is_empty() {
            let middle_messages: Vec<_> = middle_blocks.iter().flat_map(|b| &b.messages).collect();
            let middle_size = TokenBudget::estimate_tokens(&middle_messages);

            if middle_size > middle_budget {
                // Drop blocks from highest order (lowest priority) until within budget
                middle_blocks.sort_by(|a, b| b.anchor.cmp(&a.anchor));
                while !middle_blocks.is_empty() {
                    let remaining: Vec<_> = middle_blocks.iter().flat_map(|b| &b.messages).collect();
                    let remaining_size = TokenBudget::estimate_tokens(&remaining);
                    if remaining_size <= middle_budget {
                        break;
                    }
                    middle_blocks.pop();
                }
            }
        }

        // Sort within each zone
        head_blocks.sort_by(|a, b| a.anchor.cmp(&b.anchor));
        middle_blocks.sort_by(|a, b| a.anchor.cmp(&b.anchor));
        tail_blocks.sort_by(|a, b| a.anchor.cmp(&b.anchor));

        // Concatenate
        let mut messages = Vec::new();
        for block in head_blocks {
            messages.extend(block.messages);
        }
        for block in middle_blocks {
            messages.extend(block.messages);
        }
        for block in tail_blocks {
            messages.extend(block.messages);
        }

        ContextOutput { messages }
    }
}

/// Builder pattern for ContextBuilder.
pub struct ContextBuilderBuilder {
    token_limit: usize,
    contributors: Vec<Box<dyn ContextContributor>>,
}

impl ContextBuilderBuilder {
    pub fn new() -> Self {
        Self {
            token_limit: 200_000,
            contributors: Vec::new(),
        }
    }

    pub fn contributor(mut self, contributor: impl ContextContributor + 'static) -> Self {
        self.contributors.push(Box::new(contributor));
        self
    }

    pub fn token_limit(mut self, limit: usize) -> Self {
        self.token_limit = limit;
        self
    }

    pub fn build(self) -> ContextBuilder {
        let mut builder = ContextBuilder::new(self.token_limit);
        for contributor in self.contributors {
            builder.contributors.push(contributor);
        }
        builder
    }
}

impl Default for ContextBuilderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{ContextBlock, AttentionAnchor, TokenBudget};
    use crate::builtin::{RoleContributor, TaskContributor, RulesContributor};
    use crate::contributor::ContextContributor;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_build_with_builtin_contributors() {
        let mut builder = ContextBuilder::builder()
            .contributor(RoleContributor::new("You are a helpful assistant"))
            .contributor(TaskContributor::new("Answer questions about code"))
            .contributor(RulesContributor::new(vec!["Use snake_case"]))
            .token_limit(100_000)
            .build();

        let output = builder.build().await;
        assert!(!output.messages.is_empty());
    }

    #[tokio::test]
    async fn test_build_with_custom_contributor() {
        struct MemoryContributor;

        #[async_trait::async_trait]
        impl ContextContributor for MemoryContributor {
            fn name(&self) -> &str { "memory" }

            async fn contribute(&self) -> Vec<ContextBlock> {
                vec![ContextBlock {
                    messages: vec![Message::system("User prefers Rust".to_string())],
                    anchor: AttentionAnchor::Middle(0),
                }]
            }
        }

        let mut builder = ContextBuilder::builder()
            .contributor(RoleContributor::new("Assistant"))
            .contributor(MemoryContributor)
            .token_limit(100_000)
            .build();

        let output = builder.build().await;
        assert!(output.messages.len() >= 2);
    }

    #[tokio::test]
    async fn test_build_empty() {
        let mut builder = ContextBuilder::builder()
            .token_limit(100_000)
            .build();

        let output = builder.build().await;
        assert!(output.messages.is_empty());
    }

    #[tokio::test]
    async fn test_build_respects_anchor_ordering() {
        struct HeadContributor;
        struct TailContributor;

        #[async_trait::async_trait]
        impl ContextContributor for HeadContributor {
            fn name(&self) -> &str { "head" }
            async fn contribute(&self) -> Vec<ContextBlock> {
                vec![ContextBlock {
                    messages: vec![Message::system("HEAD".to_string())],
                    anchor: AttentionAnchor::Head(0),
                }]
            }
        }

        #[async_trait::async_trait]
        impl ContextContributor for TailContributor {
            fn name(&self) -> &str { "tail" }
            async fn contribute(&self) -> Vec<ContextBlock> {
                vec![ContextBlock {
                    messages: vec![Message::user("TAIL".to_string())],
                    anchor: AttentionAnchor::Tail(0),
                }]
            }
        }

        let mut builder = ContextBuilder::builder()
            .contributor(TailContributor)
            .contributor(HeadContributor)
            .token_limit(100_000)
            .build();

        let output = builder.build().await;
        let head_idx = output.messages.iter().position(|m| {
            m.content.as_ref().map_or(false, |c| c.contains("HEAD"))
        }).unwrap();
        let tail_idx = output.messages.iter().position(|m| {
            m.content.as_ref().map_or(false, |c| c.contains("TAIL"))
        }).unwrap();
        assert!(head_idx < tail_idx);
    }

    #[tokio::test]
    async fn test_build_token_budget_drops_excess() {
        struct BigContributor {
            content: String,
        }

        #[async_trait::async_trait]
        impl ContextContributor for BigContributor {
            fn name(&self) -> &str { "big" }

            async fn contribute(&self) -> Vec<ContextBlock> {
                vec![ContextBlock {
                    messages: vec![Message::system(self.content.clone())],
                    anchor: AttentionAnchor::Middle(0),
                }]
            }

            fn compress(&mut self) {
                let half = self.content.len() / 2;
                self.content = self.content[..half].to_string();
            }
        }

        let mut builder = ContextBuilder::builder()
            .contributor(BigContributor {
                content: "A".repeat(10000),
            })
            .token_limit(1000)
            .build();

        let output = builder.build().await;
        let total_tokens = TokenBudget::estimate_tokens(&output.messages);
        assert!(total_tokens < 10000 / 4);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vol-llm-context --lib`
Expected: 11 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-context/src/builder.rs
git commit -m "feat: add ContextBuilder with budget-driven compression"
```

---

### Task 5: Implement Built-in Contributors

**Files:**
- Modify: `crates/vol-llm-context/src/builtin/mod.rs`
- Modify: `crates/vol-llm-context/src/builtin/role.rs`
- Modify: `crates/vol-llm-context/src/builtin/task.rs`
- Modify: `crates/vol-llm-context/src/builtin/rules.rs`

- [ ] **Step 1: Implement builtin/role.rs**

```rust
use async_trait::async_trait;
use vol_llm_core::Message;

use crate::block::{AttentionAnchor, ContextBlock};
use crate::contributor::ContextContributor;

/// Contributes the AI's role/identity to the Head zone.
pub struct RoleContributor {
    role: String,
}

impl RoleContributor {
    pub fn new(role: impl Into<String>) -> Self {
        Self { role: role.into() }
    }
}

#[async_trait]
impl ContextContributor for RoleContributor {
    fn name(&self) -> &str { "role" }

    async fn contribute(&self) -> Vec<ContextBlock> {
        vec![ContextBlock {
            messages: vec![Message::system(format!("## Role\n\n{}", self.role))],
            anchor: AttentionAnchor::Head(0),
        }]
    }
}
```

- [ ] **Step 2: Implement builtin/task.rs**

```rust
use async_trait::async_trait;
use vol_llm_core::Message;

use crate::block::{AttentionAnchor, ContextBlock};
use crate::contributor::ContextContributor;

/// Contributes the task description to the Tail zone.
pub struct TaskContributor {
    task: String,
}

impl TaskContributor {
    pub fn new(task: impl Into<String>) -> Self {
        Self { task: task.into() }
    }
}

#[async_trait]
impl ContextContributor for TaskContributor {
    fn name(&self) -> &str { "task" }

    async fn contribute(&self) -> Vec<ContextBlock> {
        vec![ContextBlock {
            messages: vec![Message::system(format!("## Task\n\n{}", self.task))],
            anchor: AttentionAnchor::Tail(0),
        }]
    }
}
```

- [ ] **Step 3: Implement builtin/rules.rs**

```rust
use async_trait::async_trait;
use vol_llm_core::Message;

use crate::block::{AttentionAnchor, ContextBlock};
use crate::contributor::ContextContributor;

/// Contributes general rules/norms to the Head zone.
pub struct RulesContributor {
    rules: Vec<String>,
}

impl RulesContributor {
    pub fn new(rules: Vec<String>) -> Self {
        Self { rules }
    }
}

#[async_trait]
impl ContextContributor for RulesContributor {
    fn name(&self) -> &str { "rules" }

    async fn contribute(&self) -> Vec<ContextBlock> {
        let content = if self.rules.is_empty() {
            "## Rules\n\nNo specific rules.".to_string()
        } else {
            let mut s = String::from("## Rules\n\n");
            for rule in &self.rules {
                s.push_str(&format!("- {}\n", rule));
            }
            s
        };

        vec![ContextBlock {
            messages: vec![Message::system(content)],
            anchor: AttentionAnchor::Head(10),
        }]
    }
}
```

- [ ] **Step 4: Implement builtin/mod.rs**

```rust
//! Built-in context contributors.

mod role;
mod task;
mod rules;

pub use role::RoleContributor;
pub use task::TaskContributor;
pub use rules::RulesContributor;
```

- [ ] **Step 5: Run tests to verify**

Run: `cargo test -p vol-llm-context --lib`
Expected: All 11+ tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-context/src/builtin/
git commit -m "feat: add built-in contributors (role, task, rules)"
```

---

### Task 6: Integration Tests

**Files:**
- Create: `crates/vol-llm-context/tests/context_test.rs`

- [ ] **Step 1: Write integration tests**

```rust
use vol_llm_context::builder::ContextBuilder;
use vol_llm_context::builtin::{RoleContributor, TaskContributor, RulesContributor};
use vol_llm_context::contributor::ContextContributor;
use vol_llm_context::block::{ContextBlock, AttentionAnchor, TokenBudget};
use vol_llm_core::Message;

#[tokio::test]
async fn test_full_build_with_builtins() {
    let mut builder = ContextBuilder::builder()
        .contributor(RoleContributor::new("You are an expert coding assistant"))
        .contributor(TaskContributor::new("Help users understand and modify their codebase"))
        .contributor(RulesContributor::new(vec![
            "Use snake_case for functions".to_string(),
            "Write tests before implementing".to_string(),
            "Keep functions under 50 lines".to_string(),
        ]))
        .token_limit(100_000)
        .build();

    let output = builder.build().await;

    assert!(output.messages.len() >= 3);

    for msg in &output.messages {
        assert!(msg.content.is_some());
    }
}

#[tokio::test]
async fn test_custom_middle_contributor() {
    struct MemoryContributor;

    #[async_trait::async_trait]
    impl ContextContributor for MemoryContributor {
        fn name(&self) -> &str { "memory" }

        async fn contribute(&self) -> Vec<ContextBlock> {
            vec![ContextBlock {
                messages: vec![Message::system("## Memory\n\nUser prefers Rust".to_string())],
                anchor: AttentionAnchor::Middle(0),
            }]
        }
    }

    let mut builder = ContextBuilder::builder()
        .contributor(RoleContributor::new("Assistant"))
        .contributor(MemoryContributor)
        .token_limit(100_000)
        .build();

    let output = builder.build().await;
    assert_eq!(output.messages.len(), 2);

    let role_idx = output.messages.iter().position(|m| {
        m.content.as_ref().map_or(false, |c| c.contains("Assistant"))
    }).unwrap();
    let memory_idx = output.messages.iter().position(|m| {
        m.content.as_ref().map_or(false, |c| c.contains("Rust"))
    }).unwrap();
    assert!(role_idx < memory_idx);
}

#[tokio::test]
async fn test_token_budget_truncates_middle() {
    struct BigContributor {
        content: String,
    }

    #[async_trait::async_trait]
    impl ContextContributor for BigContributor {
        fn name(&self) -> &str { "big" }

        async fn contribute(&self) -> Vec<ContextBlock> {
            vec![ContextBlock {
                messages: vec![Message::system(self.content.clone())],
                anchor: AttentionAnchor::Middle(0),
            }]
        }
    }

    let big_content = "x".repeat(10_000);

    let mut builder = ContextBuilder::builder()
        .contributor(RoleContributor::new("Assistant"))
        .contributor(BigContributor { content: big_content })
        .token_limit(500)
        .build();

    let output = builder.build().await;

    let total_tokens = TokenBudget::estimate_tokens(&output.messages);
    assert!(total_tokens <= 500, "Total tokens {} should be within budget 500", total_tokens);
}

#[tokio::test]
async fn test_build_multiple_times_returns_empty() {
    let mut builder = ContextBuilder::builder()
        .contributor(RoleContributor::new("Assistant"))
        .token_limit(100_000)
        .build();

    let output1 = builder.build().await;
    let output2 = builder.build().await;

    assert!(!output1.messages.is_empty());
    assert!(output2.messages.is_empty());
}

#[tokio::test]
async fn test_contributor_with_compression() {
    struct CompressibleContributor {
        k: std::sync::Mutex<usize>,
        base_content: &'static str,
    }

    #[async_trait::async_trait]
    impl ContextContributor for CompressibleContributor {
        fn name(&self) -> &str { "compressible" }

        async fn contribute(&self) -> Vec<ContextBlock> {
            let k = *self.k.lock().unwrap();
            let content = self.base_content.repeat(k);
            vec![ContextBlock {
                messages: vec![Message::system(content)],
                anchor: AttentionAnchor::Middle(0),
            }]
        }

        fn compress(&mut self) {
            let mut k = self.k.lock().unwrap();
            *k = (*k).max(1) / 2;
        }
    }

    let contributor = CompressibleContributor {
        k: std::sync::Mutex::new(100),
        base_content: "mem ",
    };

    let mut builder = ContextBuilder::builder()
        .contributor(contributor)
        .token_limit(50)
        .build();

    let output = builder.build().await;

    let total_tokens = TokenBudget::estimate_tokens(&output.messages);
    assert!(total_tokens <= 50 || output.messages.is_empty());
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p vol-llm-context`
Expected: All 5 integration tests + 11 unit tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-context/tests/context_test.rs
git commit -m "test: add integration tests for vol-llm-context"
```

---

### Task 7: Full Workspace Verification

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run all existing tests**

```bash
cargo test --workspace --lib
```

Expected: All existing tests pass. No new compilation warnings.

---

## Summary

| Crate | Files Changed | Purpose |
|-------|---------------|---------|
| `vol-llm-context` | **new** (9 files) | Context management crate |
| `Cargo.toml` (root) | Modify | Add workspace member + dependency |

Total: ~500 lines of implementation code, ~400 lines of tests.
