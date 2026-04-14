//! Integration tests for ChannelledEventObserver with concurrent sends

use vol_llm_agents::coding::{ChannelledEventObserver, EventObserver};
use vol_llm_core::AgentStreamEvent;
use std::sync::Arc;

#[tokio::test]
async fn test_concurrent_on_event_receives_all_events() {
    let observer = Arc::new(ChannelledEventObserver::new());

    // Spawn multiple tasks that send events concurrently
    let mut handles = Vec::new();
    for i in 0..10 {
        let obs = observer.clone();
        let handle = tokio::spawn(async move {
            let event = AgentStreamEvent::ToolCallBegin {
                tool_call_id: format!("{}", i),
                tool_name: format!("tool_{}", i),
                arguments: format!("arg_{}", i),
            };
            obs.on_event(&event).await.unwrap();
        });
        handles.push(handle);
    }

    // Wait for all senders to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Wait for consumer to process all events
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let events = observer.events().await;
    assert_eq!(events.len(), 10);

    // All events should be ToolCallBegin
    for event in &events {
        assert!(matches!(event, AgentStreamEvent::ToolCallBegin { .. }));
    }
}

#[tokio::test]
async fn test_sequential_on_event_preserves_exact_order() {
    let observer = ChannelledEventObserver::new();

    // Send events sequentially with small delays
    let events_in: Vec<AgentStreamEvent> = vec![
        AgentStreamEvent::AgentStart { input: "1".to_string() },
        AgentStreamEvent::ThinkingComplete { thinking: "2".to_string() },
        AgentStreamEvent::ToolCallBegin { tool_call_id: "3".to_string(), tool_name: "3".to_string(), arguments: "".to_string() },
        AgentStreamEvent::ToolCallComplete { tool_call_id: "4".to_string(), tool_name: "4".to_string(), result: "".to_string() },
        AgentStreamEvent::IterationComplete { iteration: 5, tool_calls: vec![], final_answer: None },
        AgentStreamEvent::AgentComplete,
    ];

    for event in &events_in {
        observer.on_event(event).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // Wait and shutdown
    observer.on_complete().await.unwrap();

    let events_out = observer.events().await;
    assert_eq!(events_out.len(), 6);

    // Verify exact order
    assert!(matches!(events_out[0], AgentStreamEvent::AgentStart { .. }));
    assert!(matches!(events_out[1], AgentStreamEvent::ThinkingComplete { .. }));
    assert!(matches!(events_out[2], AgentStreamEvent::ToolCallBegin { .. }));
    assert!(matches!(events_out[3], AgentStreamEvent::ToolCallComplete { .. }));
    assert!(matches!(events_out[4], AgentStreamEvent::IterationComplete { .. }));
    assert!(matches!(events_out[5], AgentStreamEvent::AgentComplete));
}

#[tokio::test]
async fn test_rapid_sequential_events() {
    let observer = ChannelledEventObserver::new();

    // Send 100 events with no delay between them
    for i in 0..100 {
        let event = AgentStreamEvent::ToolCallComplete {
            tool_call_id: format!("{}", i),
            tool_name: format!("tool"),
            result: format!("result_{}", i),
        };
        observer.on_event(&event).await.unwrap();
    }

    // Wait for consumer to process
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let events = observer.events().await;
    assert_eq!(events.len(), 100);

    // Verify order is preserved
    for (i, event) in events.iter().enumerate() {
        if let AgentStreamEvent::ToolCallComplete { result, .. } = event {
            assert_eq!(result, &format!("result_{}", i));
        } else {
            panic!("Expected ToolCallComplete event at index {}", i);
        }
    }
}
