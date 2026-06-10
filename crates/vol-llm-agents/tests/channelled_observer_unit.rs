//! Unit tests for ChannelledEventObserver

use vol_llm_agents::coding::{ChannelledEventObserver, EventObserver};
use vol_llm_core::AgentStreamEvent;

#[tokio::test]
async fn test_channelled_observer_new_creates_empty_events() {
    let observer = ChannelledEventObserver::new();
    let events = observer.events().await;
    assert!(events.is_empty());
}

#[tokio::test]
async fn test_channelled_observer_on_event_records_event() {
    let observer = ChannelledEventObserver::new();

    let event = AgentStreamEvent::AgentStart {
        input: "test task".to_string(),
        timestamp: chrono::Utc::now(),
    };

    observer.on_event(&event).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let events = observer.events().await;
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], AgentStreamEvent::AgentStart { .. }));
}

#[tokio::test]
async fn test_channelled_observer_preserves_order() {
    let observer = ChannelledEventObserver::new();

    let ts = chrono::Utc::now();
    let events_in: Vec<AgentStreamEvent> = vec![
        AgentStreamEvent::AgentStart {
            input: "start".to_string(),
            timestamp: ts,
        },
        AgentStreamEvent::ThinkingComplete {
            thinking: "thinking".to_string(),
            timestamp: ts,
        },
        AgentStreamEvent::ToolCallBegin {
            tool_call_id: "1".to_string(),
            tool_name: "test".to_string(),
            arguments: "{}".to_string(),
            timestamp: ts,
        },
        AgentStreamEvent::ToolCallComplete {
            tool_call_id: "1".to_string(),
            tool_name: "test".to_string(),
            result: "ok".to_string(),
            timestamp: ts,
            duration_ms: None,
        },
        AgentStreamEvent::AgentComplete {
            timestamp: ts,
            response: None,
        },
    ];

    for event in &events_in {
        observer.on_event(event).await.unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let events_out = observer.events().await;
    assert_eq!(events_out.len(), 5);

    assert!(matches!(events_out[0], AgentStreamEvent::AgentStart { .. }));
    assert!(matches!(
        events_out[1],
        AgentStreamEvent::ThinkingComplete { .. }
    ));
    assert!(matches!(
        events_out[2],
        AgentStreamEvent::ToolCallBegin { .. }
    ));
    assert!(matches!(
        events_out[3],
        AgentStreamEvent::ToolCallComplete { .. }
    ));
    assert!(matches!(
        events_out[4],
        AgentStreamEvent::AgentComplete { .. }
    ));
}

#[tokio::test]
async fn test_channelled_observer_on_complete_waits() {
    let observer = ChannelledEventObserver::new();

    let event = AgentStreamEvent::AgentStart {
        input: "test".to_string(),
        timestamp: chrono::Utc::now(),
    };
    observer.on_event(&event).await.unwrap();

    observer.on_complete().await.unwrap();

    let events = observer.events().await;
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn test_channelled_observer_handles_many_events() {
    let observer = ChannelledEventObserver::new();

    for i in 0..50 {
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: chrono::Utc::now(),
            tool_call_id: i.to_string(),
            tool_name: format!("tool_{}", i),
            arguments: format!("arg_{}", i),
        };
        observer.on_event(&event).await.unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let events = observer.events().await;
    assert_eq!(events.len(), 50);

    for (i, event) in events.iter().enumerate() {
        if let AgentStreamEvent::ToolCallBegin {
            tool_name,
            arguments,
            ..
        } = event
        {
            assert_eq!(tool_name, &format!("tool_{}", i));
            assert_eq!(arguments, &format!("arg_{}", i));
        } else {
            panic!("Expected ToolCallBegin event");
        }
    }
}
