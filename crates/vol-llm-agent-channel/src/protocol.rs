// crates/vol-llm-agent-channel/src/protocol.rs

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Messages received from client (inbound).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InboundMessage {
    Submit {
        req_id: String,
        target_id: String,
        input: String,
        #[serde(default)]
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    Cancel {
        req_id: String,
    },
}

/// Messages sent to client (outbound).
/// Note: OutboundMessage is only serialized (never deserialized),
/// so it only derives Serialize.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundMessage {
    Connected {
        agent_id: String,
    },
    Event {
        event: serde_json::Value,
    },
    Result {
        result: serde_json::Value,
    },
    Error {
        req_id: Option<String>,
        message: String,
    },
}
