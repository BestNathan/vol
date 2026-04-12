# vol-session

Session management and message persistence for ReAct Agent.

## Features

- Session lifecycle management
- Message persistence with JSONL format
- Event-driven recording via SessionListener
- In-memory and file-based storage backends

## Installation

```toml
[dependencies]
vol-session = "0.1.0"
```

## Quick Start

### SessionListener Usage

The `SessionListener` subscribes to agent events and automatically records key events to a message store:

```rust
use vol_session::{SessionListener, InMemoryMessageStore, FileMessageStore};
use vol_llm_core::AgentStreamEvent;
use vol_tracing::TracedEvent;
use tokio::sync::broadcast;
use std::sync::Arc;

// Create a message store
let store = Arc::new(InMemoryMessageStore::new());

// Create event channel
let (tx, rx) = broadcast::channel(100);

// Create session listener
let session_id = "session-123".to_string();
let mut listener = SessionListener::new(rx, store.clone(), session_id.clone());

// Spawn listener task
let listener_handle = tokio::spawn(async move {
    listener.run().await.unwrap();
});

// Emit events (these will be automatically recorded)
tx.send(TracedEvent::without_span(AgentStreamEvent::ThinkingComplete {
    thinking: "Let me think about this...".to_string(),
})).unwrap();

tx.send(TracedEvent::without_span(AgentStreamEvent::ToolCallComplete {
    tool_name: "get_weather".to_string(),
    result: "25°C".to_string(),
})).unwrap();

// Drop sender to close channel
drop(tx);

// Wait for listener to complete
listener_handle.await.unwrap();

// Retrieve recorded messages
let messages = store.get_by_session(&session_id, 10).await.unwrap();
println!("Recorded {} messages", messages.len());
```

### FileMessageStore with JSONL Format

For persistent storage, use `FileMessageStore` which saves messages in JSONL format:

```rust
use vol_session::{FileMessageStore, SessionMessage, SessionListener};
use vol_llm_core::Message;
use std::sync::Arc;

// Create file-based message store
let store = Arc::new(FileMessageStore::new("/path/to/logs", "session-123"));

// Save a message
let session_msg = SessionMessage::new(
    "session-123".to_string(),
    Message::user("What is the weather?"),
);
store.save(session_msg).await.unwrap();

// Retrieve messages
let messages = store.get_by_session("session-123", 10).await.unwrap();
```

### JSONL File Format

Messages are stored in JSONL format (one JSON object per line):

```json
{"event":"SessionMessage","data":{"id":"uuid-1","session_id":"session-123","message":{"role":"assistant","content":"Let me think..."},"parent_id":null,"created_at":1712851200,"metadata":{}},"session_id":"session-123","timestamp":1712851200}
{"event":"SessionMessage","data":{"id":"uuid-2","session_id":"session-123","message":{"role":"system","content":"Tool 'get_weather' returned: 25°C"},"parent_id":null,"created_at":1712851201,"metadata":{}},"session_id":"session-123","timestamp":1712851201}
{"event":"SessionMessage","data":{"id":"uuid-3","session_id":"session-123","message":{"role":"assistant","content":"The weather is 25°C"},"parent_id":null,"created_at":1712851202,"metadata":{}},"session_id":"session-123","timestamp":1712851202}
```

Each line contains:
- `event`: Event type (always "SessionMessage" for now)
- `data`: Message data including id, session_id, message content, parent_id, created_at, metadata
- `session_id`: Session identifier
- `timestamp`: Unix timestamp

### Restoring Session from File

You can restore a complete conversation from a session file:

```rust
use vol_session::{FileMessageStore, Session};
use std::sync::Arc;

// Create file message store pointing to existing session file
let store = Arc::new(FileMessageStore::new("/path/to/logs", "session-123"));

// Load all messages from the session file
let messages = store.get_by_session("session-123", 1000).await.unwrap();

// Convert to format suitable for LLM conversation
let conversation_messages: Vec<vol_llm_core::Message> = messages
    .into_iter()
    .map(|session_msg| session_msg.message)
    .collect();

// Use in next conversation iteration
println!("Restored {} messages from session file", conversation_messages.len());
```

### Events Recorded by SessionListener

The `SessionListener` automatically records these events:

| Event Type | Message Role | Content |
|------------|--------------|---------|
| `ThinkingComplete` | Assistant | Thinking content |
| `ToolCallBegin` | Assistant | Tool call intent |
| `ToolCallComplete` | System | Tool result |
| `IterationComplete` (with final_answer) | Assistant | Final answer |

Events like `AgentStart`, `AgentStop`, and `IterationComplete` (without final_answer) are not recorded.

## Architecture

```
AgentStreamEvent (broadcast) 
       │
       ▼
SessionListener (filters & converts)
       │
       ▼
MessageStore (trait)
       │
       ├── InMemoryMessageStore
       │
       └── FileMessageStore (JSONL)
```

## License

MIT
