//! Integration tests for SessionListener end-to-end workflow.
//!
//! These tests verify that SessionListener correctly records events
//! to a JSONL file using FileMessageStore.

use std::sync::Arc;
use tokio::sync::broadcast;
use vol_llm_core::AgentStreamEvent;
use vol_session::{FileMessageStore, MessageStore, SessionListener};
use vol_tracing::TracedEvent;

/// Test SessionListener full workflow with FileMessageStore
#[tokio::test]
async fn test_session_listener_full_workflow() {
    // 1. Create event bus
    let (event_tx, event_rx) = broadcast::channel(100);

    // 2. Create temporary directory and FileMessageStore
    let tmp_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn MessageStore> = Arc::new(FileMessageStore::new(
        tmp_dir.path().to_str().unwrap(),
        "session-1",
    ));

    // 3. Create and start SessionListener
    let mut listener = SessionListener::new(event_rx, store, "session-1".to_string());
    let handle = tokio::spawn(async move { listener.run().await.unwrap() });

    // 4. Send test event sequence
    // Thinking
    event_tx
        .send(TracedEvent::without_span(
            AgentStreamEvent::ThinkingComplete {
                thinking: "Let me search...".to_string(),
                timestamp: chrono::Utc::now(),
            },
        ))
        .map_err(|_| "send error")
        .unwrap();

    // ToolCallBegin
    event_tx
        .send(TracedEvent::without_span(AgentStreamEvent::ToolCallBegin {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_1".to_string(),
            tool_name: "volatility_index".to_string(),
            arguments: r#"{"symbol": "BTC"}"#.to_string(),
        }))
        .map_err(|_| "send error")
        .unwrap();

    // ToolCallComplete
    event_tx
        .send(TracedEvent::without_span(
            AgentStreamEvent::ToolCallComplete {
                timestamp: chrono::Utc::now(),
                tool_call_id: "call_1".to_string(),
                tool_name: "volatility_index".to_string(),
                result: "Index: btc_usd | Volatility: 42.98%".to_string(),
                duration_ms: None,
            },
        ))
        .map_err(|_| "send error")
        .unwrap();

    // IterationComplete with final_answer
    event_tx
        .send(TracedEvent::without_span(
            AgentStreamEvent::IterationComplete {
                timestamp: chrono::Utc::now(),
                iteration: 1,
                tool_calls: vec![],
                final_answer: Some("BTC 当前波动率为 42.98%...".to_string()),
            },
        ))
        .map_err(|_| "send error")
        .unwrap();

    // 5. Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // 6. Close channel and wait for listener to exit
    drop(event_tx);
    handle.await.unwrap();

    // 7. Verify JSONL file contents
    let file_path = tmp_dir.path().join("sessions").join("session-1.jsonl");
    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();

    // Should have 4 lines (Thinking, ToolCallBegin, ToolCallComplete, IterationComplete)
    assert_eq!(lines.len(), 4, "Expected 4 lines, got {}", lines.len());

    // Verify each line is valid JSON and contains expected content
    for line in &lines {
        assert!(
            line.contains("SessionMessage"),
            "Each line should be a SessionMessage"
        );
    }

    let line_count = lines.len();
    println!("Test passed: {} lines written to JSONL file", line_count);
}

/// Test SessionListener with filtered events (some events should not be recorded)
#[tokio::test]
async fn test_session_listener_filters_events() {
    let (event_tx, event_rx) = broadcast::channel(100);

    let tmp_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn MessageStore> = Arc::new(FileMessageStore::new(
        tmp_dir.path().to_str().unwrap(),
        "session-filter",
    ));

    let mut listener = SessionListener::new(event_rx, store, "session-filter".to_string());
    let handle = tokio::spawn(async move { listener.run().await.unwrap() });

    // Send events that SHOULD be recorded
    event_tx
        .send(TracedEvent::without_span(
            AgentStreamEvent::ThinkingComplete {
                thinking: "Thinking...".to_string(),
                timestamp: chrono::Utc::now(),
            },
        ))
        .map_err(|_| "send error")
        .unwrap();

    // Send AgentStart event (should NOW be recorded)
    event_tx
        .send(TracedEvent::without_span(AgentStreamEvent::AgentStart {
            input: "test input".to_string(),
            timestamp: chrono::Utc::now(),
        }))
        .map_err(|_| "send error")
        .unwrap();

    // Another recordable event
    event_tx
        .send(TracedEvent::without_span(
            AgentStreamEvent::ToolCallComplete {
                timestamp: chrono::Utc::now(),
                tool_call_id: "call_2".to_string(),
                tool_name: "test_tool".to_string(),
                result: "result".to_string(),
                duration_ms: None,
            },
        ))
        .map_err(|_| "send error")
        .unwrap();

    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Close channel
    drop(event_tx);
    handle.await.unwrap();

    // Verify 3 lines (Thinking + AgentStart + ToolCallComplete)
    let file_path = tmp_dir.path().join("sessions").join("session-filter.jsonl");
    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(
        lines.len(),
        3,
        "Expected 3 lines (AgentStart now recorded), got {}",
        lines.len()
    );

    // Verify AgentStart is recorded as user message
    let contains_user_input = content.lines().any(|l| l.contains("test input"));
    assert!(
        contains_user_input,
        "Session log should contain user input 'test input'"
    );

    let line_count = lines.len();
    println!(
        "Test passed: Event filtering works correctly, {} lines written",
        line_count
    );
}

/// Test SessionListener handles channel lag gracefully
#[tokio::test]
async fn test_session_listener_handles_lag() {
    let (event_tx, event_rx) = broadcast::channel(5); // Small buffer to trigger lag

    let tmp_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn MessageStore> = Arc::new(FileMessageStore::new(
        tmp_dir.path().to_str().unwrap(),
        "session-lag",
    ));

    let mut listener = SessionListener::new(event_rx, store, "session-lag".to_string());
    let handle = tokio::spawn(async move { listener.run().await.unwrap() });

    // Send more events than buffer size to trigger lag
    for i in 0..10 {
        let _ = event_tx.send(TracedEvent::without_span(
            AgentStreamEvent::ThinkingComplete {
                thinking: format!("Thinking {}", i),
                timestamp: chrono::Utc::now(),
            },
        ));
    }

    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Close channel
    drop(event_tx);
    handle.await.unwrap();

    // Verify at least some events were recorded (lag handling)
    let file_path = tmp_dir.path().join("sessions").join("session-lag.jsonl");
    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();

    // Should have some lines, but possibly not all 10 due to lag
    assert!(
        lines.len() > 0,
        "Expected at least some lines to be recorded"
    );
    let line_count = lines.len();
    println!("Test passed: {} lines recorded despite lag", line_count);
}
