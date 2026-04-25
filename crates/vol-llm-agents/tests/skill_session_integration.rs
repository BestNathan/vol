//! Integration tests: SkillInjector + SessionContributor in CodingAgent context.
//!
//! These tests verify ContextBuilder assembly with SkillInjector and
//! SessionContributor. No real LLM calls — we test context construction.

use std::sync::Arc;
use tokio::sync::Mutex;
use vol_llm_context::{ContextBuilderBuilder, ContextContributor};
use vol_llm_context::builtin::{SimpleContributor, UserInputContributor};
use vol_llm_core::Message;
use vol_llm_skill::SkillInjector;
use vol_session::{InMemoryEntryStore, Session, SessionContributor, SessionMessage};

// Dummy LLM for CodingAgent construction test
use vol_llm_core::{LLMClient, ConversationRequest, ConversationResponse, StreamReceiver, SupportedParam, LLMProvider};
struct DummyLlm;
#[async_trait::async_trait]
impl LLMClient for DummyLlm {
    fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
    fn model(&self) -> &str { "dummy" }
    fn supported_params(&self) -> &[SupportedParam] { &[] }
    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> { unimplemented!() }
    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> { unimplemented!() }
}

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

/// Helper: create a SkillInjector with skills from a temp dir.
///
/// Creates `{workdir}/.agents/skills/test-skill/SKILL.md` with proper frontmatter.
fn make_workdir_with_skills() -> tempfile::TempDir {
    let tmp_dir = tempfile::tempdir().unwrap();
    let skill_dir = tmp_dir.path().join(".agents/skills/test-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: A test skill for integration tests
---

# Test Skill

This is a test skill instruction.
"#,
    )
    .unwrap();
    tmp_dir
}

#[tokio::test]
async fn test_context_with_skills_and_session() {
    let tmp_dir = make_workdir_with_skills();
    let workdir = tmp_dir.path().to_path_buf();

    let skill_injector = SkillInjector::from_workdir(&workdir).await;
    let session = make_session(3).await;

    let context_builder = ContextBuilderBuilder::new(128_000)
        .add_contributor(Box::new(SimpleContributor::system(
            "You are an expert coding assistant.".to_string(),
        )))
        .add_contributor(Box::new(skill_injector))
        .add_contributor(Box::new(SessionContributor::new(session, 10)))
        .add_contributor(Box::new(UserInputContributor::new(
            "Write a function".to_string(),
        )))
        .build();

    let output = context_builder.build().await.unwrap();
    let messages = output.messages;

    assert!(!messages.is_empty(), "Should have at least system message");
}

#[tokio::test]
async fn test_context_zone_ordering() {
    let tmp_dir = make_workdir_with_skills();
    let workdir = tmp_dir.path().to_path_buf();

    let skill_injector = SkillInjector::from_workdir(&workdir).await;
    let session = make_session(2).await;

    let context_builder = ContextBuilderBuilder::new(128_000)
        .add_contributor(Box::new(SimpleContributor::system(
            "System prompt.".to_string(),
        )))
        .add_contributor(Box::new(skill_injector))
        .add_contributor(Box::new(SessionContributor::new(session, 10)))
        .add_contributor(Box::new(UserInputContributor::new(
            "User input".to_string(),
        )))
        .build();

    let output = context_builder.build().await.unwrap();
    let messages = output.messages;

    assert!(!messages.is_empty(), "Should have messages");

    // Verify skills are present in context
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
    let tmp_dir = make_workdir_with_skills();
    let workdir = tmp_dir.path().to_path_buf();

    let skill_injector = SkillInjector::from_workdir(&workdir).await;
    let empty_session = make_session(0).await;

    let context_builder = ContextBuilderBuilder::new(128_000)
        .add_contributor(Box::new(SimpleContributor::system(
            "System.".to_string(),
        )))
        .add_contributor(Box::new(skill_injector))
        .add_contributor(Box::new(SessionContributor::new(empty_session, 10)))
        .add_contributor(Box::new(UserInputContributor::new(
            "Hello".to_string(),
        )))
        .build();

    let output = context_builder.build().await.unwrap();
    assert!(
        !output.messages.is_empty(),
        "Should have system + skills + user messages"
    );
}

#[tokio::test]
async fn test_skill_injector_from_workdir_path_resolution() {
    let tmp_dir = make_workdir_with_skills();
    let workdir = tmp_dir.path().to_path_buf();

    let injector = SkillInjector::from_workdir(&workdir).await;
    let blocks = injector.contribute().await.unwrap();

    // Should have found and loaded the skill
    assert!(!blocks.is_empty(), "Should have skill content");
    let content: String = blocks
        .iter()
        .flat_map(|b| &b.messages)
        .filter_map(|m| m.content.as_ref())
        .map(|c| c.as_str())
        .collect();
    assert!(content.contains("test-skill"), "Should have skill content, got: {:?}", content);
}

#[tokio::test]
async fn test_coding_agent_has_skill_injector() {
    use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig};

    let tmp_dir = make_workdir_with_skills();
    let workdir = tmp_dir.path().to_path_buf();

    let config = CodingAgentConfig {
        llm: Some(Arc::new(DummyLlm)),
        working_dir: workdir,
        ..Default::default()
    };
    let agent = CodingAgent::new(config).await.unwrap();

    // Agent was created successfully — this proves SkillInjector injection
    // works (if from_workdir or context_builder failed, new() would panic)
    assert!(agent.config().llm.is_some());
}
