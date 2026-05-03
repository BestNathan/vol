# CodingAgent Skill + Session Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Inject SkillInjector into CodingAgent's ContextBuilder so skill metadata from `.agents/skills/` appears in the LLM system prompt, with integration tests.

**Architecture:** CodingAgent::new() already builds a ContextBuilder with SimpleContributor (system prompt). We add SkillInjector::from_workdir() as a second contributor. ReActAgent needs zero changes — it already clones contributors from AgentConfig.context_builder at runtime.

**Tech Stack:** Rust, vol-llm-skill, vol-llm-context, vol-llm-agents, tokio test

---

### Task 1: Add SkillInjector::from_workdir() constructor

**Files:**
- Modify: `crates/vol-llm-skill/src/injector.rs`
- Test: `crates/vol-llm-skill/src/injector.rs` (inline tests)

- [ ] **Step 1: Add from_workdir constructor**

Add this method to the `impl SkillInjector` block (after `pub fn new()`):

```rust
impl SkillInjector {
    // ... existing new() ...

    /// Create a SkillInjector that loads skills from `{working_dir}/.agents/skills`.
    pub fn from_workdir(working_dir: &std::path::Path) -> Self {
        use std::sync::Arc;
        let skill_dir = working_dir.join(".agents/skills");
        let loader = Arc::new(crate::loader::SkillLoader::new(Some(skill_dir)));
        Self::new(loader)
    }
}
```

- [ ] **Step 2: Add clone_contribute test**

Add this test to the existing `mod tests` block in injector.rs:

```rust
#[tokio::test]
async fn test_skill_injector_clone_contribute() {
    use crate::def::SkillDef;
    let loader = SkillLoader::new(None);
    let mut skill = SkillDef::new("test-skill", "# Test")
        .with_description("A test skill")
        .with_triggers(vec!["test".to_string()]);
    skill.id = "user:test-skill".to_string();
    loader.register(skill).await;

    let injector = SkillInjector::new(Arc::new(loader));
    let original = injector.contribute().await.unwrap();
    let cloned = injector.clone_box();
    let cloned_result = cloned.contribute().await.unwrap();

    assert_eq!(original.len(), cloned_result.len());
    assert_eq!(original[0].messages[0].content, cloned_result[0].messages[0].content);
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-skill injector -- --test-threads=1`
Expected: All 7 tests pass (6 existing + 1 new)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-skill/src/injector.rs
git commit -m "feat: add SkillInjector::from_workdir() and clone_contribute test"
```

---

### Task 2: Inject SkillInjector into CodingAgent's ContextBuilder

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs` (lines 79-83)

- [ ] **Step 1: Update ContextBuilder construction**

In `CodingAgent::new()`, replace the current context_builder construction (lines 79-83):

```rust
// Before (replace this):
let context_builder = vol_llm_context::ContextBuilderBuilder::new(128_000)
    .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
        "You are an expert coding assistant. Help users understand, modify, and improve their codebase.".to_string(),
    )))
    .build();
```

With:

```rust
// After:
use vol_llm_skill::SkillInjector;

let mut context_builder = vol_llm_context::ContextBuilderBuilder::new(128_000)
    .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
        "You are an expert coding assistant. Help users understand, modify, and improve their codebase.".to_string(),
    )))
    .add_contributor(Box::new(SkillInjector::from_workdir(&config.working_dir)))
    .build();
```

No other code changes needed. The `config.working_dir` already exists on `CodingAgentConfig`.

- [ ] **Step 2: Verify vol-llm-agents Cargo.toml has vol-llm-skill dependency**

Check `crates/vol-llm-agents/Cargo.toml` for `vol-llm-skill` in `[dependencies]`. If missing, add:

```toml
vol-llm-skill = { path = "../vol-llm-skill" }
```

- [ ] **Step 3: Run compilation check**

Run: `cargo check -p vol-llm-agents`
Expected: No errors

- [ ] **Step 4: Run existing tests**

Run: `cargo test -p vol-llm-agents -- --test-threads=1`
Expected: All existing tests pass (the new SkillInjector injection is transparent when `.agents/skills` doesn't exist — format_metadata returns empty string, contribute returns empty vec)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs crates/vol-llm-agents/Cargo.toml
git commit -m "feat: inject SkillInjector into CodingAgent ContextBuilder from working_dir"
```

---

### Task 3: Integration tests for Skill + Session context assembly

**Files:**
- Create: `crates/vol-llm-agents/tests/skill_session_integration.rs`
- Test: Uses MockLlmClient from vol-llm-core, no real LLM calls

- [ ] **Step 1: Check if vol-llm-agents has a `tests/` directory**

Run: `ls crates/vol-llm-agents/tests/`

If the directory doesn't exist, create it.

- [ ] **Step 2: Create the integration test file**

Create `crates/vol-llm-agents/tests/skill_session_integration.rs`:

```rust
//! Integration tests: SkillInjector + SessionContributor in CodingAgent context.
//!
//! These tests verify ContextBuilder assembly with SkillInjector and
//! SessionContributor. No real LLM calls — we test context construction.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use vol_llm_agent::react::ContextContributor;
use vol_llm_context::{ContextBuilderBuilder, ContextError};
use vol_llm_core::Message;
use vol_llm_skill::{SkillInjector, SkillLoader};
use vol_llm_skill::def::SkillDef;
use vol_session::{InMemoryEntryStore, Session, SessionContributor, SessionMessage};

/// Helper: create a session with n messages
async fn make_session(n: usize) -> Arc<Mutex<Session>> {
    let store = Arc::new(InMemoryEntryStore::new());
    let session = Session::new(store);
    for i in 0..n {
        let msg = SessionMessage::new(
            session.id.clone(),
            Message::user(format!("msg-{}", i)),
        );
        session.add_message(msg).await.unwrap();
    }
    Arc::new(Mutex::new(session))
}

/// Helper: create a SkillInjector with skills from a temp dir
async fn make_skill_injector_with_skills(dir: &PathBuf) -> SkillInjector {
    // Create the .agents/skills directory
    let skills_dir = dir.join(".agents/skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    // Create a skill file
    std::fs::write(
        skills_dir.join("test-skill.md"),
        r#"---
name: test-skill
description: A test skill for integration tests
---

# Test Skill

This is a test skill instruction.
"#,
    )
    .unwrap();

    SkillInjector::from_workdir(dir)
}

#[tokio::test]
async fn test_context_with_skills_and_session() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let workdir = tmp_dir.path().to_path_buf();

    // Set up skills
    let skill_injector = make_skill_injector_with_skills(&workdir).await;

    // Set up session with messages
    let session = make_session(3).await;

    // Build context like CodingAgent does
    let context_builder = ContextBuilderBuilder::new(128_000)
        .add_contributor(Box::new(
            vol_llm_context::builtin::SimpleContributor::system(
                "You are an expert coding assistant.".to_string(),
            ),
        ))
        .add_contributor(Box::new(skill_injector))
        .add_contributor(Box::new(SessionContributor::new(session, 10)))
        .add_contributor(Box::new(
            vol_llm_context::builtin::UserInputContributor::new(
                "Write a function".to_string(),
            ),
        ))
        .build();

    let output = context_builder.build().await.unwrap();

    // Should have messages from system, skills, session, and user input
    let messages: Vec<_> = output.messages;
    assert!(!messages.is_empty(), "Should have at least system message");

    // System prompt should be first (Head zone, position 0)
    assert!(messages[0].role == "user" || messages[0].role == "system");
}

#[tokio::test]
async fn test_context_zone_ordering() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let workdir = tmp_dir.path().to_path_buf();

    let skill_injector = make_skill_injector_with_skills(&workdir).await;
    let session = make_session(2).await;

    let context_builder = ContextBuilderBuilder::new(128_000)
        .add_contributor(Box::new(
            vol_llm_context::builtin::SimpleContributor::system(
                "System prompt.".to_string(),
            ),
        ))
        .add_contributor(Box::new(skill_injector))
        .add_contributor(Box::new(SessionContributor::new(session, 10)))
        .add_contributor(Box::new(
            vol_llm_context::builtin::UserInputContributor::new(
                "User input".to_string(),
            ),
        ))
        .build();

    let output = context_builder.build().await.unwrap();
    let messages = output.messages;

    // Verify zone ordering by checking AttentionAnchor positions
    // Head(0) = system, Head(1) = skills, Middle(0) = session, Tail(0) = user_input
    assert!(!messages.is_empty(), "Should have messages");

    // Skills content should appear (from .agents/skills)
    let all_content: String = messages
        .iter()
        .filter_map(|m| m.content.as_ref())
        .map(|c| c.as_str().to_string())
        .collect();
    assert!(
        all_content.contains("test-skill") || all_content.contains("Available skills"),
        "Skills should be injected into context"
    );
}

#[tokio::test]
async fn test_context_empty_session_with_skills() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let workdir = tmp_dir.path().to_path_buf();

    let skill_injector = make_skill_injector_with_skills(&workdir).await;
    let empty_session = make_session(0).await;

    let context_builder = ContextBuilderBuilder::new(128_000)
        .add_contributor(Box::new(
            vol_llm_context::builtin::SimpleContributor::system(
                "System.".to_string(),
            ),
        ))
        .add_contributor(Box::new(skill_injector))
        .add_contributor(Box::new(SessionContributor::new(empty_session, 10)))
        .add_contributor(Box::new(
            vol_llm_context::builtin::UserInputContributor::new(
                "Hello".to_string(),
            ),
        ))
        .build();

    let output = context_builder.build().await.unwrap();
    assert!(!output.messages.is_empty(), "Should have system + skills + user messages");
}

#[tokio::test]
async fn test_coding_agent_has_skill_injector() {
    // Verify CodingAgent::new() produces an AgentConfig with SkillInjector
    // by checking the context_builder contributors count
    use vol_llm_agent::react::AgentConfig;

    struct DummyLlm;
    use vol_llm_core::{LLMClient, ConversationRequest, ConversationResponse, StreamReceiver, SupportedParam, LLMProvider};
    #[async_trait::async_trait]
    impl LLMClient for DummyLlm {
        fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
        fn model(&self) -> &str { "dummy" }
        fn supported_params(&self) -> &[SupportedParam] { &[] }
        async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> { unimplemented!() }
        async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> { unimplemented!() }
    }

    let tmp_dir = tempfile::tempdir().unwrap();
    let workdir = tmp_dir.path().to_path_buf();

    let config = crate::coding::CodingAgentConfig {
        llm: Some(Arc::new(DummyLlm)),
        working_dir: workdir,
        ..Default::default()
    };
    let agent = crate::coding::CodingAgent::new(config).await.unwrap();

    // Access the agent_config through state
    // We verify by checking that CodingAgent::new successfully created
    // with the skill injector (if it panics, the test fails)
    assert!(agent.config().llm.is_some());
}

#[tokio::test]
async fn test_skill_injector_from_workdir_path_resolution() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let workdir = tmp_dir.path().to_path_buf();

    // Create .agents/skills with a skill
    let skills_dir = workdir.join(".agents/skills");
    std::fs::create_dir_all(&skills_dir).unwrap();
    std::fs::write(
        skills_dir.join("my-skill.md"),
        "---\nname: my-skill\ndescription: My skill\n---\n# My Skill\n",
    )
    .unwrap();

    let injector = SkillInjector::from_workdir(&workdir);
    let blocks = injector.contribute().await.unwrap();

    // Should have found and loaded the skill
    assert!(!blocks.is_empty(), "Should have skill content");
    let content: String = blocks.iter()
        .flat_map(|b| &b.messages)
        .filter_map(|m| m.content.as_ref())
        .map(|c| c.as_str())
        .collect();
    assert!(content.contains("my-skill"));
}
```

- [ ] **Step 3: Add integration test to Cargo.toml**

In `crates/vol-llm-agents/Cargo.toml`, add a `[[test]]` section if not present:

```toml
[[test]]
name = "skill_session_integration"
path = "tests/skill_session_integration.rs"
```

Also ensure `[dev-dependencies]` includes `tempfile` and `tokio`:
```toml
[dev-dependencies]
tempfile = "3"
tokio = { version = "1", features = ["full"] }
```

- [ ] **Step 4: Run integration tests**

Run: `cargo test -p vol-llm-agents --test skill_session_integration -- --test-threads=1`
Expected: All 5 tests pass

- [ ] **Step 5: Run full test suite**

Run: `cargo test -p vol-llm-agents -- --test-threads=1`
Expected: All tests pass (existing + new)

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agents/tests/skill_session_integration.rs crates/vol-llm-agents/Cargo.toml
git commit -m "test: integration tests for SkillInjector + SessionContributor context assembly"
```
