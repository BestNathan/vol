# Context Management System Design

> **Goal:** Create `vol-llm-context` crate providing structured context management with attention-zone-aware sorting, budget-driven compression, and trait-based extensibility.

**Architecture:** `ContextBlock` (messages + anchor) → `ContextContributor` trait (extension) → `ContextBuilder` (orchestrator). Contributors produce `Vec<Message>` directly — no separate formatter layer. Budget-driven compression when middle-zone content exceeds token limits.

**Key principle:** Contributors decide how to format their content into `Message` objects. The builder only handles sorting, budgeting, compression, and concatenation.

---

## 1. Core Types

### 1.1 `AttentionAnchor` — Single enum with position value

```rust
pub enum AttentionAnchor {
    Head(u32),   // Top high-attention zone (built-in only)
    Middle(u32), // Low-attention zone (external contributors)
    Tail(u32),   // Bottom high-attention zone (built-in only)
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
    // Sort: zone priority ascending, then position ascending within zone
}
```

### 1.2 `ContextBlock` — Messages + anchor

```rust
use vol_llm_core::Message;

pub struct ContextBlock {
    pub messages: Vec<Message>,   // contributor-formatted messages
    pub anchor: AttentionAnchor,
}
```

Contributors format content into messages themselves. Text-based contributors create `Message::system` with markdown formatting. Session contributors return raw conversation messages directly.

### 1.3 `TokenBudget` — Budget tracking

```rust
pub struct TokenBudget {
    pub total: usize,           // total token limit
    pub used: usize,            // tokens consumed so far
}

impl TokenBudget {
    pub fn remaining(&self) -> usize { self.total - self.used }
    pub fn is_exceeded(&self) -> bool { self.used > self.total }
}
```

Token counting uses a simple estimator (chars / 4) or a configurable tokenizer function.

---

## 2. Extension Trait

### 2.1 `ContextContributor`

```rust
#[async_trait]
pub trait ContextContributor: Send + Sync {
    fn name(&self) -> &str;

    /// Contribution — returns blocks with their preferred anchors.
    /// After compress() is called, this should return reduced content.
    async fn contribute(&self) -> Vec<ContextBlock>;

    /// Compress internal state to reduce output size.
    /// No return value — caller should re-call contribute() after this.
    /// Default: no-op (not compressible).
    fn compress(&mut self) {}
}
```

**Constraints:**
- **Built-in contributors** (role, identity, task, rules, skills): may return `Head` and `Tail` anchors. `compress()` is no-op.
- **External contributors** (memory, project facts, session): only return `Middle` anchors. Should implement `compress()` to reduce future output.

**Compression semantics:**
- When total middle-zone size exceeds budget threshold, `ContextBuilder` calls `compress()` on all external contributors (mutating their internal state).
- After compression, `contribute()` is called again to get the reduced blocks.
- If still over budget, lowest-priority contributors (by order value) are dropped first.

### 2.2 Built-in contributor types

| Contributor | Anchor | Compressible | Purpose |
|-------------|--------|--------------|---------|
| `RoleContributor` | Head(0) | No | AI role/identity |
| `TaskContributor` | Tail(0) | No | Task description |
| `RulesContributor` | Head(10) | No | General rules/norms |
| `SkillsContributor` | Head(20) | No | Skill instructions from vol-llm-skill |

### 2.3 External contributor example (memory)

```rust
impl ContextContributor for MemoryContributor {
    fn name(&self) -> &str { "memory" }

    async fn contribute(&self) -> Vec<ContextBlock> {
        let memories = self.manager.search(self.query, self.k).await;
        let text = format!("## Memory\n\n{}\n", format_memories(&memories));
        vec![ContextBlock {
            messages: vec![Message::system(text)],
            anchor: AttentionAnchor::Middle(0),
        }]
    }

    fn compress(&mut self) {
        // Reduce query count on next contribute() call
        self.k = self.k.max(1) / 2;
    }
}
```

### 2.4 External contributor example (session)

```rust
impl ContextContributor for SessionContributor {
    fn name(&self) -> &str { "session" }

    async fn contribute(&self) -> Vec<ContextBlock> {
        // Return conversation history as-is — already structured messages
        let msgs = self.session.history().await;
        vec![ContextBlock {
            messages: msgs,
            anchor: AttentionAnchor::Tail(10),
        }]
    }

    fn compress(&mut self) {
        // Keep only last N messages
        self.keep_last = self.keep_last.max(2) / 2;
    }
}
```

---

## 3. ContextBuilder — Orchestrator

### 3.1 `ContextOutput`

```rust
pub struct ContextOutput {
    pub messages: Vec<Message>,
}
```

### 3.2 Structure

```rust
pub struct ContextBuilder {
    contributors: Vec<Box<dyn ContextContributor>>,
    token_limit: usize,
}

impl ContextBuilder {
    pub fn builder() -> ContextBuilderBuilder { ... }

    pub fn add_contributor(&mut self, contributor: impl ContextContributor + 'static);

    /// Build context messages. &mut because compress() mutates contributors.
    pub async fn build(&mut self) -> ContextOutput;
}
```

### 3.3 Builder API

```rust
let mut builder = ContextBuilder::builder()
    .contributor(RoleContributor::new("You are a coding assistant"))
    .contributor(TaskContributor::new("Help users modify code"))
    .contributor(RulesContributor::new(vec!["Use snake_case"]))
    .contributor(MemoryContributor::new(memory_manager))
    .contributor(SkillsContributor::new(skill_loader))
    .contributor(SessionContributor::new(session))
    .token_limit(200_000)
    .build();

let output = builder.build().await;
// output.messages is Vec<Message> ready to send to the LLM
```

### 3.4 Build flow

```
1. Collect all blocks from contributors (async, via contribute())
2. Separate into Head / Middle / Tail groups
3. Estimate token sizes: head_tokens + tail_tokens
4. Calculate: middle_budget = total - head_tokens - tail_tokens
5. Estimate middle token size
6. If middle exceeds budget:
   a. Call compress() on all external contributors (mutates their state)
   b. Re-call contribute() on all external contributors to get compressed blocks
   c. Re-estimate middle size
7. If still over budget after compression:
   a. Sort middle blocks by order descending
   b. Drop lowest-priority blocks until within budget
8. Final sort: Head → Middle → Tail (within each group, by position value)
9. Concatenate all block messages in order → Vec<Message>
10. Return ContextOutput { messages }
```

### 3.5 Message grouping strategy

Each zone maps to message roles:

| Zone | Typical message role | Purpose |
|------|---------------------|---------|
| Head | `Message::system` | Core instructions (role, rules, skills) |
| Middle | `Message::system` or mixed | Context data (memory, project facts) |
| Tail | `Message::user` or mixed | Task description, session history — placed last for high attention |

Contributors decide the message role when constructing their `ContextBlock`. The builder preserves the message structure within each block.

---

## 4. Module Structure

| File | Purpose |
|------|---------|
| `crates/vol-llm-context/Cargo.toml` | Crate manifest |
| `crates/vol-llm-context/src/lib.rs` | Re-exports |
| `crates/vol-llm-context/src/block.rs` | `ContextBlock`, `AttentionAnchor`, `TokenBudget` |
| `crates/vol-llm-context/src/contributor.rs` | `ContextContributor` trait |
| `crates/vol-llm-context/src/builder.rs` | `ContextBuilder` with builder pattern |
| `crates/vol-llm-context/src/builtin/` | Built-in contributor implementations (role, task, rules, skills) |
| `crates/vol-llm-context/tests/context_test.rs` | Integration tests |

---

## 5. Compression Flow Detail

```
total_limit = 200_000
  ↓
Step 1: collect head + tail blocks (fixed, never compressed)
  head_size = estimate(head_blocks)
  tail_size = estimate(tail_blocks)
  ↓
Step 2: calculate middle budget
  middle_budget = total_limit - head_size - tail_size
  ↓
Step 3: collect middle blocks from external contributors
  middle_size = estimate(middle_blocks)
  ↓
Step 4: check budget
  if middle_size > middle_budget:
    → call compress() on all external contributors (mutates state)
    → re-call contribute() on all external contributors
    → re-collect middle blocks (compressed)
    → re-estimate
  ↓
Step 5: still over? drop by priority
  sort middle blocks by order descending (highest order = lowest priority)
  drop blocks until within budget
  ↓
Step 6: sort all blocks
  Head (by position asc) → Middle (by position asc) → Tail (by position asc)
  ↓
Step 7: concatenate
  extend all block.messages into a single Vec<Message>
```

---

## 6. Relationship with Existing Code

This crate is **independent** of the existing `PromptContext` system in `vol-llm-agent`. It produces a `Vec<Message>` ready for LLM consumption. Integration with `PromptContext` (replacement or wrapping) is a separate future task.

Usage example:

```rust
let mut builder = ContextBuilder::builder()
    .contributor(RoleContributor::new("You are an expert coding assistant"))
    .contributor(MemoryContributor::new(memory_manager))
    .contributor(SessionContributor::new(session))
    .token_limit(100_000)
    .build();

let output = builder.build().await;
let messages = output.messages;
// messages can be sent directly to the LLM
```
