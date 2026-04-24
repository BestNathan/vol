//! Compression strategy tests with realistic conversation patterns.
//!
//! Verifies PositionSampleCompressor and RoleFilterCompressor behavior
//! with mixed-role conversations.

use vol_llm_core::{Message, MessageRole};
use vol_session::{
    PositionSampleCompressor, RoleFilterCompressor,
    SessionMessage, MessageCompressor,
};

// ─── Helper: create a realistic conversation ────────────────────────────────

fn make_conversation(n_pairs: usize) -> Vec<SessionMessage> {
    let mut messages = Vec::new();
    for i in 0..n_pairs {
        // User message
        messages.push(SessionMessage::new(
            "sess".to_string(),
            Message::user(format!("User question {}", i)),
        ));
        // Assistant response
        messages.push(SessionMessage::new(
            "sess".to_string(),
            Message::assistant(format!("Assistant reply {}", i)),
        ));
        // Tool call (every other pair)
        if i % 2 == 0 {
            let mut tool_msg = SessionMessage::new(
                "sess".to_string(),
                Message::assistant(format!("Tool result {}", i)),
            );
            tool_msg.message.role = MessageRole::Tool;
            messages.push(tool_msg);
        }
    }
    messages
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_position_sample_compressor_realistic() {
    let conversation = make_conversation(10); // ~25 messages
    assert!(conversation.len() > 10);

    let compressor = PositionSampleCompressor::new(3, 5);
    let result = compressor.compress(conversation.clone()).await;

    // Should have fewer messages than input
    assert!(result.len() < conversation.len());
    // Should keep at least first N
    assert!(result.len() >= 3);
    // First message should be from input[0]
    assert_eq!(result[0].id, conversation[0].id);
    // Last message should be the original last
    assert_eq!(result.last().unwrap().id, conversation.last().unwrap().id);
}

#[tokio::test]
async fn test_position_sample_compressor_small_input() {
    let messages = make_conversation(1); // 2-3 messages
    let compressor = PositionSampleCompressor::new(3, 5);
    let result = compressor.compress(messages).await;

    // Small input should be mostly preserved
    assert!(result.len() >= 2);
}

#[tokio::test]
async fn test_role_filter_compressor_removes_tool_messages() {
    let conversation = make_conversation(10);
    let tool_count = conversation.iter()
        .filter(|m| m.message.role == MessageRole::Tool)
        .count();
    assert!(tool_count > 0, "Test conversation should have Tool messages");

    let compressor = RoleFilterCompressor::default();
    let result = compressor.compress(conversation.clone()).await;

    // No Tool messages should survive
    let remaining_tool = result.iter()
        .filter(|m| m.message.role == MessageRole::Tool)
        .count();
    assert_eq!(remaining_tool, 0);
    // User and Assistant should survive
    assert!(result.len() > 0);
}

#[tokio::test]
async fn test_role_filter_compressor_custom_roles() {
    let conversation = make_conversation(5);
    let n_pairs = 5;

    // Keep only User messages
    let compressor = RoleFilterCompressor::new(vec![MessageRole::User]);
    let result = compressor.compress(conversation).await;

    // With n_pairs=5: 5 User messages (one per pair), plus Assistant and Tool messages.
    let expected_user_count = n_pairs;
    assert_eq!(result.len(), expected_user_count);
    assert!(result.iter().all(|m| m.message.role == MessageRole::User));
}

#[tokio::test]
async fn test_compressor_empty_input() {
    let pos = PositionSampleCompressor::default();
    let result = pos.compress(vec![]).await;
    assert!(result.is_empty());

    let role = RoleFilterCompressor::default();
    let result = role.compress(vec![]).await;
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_position_sample_different_configs() {
    let conversation = make_conversation(20);

    // Aggressive: keep 1, sample every 10
    let aggressive = PositionSampleCompressor::new(1, 10);
    let aggressive_result = aggressive.compress(conversation.clone()).await;

    // Conservative: keep 5, sample every 2
    let conservative = PositionSampleCompressor::new(5, 2);
    let conservative_result = conservative.compress(conversation.clone()).await;

    // Aggressive should produce fewer messages
    assert!(
        aggressive_result.len() < conservative_result.len(),
        "Aggressive ({}) should produce fewer than conservative ({})",
        aggressive_result.len(),
        conservative_result.len()
    );
    // Both should preserve first message
    assert_eq!(aggressive_result[0].id, conversation[0].id);
    assert_eq!(conservative_result[0].id, conversation[0].id);
}
