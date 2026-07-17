//! Integration tests for prompt_context module.
//!
//! Tests cover:
//! - Cache key stability across multiple calls
//! - RAG scenario with System/User separation
//! - Multi-turn conversation with history
//!
//! Run with: cargo test --test prompt_context_integration

use vol_llm_agent::prompt_context::{
    FragmentType, MessageAssembler, PromptContext, PromptFragment, PromptTemplate,
};
use vol_llm_agent::rag::Document;
use vol_llm_core::Message;

#[test]
fn test_cache_key_stability_same_config() {
    // Test that identical configurations produce identical cache keys
    let template = PromptTemplate::new(
        "market-analyst",
        r#"You are a {role}.

## Tools
{tools}

## Rules
{rules}"#,
    );

    let tools_json = r#"[{"name": "get_weather"}]"#;

    // First call
    let context1 = PromptContext::new(template.clone())
        .with_fragment(PromptFragment::new(
            "role",
            "Financial analyst",
            FragmentType::Role,
        ))
        .with_fragment(PromptFragment::new(
            "tools",
            tools_json,
            FragmentType::Tools,
        ))
        .with_fragment(PromptFragment::new(
            "rules",
            "Be accurate",
            FragmentType::Rules,
        ));

    // Second call with identical configuration
    let context2 = PromptContext::new(template)
        .with_fragment(PromptFragment::new(
            "role",
            "Financial analyst",
            FragmentType::Role,
        ))
        .with_fragment(PromptFragment::new(
            "tools",
            tools_json,
            FragmentType::Tools,
        ))
        .with_fragment(PromptFragment::new(
            "rules",
            "Be accurate",
            FragmentType::Rules,
        ));

    // Cache keys must be identical
    assert_eq!(
        context1.cache_key(),
        context2.cache_key(),
        "Identical configurations should produce identical cache keys"
    );
}

#[test]
fn test_cache_key_changes_with_different_fragments() {
    // Test that different fragment content produces different cache keys
    let template = PromptTemplate::new("test", "Role: {role}");

    let context1 = PromptContext::new(template.clone()).with_fragment(PromptFragment::new(
        "role",
        "Analyst",
        FragmentType::Role,
    ));

    let context2 = PromptContext::new(template).with_fragment(PromptFragment::new(
        "role",
        "Assistant",
        FragmentType::Role,
    ));

    // Different fragment content should produce different cache keys
    assert_ne!(
        context1.cache_key(),
        context2.cache_key(),
        "Different fragment content should produce different cache keys"
    );
}

#[test]
fn test_cache_key_unchanged_by_dynamic_variables() {
    // Test that dynamic variables don't affect cache key
    let template = PromptTemplate::new("test", "Query: {query}");
    let fragment = PromptFragment::new("query", "Fixed query", FragmentType::Custom);

    let context1 = PromptContext::new(template.clone()).with_fragment(fragment.clone());

    let context2 = PromptContext::new(template)
        .with_fragment(fragment)
        .with_dynamic("query", "Different dynamic value");

    // Dynamic variables should not affect cache key
    assert_eq!(
        context1.cache_key(),
        context2.cache_key(),
        "Dynamic variables should not affect cache key"
    );
}

#[test]
fn test_rag_scenario_system_user_separation() {
    // Test RAG scenario: System contains fixed content, User contains RAG context
    let template = PromptTemplate::new(
        "rag-agent",
        "You are a knowledge base assistant.\n\n## Tools\n{tools}",
    );

    let prompt_ctx = PromptContext::new(template).with_fragment(PromptFragment::new(
        "tools",
        "- search_knowledge_base: Search for information",
        FragmentType::Tools,
    ));

    let rag_docs = vec![
        Document::new(
            "IV (Implied Volatility) is the market's forecast of future volatility.".to_string(),
        )
        .with_metadata("source", "options_glossary")
        .with_score(0.92),
        Document::new("Higher IV indicates greater expected price movement.".to_string())
            .with_metadata("source", "trading_basics")
            .with_score(0.85),
    ];

    let messages =
        MessageAssembler::assemble_with_rag(&prompt_ctx, "Explain implied volatility", &rag_docs);

    // Verify message structure
    assert_eq!(messages.len(), 2, "Should have System + User messages");
    assert_eq!(messages[0].role, vol_llm_core::MessageRole::System);
    assert_eq!(messages[1].role, vol_llm_core::MessageRole::User);

    // Verify System message contains fixed content only
    let system_content = messages[0].content.as_ref().unwrap().as_str();
    assert!(
        system_content.contains("knowledge base assistant"),
        "System should contain role definition"
    );
    assert!(
        system_content.contains("search_knowledge_base"),
        "System should contain tools"
    );
    assert!(
        !system_content.contains("参考资料"),
        "System should not contain RAG context"
    );
    assert!(
        !system_content.contains("IV"),
        "System should not contain dynamic RAG content"
    );

    // Verify User message contains RAG context
    let user_content = messages[1].content.as_ref().unwrap().as_str();
    assert!(
        user_content.contains("参考资料"),
        "User should contain RAG header"
    );
    assert!(
        user_content.contains("IV (Implied Volatility)"),
        "User should contain first RAG document"
    );
    assert!(
        user_content.contains("Higher IV"),
        "User should contain second RAG document"
    );
    assert!(
        user_content.contains("Explain implied volatility"),
        "User should contain original query"
    );
}

#[test]
fn test_rag_scenario_empty_documents() {
    // Test RAG scenario with empty document list
    let template = PromptTemplate::new("test", "System prompt");
    let prompt_ctx = PromptContext::new(template);

    let messages = MessageAssembler::assemble_with_rag(&prompt_ctx, "What is AI?", &[]);

    assert_eq!(messages.len(), 2);
    let user_content = messages[1].content.as_ref().unwrap().as_str();
    assert!(user_content.contains("问题：What is AI?"));
    // Empty docs should result in empty RAG context section
    assert!(!user_content.contains("参考资料:\n\n---"));
}

#[test]
fn test_multi_turn_conversation_system_once() {
    // Test multi-turn conversation: System appears only once, history accumulates
    // Note: assemble_with_history always adds a fresh System message
    // So history should only contain User/Assistant messages, not System
    let template = PromptTemplate::new("assistant", "You are a helpful AI assistant.");

    let prompt_ctx = PromptContext::new(template);

    // Turn 1
    let turn1_messages = MessageAssembler::assemble(&prompt_ctx, "What is IV?");
    assert_eq!(turn1_messages.len(), 2);
    assert_eq!(turn1_messages[0].role, vol_llm_core::MessageRole::System);

    // Simulate assistant response
    let turn1_response = Message::assistant("IV stands for Implied Volatility.");

    // Turn 2: Build history from turn 1 (exclude System, as assemble_with_history adds fresh System)
    let history = vec![
        turn1_messages[1].clone(), // User turn 1
        turn1_response,            // Assistant turn 1
    ];

    let turn2_messages =
        MessageAssembler::assemble_with_history(&prompt_ctx, "What about CV?", &history);

    // Turn 2 should include: System (1) + history (2) + current user (1) = 4
    assert_eq!(turn2_messages.len(), 4);

    // System should appear only once at the beginning
    assert_eq!(turn2_messages[0].role, vol_llm_core::MessageRole::System);

    // Count System messages - should be exactly 1
    let system_count = turn2_messages
        .iter()
        .filter(|m| m.role == vol_llm_core::MessageRole::System)
        .count();
    assert_eq!(system_count, 1, "System message should appear exactly once");

    // Verify history is preserved
    assert!(turn2_messages[1]
        .content
        .as_ref()
        .unwrap()
        .as_str()
        .contains("What is IV?"));
    assert!(turn2_messages[2]
        .content
        .as_ref()
        .unwrap()
        .as_str()
        .contains("Implied Volatility"));
    assert!(turn2_messages[3]
        .content
        .as_ref()
        .unwrap()
        .as_str()
        .contains("What about CV?"));
}

#[test]
fn test_multi_turn_conversation_with_rag_history() {
    // Test multi-turn conversation where RAG context is passed through history
    let template = PromptTemplate::new(
        "rag-assistant",
        "You are a RAG-powered assistant.\n\n## Tools\n{tools}",
    );

    let prompt_ctx = PromptContext::new(template).with_fragment(PromptFragment::new(
        "tools",
        "- search: Search knowledge base",
        FragmentType::Tools,
    ));

    // Turn 1: RAG query
    let turn1_docs = vec![Document::new(
        "Gamma measures the rate of change of delta.".to_string(),
    )];

    let turn1_messages =
        MessageAssembler::assemble_with_rag(&prompt_ctx, "What is gamma?", &turn1_docs);

    // Simulate assistant response
    let turn1_response = Message::assistant(
        "Gamma measures how quickly delta changes as the underlying price moves.",
    );

    // Turn 2: Build history (exclude System, as assemble_with_history adds fresh System)
    // Note: RAG context from turn 1 is in turn1_messages[1] (User message)
    let history = vec![
        turn1_messages[1].clone(), // User turn 1 (with RAG)
        turn1_response,            // Assistant turn 1
    ];

    let turn2_messages = MessageAssembler::assemble_with_history(
        &prompt_ctx,
        "How does it relate to delta?",
        &history,
    );

    // Verify turn 2 has correct structure
    assert_eq!(turn2_messages.len(), 4); // System + 2 history + current user
    assert_eq!(turn2_messages[0].role, vol_llm_core::MessageRole::System);

    // Verify RAG context from turn 1 is in history
    let turn1_user_content = turn2_messages[1].content.as_ref().unwrap().as_str();
    assert!(
        turn1_user_content.contains("Gamma measures"),
        "Turn 1 RAG context should be in history"
    );

    // Current user message should not have RAG context (not provided for turn 2)
    let turn2_user_content = turn2_messages[3].content.as_ref().unwrap().as_str();
    assert!(turn2_user_content.contains("How does it relate to delta?"));
    assert!(!turn2_user_content.contains("参考资料:"));
}

#[test]
fn test_multi_turn_accumulates_history_correctly() {
    // Test that history accumulates correctly across multiple turns
    // Note: history passed to assemble_with_history should not include System
    let template = PromptTemplate::new("test", "You are helpful.");
    let prompt_ctx = PromptContext::new(template);

    // Turn 1
    let msg1 = MessageAssembler::assemble(&prompt_ctx, "Q1");
    let resp1 = Message::assistant("A1");

    // Turn 2: history excludes System
    let history2 = vec![msg1[1].clone(), resp1]; // User1 + Assistant1
    let msg2 = MessageAssembler::assemble_with_history(&prompt_ctx, "Q2", &history2);
    let _resp2 = Message::assistant("A2");

    // Turn 3: history accumulates all User/Assistant messages
    let history3 = vec![
        msg2[1].clone(), // User1
        msg2[2].clone(), // Assistant1
        msg2[3].clone(), // User2
    ];
    let msg3 = MessageAssembler::assemble_with_history(&prompt_ctx, "Q3", &history3);

    // Turn 3 should have: System (1) + history (3) + Q3 (1) = 5
    assert_eq!(msg3.len(), 5);

    // Verify System is first
    assert_eq!(msg3[0].role, vol_llm_core::MessageRole::System);

    // Verify all questions are in history
    let all_content: Vec<&str> = msg3
        .iter()
        .map(|m| m.content.as_ref().unwrap().as_str())
        .collect();

    assert!(all_content.iter().any(|c| c.contains("Q1")));
    assert!(all_content.iter().any(|c| c.contains("Q2")));
    assert!(all_content.iter().any(|c| c.contains("Q3")));

    // System appears exactly once
    let system_count = msg3
        .iter()
        .filter(|m| m.role == vol_llm_core::MessageRole::System)
        .count();
    assert_eq!(system_count, 1);
}

#[test]
fn test_build_user_with_and_without_rag_context() {
    // Test build_user method with and without RAG context
    let template = PromptTemplate::new("test", "System");
    let prompt_ctx = PromptContext::new(template);

    // Without RAG
    let user_without = prompt_ctx.build_user("What is the weather?", None);
    assert_eq!(user_without, "问题：What is the weather?");

    // With RAG
    let user_with = prompt_ctx.build_user("What is the weather?", Some("Forecast: Sunny, 25C"));
    assert!(user_with.contains("参考资料:"));
    assert!(user_with.contains("Forecast: Sunny, 25C"));
    assert!(user_with.contains("问题：What is the weather?"));
}

#[test]
fn test_integration_full_rag_workflow() {
    // Full RAG workflow simulation
    let template = PromptTemplate::new(
        "knowledge-assistant",
        "You are a knowledge base assistant.\n\n## Guidelines\n{guidelines}",
    );

    let prompt_ctx = PromptContext::new(template).with_fragment(PromptFragment::new(
        "guidelines",
        "- Always cite sources\n- Be concise",
        FragmentType::Rules,
    ));

    // Simulate RAG retrieval
    let rag_docs =
        vec![
            Document::new("Option delta measures sensitivity to underlying price.".to_string())
                .with_metadata("source", "options_101")
                .with_score(0.89),
        ];

    // Assemble messages
    let messages =
        MessageAssembler::assemble_with_rag(&prompt_ctx, "Explain option delta", &rag_docs);

    // Verify complete structure
    assert_eq!(messages.len(), 2);

    let system = messages[0].content.as_ref().unwrap().as_str();
    assert!(system.contains("knowledge base assistant"));
    assert!(system.contains("Always cite sources"));

    let user = messages[1].content.as_ref().unwrap().as_str();
    assert!(user.contains("参考资料:"));
    assert!(user.contains("Option delta"));
    assert!(user.contains("Explain option delta"));
}
