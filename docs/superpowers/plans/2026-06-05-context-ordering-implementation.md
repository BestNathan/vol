# Context Ordering Standard — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish fixed context ordering (Agent Prompt → Skills → Custom Files → Session) in ContextBuilder, parameterize SkillInjector/SessionContributor anchors, and document the standard in CLAUDE.md.

**Architecture:** Add `context_files: Vec<String>` to AgentDef for custom Middle-zone files; parameterize `AttentionAnchor` in SkillInjector and SessionContributor constructors; reorder AgentConfigBuilder::build() to enforce Head(0)→AgentPrompt, Head(1)→Skills, Middle(0..n)→CustomFiles, Tail(0)→Session.

**Tech Stack:** Rust, vol-llm-core, vol-llm-context, vol-llm-skill, vol-session, vol-llm-agent

---

### Task 1: AgentDef — add context_files field

**Files:**
- Modify: `crates/vol-llm-core/src/agent_def.rs`
- Modify: `crates/vol-llm-agent/src/agent_loader.rs`

- [ ] **Step 1: Add context_files field and builder method to AgentDef**

```rust
// In AgentDef struct, after `working_dir` field:
/// Custom context files injected into the Middle zone.
/// Each path is relative to the agent's working directory.
/// Files are loaded in array order: first file → Middle(0), second → Middle(1), etc.
pub context_files: Vec<String>,
```

```rust
// In AgentDef impl block, after `with_working_dir()`:
/// Set custom context files to inject into the Middle zone.
pub fn with_context_files(mut self, files: Vec<String>) -> Self {
    self.context_files = files;
    self
}
```

- [ ] **Step 2: Update AgentDef::new() to initialize context_files**

In `AgentDef::new()` struct literal, add:
```rust
context_files: vec![],
```

- [ ] **Step 3: Update AgentLoader struct literal**

In `crates/vol-llm-agent/src/agent_loader.rs` line 88, add to the AgentDef struct literal:
```rust
context_files: vec![],
```

- [ ] **Step 4: Run tests to verify**

```bash
cargo test -p vol-llm-core -p vol-llm-agent -- agent_def
```

Expected: all existing AgentDef tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-core/src/agent_def.rs crates/vol-llm-agent/src/agent_loader.rs
git commit -m "feat(agent_def): add context_files field for Middle-zone custom files

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: SkillInjector — parameterize anchor

**Files:**
- Modify: `crates/vol-llm-skill/src/injector.rs`

- [ ] **Step 1: Add anchor field and update constructor**

```rust
pub struct SkillInjector {
    loader: Arc<SkillLoader>,
    anchor: AttentionAnchor,  // ← new field
}

impl SkillInjector {
    pub fn new(loader: Arc<SkillLoader>, anchor: AttentionAnchor) -> Self {
        Self { loader, anchor }
    }

    pub async fn from_workdir(working_dir: &std::path::Path, anchor: AttentionAnchor) -> Self {
        let loader = Arc::new(crate::loader::SkillLoader::new(Some(working_dir.to_path_buf())));
        Self::new(loader, anchor)
    }
```

- [ ] **Step 2: Update contribute() to use self.anchor + empty placeholder**

Replace the hardcoded `AttentionAnchor::Head(0)` with `self.anchor.clone()`.
Also: when no skills are loaded, return an empty block as placeholder (not empty vec) to maintain the fixed Head slot:

```rust
async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
    let metadata_text = self.format_metadata().await;
    if metadata_text.is_empty() {
        // Return empty placeholder block to maintain fixed Head slot
        return Ok(vec![ContextBlock::new(vec![], self.anchor.clone())]);
    }
    let msg = Message::user(metadata_text);
    Ok(vec![ContextBlock::new(vec![msg], self.anchor.clone())])
}
```

This means tests that assert `blocks.is_empty()` will need updating in Step 5 — change to assert `blocks[0].messages.is_empty()` instead.

- [ ] **Step 3: Update clone_box() to include anchor**

```rust
fn clone_box(&self) -> Box<dyn ContextContributor> {
    Box::new(SkillInjector {
        loader: self.loader.clone(),
        anchor: self.anchor.clone(),
    })
}
```

- [ ] **Step 4: Run SkillInjector tests**

```bash
cargo test -p vol-llm-skill -- injector
```

Expected: FAIL — tests pass `SkillInjector::new(loader)` without anchor. Fix in next step.

- [ ] **Step 5: Update all test call sites in injector.rs**

Add `AttentionAnchor::Head(0)` to every `SkillInjector::new(...)` call (lines 92, 106, 118, 132, 142, 150, 164):

```rust
// Before:
let injector = SkillInjector::new(Arc::new(loader));
// After:
let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0));
```

Also update `test_skill_injector_contribute_empty` (line 121): change `assert!(blocks.is_empty())` to:
```rust
assert_eq!(blocks.len(), 1);
assert!(blocks[0].messages.is_empty());
```
(Empty skills now return a placeholder block instead of empty vec.)

- [ ] **Step 6: Run tests to verify**

```bash
cargo test -p vol-llm-skill -- injector
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-skill/src/injector.rs
git commit -m "refactor(skill): parameterize AttentionAnchor in SkillInjector

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: SessionContributor — parameterize anchor

**Files:**
- Modify: `crates/vol-session/src/session_contributor.rs`

- [ ] **Step 1: Add anchor field and update constructor**

```rust
pub struct SessionContributor {
    session: Arc<tokio::sync::Mutex<Session>>,
    max_history: usize,
    compressor: Arc<dyn MessageCompressor>,
    anchor: AttentionAnchor,  // ← new field
}

impl SessionContributor {
    pub fn new(
        session: Arc<tokio::sync::Mutex<Session>>,
        max_history: usize,
        anchor: AttentionAnchor,
    ) -> Self {
        Self {
            session,
            max_history,
            compressor: Arc::new(PositionSampleCompressor::default()),
            anchor,
        }
    }
```

- [ ] **Step 2: Update contribute() to use self.anchor**

Replace `AttentionAnchor::Middle(0)`:
```rust
let block = ContextBlock::new(messages, self.anchor.clone());
```

- [ ] **Step 3: Update clone_box() to include anchor**

```rust
fn clone_box(&self) -> Box<dyn ContextContributor> {
    Box::new(SessionContributor {
        session: self.session.clone(),
        max_history: self.max_history,
        compressor: self.compressor.clone(),
        anchor: self.anchor.clone(),
    })
}
```

- [ ] **Step 4: Run tests to verify they fail**

```bash
cargo test -p vol-session -- session_contributor
```

Expected: FAIL — tests use old 2-arg constructor signature.

- [ ] **Step 5: Update all test call sites in session_contributor.rs**

Add `AttentionAnchor::Middle(0)` to every `SessionContributor::new(...)` call (lines 152, 168, 178, 194, 216):

```rust
// Before:
let contributor = SessionContributor::new(Arc::new(tokio::sync::Mutex::new(session)), 10);
// After:
let contributor = SessionContributor::new(
    Arc::new(tokio::sync::Mutex::new(session)),
    10,
    AttentionAnchor::Middle(0),
);
```

- [ ] **Step 6: Run tests to verify**

```bash
cargo test -p vol-session -- session_contributor
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-session/src/session_contributor.rs
git commit -m "refactor(session): parameterize AttentionAnchor in SessionContributor

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Update external call sites

**Files:**
- Modify: `crates/vol-llm-skill/tests/skill_test.rs`
- Modify: `crates/vol-llm-agents/tests/skill_session_integration.rs`
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`
- Modify: `crates/vol-llm-agent/tests/compression_flow_test.rs`

- [ ] **Step 1: Update skill_test.rs**

In `crates/vol-llm-skill/tests/skill_test.rs` line 96, add anchor parameter:
```rust
// Find: SkillInjector::new(Arc::new(loader))
// Replace with:
SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0))
```

Need to add import:
```rust
use vol_llm_context::AttentionAnchor;
```

- [ ] **Step 2: Update skill_session_integration.rs**

All `SkillInjector::from_workdir(&workdir).await` → `SkillInjector::from_workdir(&workdir, AttentionAnchor::Head(0)).await`
All `SessionContributor::new(session, N)` → `SessionContributor::new(session, N, AttentionAnchor::Middle(0))`

Add import: `use vol_llm_context::AttentionAnchor;`

Affected lines: 68, 76, 103, 111, 139, 147, 180

- [ ] **Step 3: Update coding/agent.rs line 97**

```rust
// Before:
let injector = SkillInjector::new(loader);
// After:
let injector = SkillInjector::new(loader, AttentionAnchor::Head(0));
```

- [ ] **Step 4: Update run_context.rs**

All `SessionContributor::new(...)` calls (lines 598, 641, 693) — add `AttentionAnchor::Middle(0)`:

```rust
// Before:
.add_contributor(Box::new(SessionContributor::new(session.clone(), 10)))
// After:
.add_contributor(Box::new(SessionContributor::new(session.clone(), 10, AttentionAnchor::Middle(0))))
```

- [ ] **Step 5: Update compression_flow_test.rs**

Lines 28, 55 — add `AttentionAnchor::Middle(0)` to `SessionContributor::new()` calls.

- [ ] **Step 6: Check for any remaining call sites**

```bash
cargo check --workspace 2>&1 | grep -E "this function takes 3 arguments|this function takes 2 arguments|missing field|expected.*found"
```

Fix any remaining compilation errors.

- [ ] **Step 7: Run all affected tests**

```bash
cargo test -p vol-llm-skill -p vol-session -p vol-llm-agents -p vol-llm-agent
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/
git commit -m "chore: update all call sites for parameterized SkillInjector/SessionContributor

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: AgentConfigBuilder — enforce context ordering standard

**Files:**
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs`
- Modify: `crates/vol-llm-agent/src/react/agent.rs` (SessionContributor call at line 231)

- [ ] **Step 1: Rewrite context assembly in AgentConfigBuilder::build()**

Replace the context_builder assembly block (lines 199-246) with:

```rust
// Build context builder with standardized ordering:
//
//   Head(0)  — Agent Prompt   (always present, empty placeholder if unset)
//   Head(1)  — Skills         (always present, empty block if no skills)
//   Middle(0..n) — Custom Files (from AgentDef.context_files, array order)
//   Tail(0)  — Session        (conversation history)

let mut b = ContextBuilderBuilder::new(total)
    .head_size(head_size)
    .tail_size(tail_size);

// 1. Agent Prompt — always Head(0), empty if unset
let prompt = self.def.as_ref()
    .map(|d| d.prompt.clone())
    .unwrap_or_default();
b = b.add_contributor(Box::new(
    vol_llm_context::builtin::SimpleContributor::system(prompt),
));

// 2. Skills — always Head(1)
b = b.add_contributor(Box::new(
    SkillInjector::new(skill_loader, AttentionAnchor::Head(1)),
));

// 3. Custom Files — Middle(0..n) from AgentDef.context_files
if let Some(ref def) = self.def {
    if !def.context_files.is_empty() {
        let working_dir = def.working_dir.as_ref();
        let specs: Vec<vol_llm_context::builtin::FileSpec> = def
            .context_files
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let full_path = working_dir
                    .map(|d| d.join(path))
                    .unwrap_or_else(|| PathBuf::from(path));
                vol_llm_context::builtin::FileSpec::new(
                    full_path,
                    AttentionAnchor::Middle(i as u32),
                )
            })
            .collect();
        b = b.add_contributor(Box::new(
            vol_llm_context::builtin::FileContributor::new(specs),
        ));
    }
}

// 4. Clone existing context_builder contributors (if any)
if let Some(ref cb) = self.context_builder {
    b = b.add_contributors_from(cb);
}

// 5. Manual contributors from with_system_prompt / with_contributor
for c in self.contributors {
    b = b.add_contributor(c);
}

// 6. Session — always Tail(0)
let max_history = self.def.as_ref()
    .and_then(|d| d.max_history_messages)
    .unwrap_or(50);
b = b.add_contributor(Box::new(SessionContributor::new(
    Arc::new(tokio::sync::Mutex::new((*session).clone())),
    max_history,
    AttentionAnchor::Tail(0),
)));

b.build()
```

- [ ] **Step 2: Add required imports to config_builder.rs**

```rust
use std::path::PathBuf;
use vol_llm_context::AttentionAnchor;
```

(Check if already imported — `AttentionAnchor` may not be directly imported yet.)

- [ ] **Step 3: Update agent.rs SessionContributor call (line 231-234)**

```rust
// Before:
let session_contributor = Box::new(SessionContributor::new(
    Arc::new(tokio::sync::Mutex::new((*session).clone())),
    max_history,
));
// After:
let session_contributor = Box::new(SessionContributor::new(
    Arc::new(tokio::sync::Mutex::new((*session).clone())),
    max_history,
    AttentionAnchor::Tail(0),
));
```

Add import if needed: `use vol_llm_context::AttentionAnchor;`

- [ ] **Step 4: Run tests to verify**

```bash
cargo test -p vol-llm-agent -- config_builder
cargo test -p vol-llm-agent -- agent
```

Expected: existing tests pass.

- [ ] **Step 5: Full workspace check**

```bash
cargo check --workspace 2>&1
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/config_builder.rs crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat(context): enforce standardized context ordering in AgentConfigBuilder

Head(0)=AgentPrompt, Head(1)=Skills, Middle(0..n)=CustomFiles, Tail(0)=Session.
Always add placeholder contributors for Head and Tail sections.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Document standard in CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add Context Ordering Standard section to CLAUDE.md**

Append after the existing content:

````markdown
## Context Ordering Standard

Agent context is assembled by `ContextBuilder` in the following fixed order:

| Zone | Position | Name | Source | Required |
|------|----------|------|--------|----------|
| Head | 0 | Agent Prompt | `AgentDef.prompt` | Yes (empty placeholder if unset) |
| Head | 1 | Skills | `SkillInjector` | Yes (empty block if no skills loaded) |
| Middle | 0..n | Custom Files | `AgentDef.context_files` (paths relative to work_dir) | No |
| Tail | 0 | Session | `SessionContributor` (conversation history) | Yes |

**Rules:**

- Head and Tail sections are fixed-position — always present, never dropped on budget overflow.
- Custom Files are loaded from disk in array order: first file → `Middle(0)`, second → `Middle(1)`, etc.
- Middle blocks are eligible for budget-driven truncation (highest position dropped first).
- All new contributors MUST declare their zone and position explicitly.

**Implementation:**

- `AgentConfigBuilder::build()` in `crates/vol-llm-agent/src/react/config_builder.rs` enforces this order.
- Head/Tail contributors are always registered (with empty content if no source data).
- `SkillInjector` and `SessionContributor` accept `AttentionAnchor` in their constructors — the anchor is NOT hardcoded.
````

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add Context Ordering Standard to CLAUDE.md

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 7: Full workspace verification

- [ ] **Step 1: cargo check entire workspace**

```bash
cargo check --workspace 2>&1
```

Expected: zero errors.

- [ ] **Step 2: cargo test entire workspace**

```bash
cargo test --workspace 2>&1
```

Expected: all tests pass.

- [ ] **Step 3: cargo clippy**

```bash
cargo clippy --workspace -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 4: Final commit if any clippy fixes needed**

```bash
git add -A && git commit -m "chore: workspace-wide verification and clippy fixes"
```
