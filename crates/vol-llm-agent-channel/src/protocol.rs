use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Unified message type for all agent communication.
///
/// Direction is determined by `sender` and `receiver` fields, not by the type name.
/// The same message can be both received and sent on any connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Submit {
        req_id: String,
        sender: String,
        receiver: String,
        input: String,
        #[serde(default)]
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    Cancel {
        req_id: String,
        sender: String,
        receiver: String,
    },
    Connected {
        sender: String,
        receiver: String,
    },
    Event {
        sender: String,
        receiver: String,
        event: serde_json::Value,
    },
    Result {
        req_id: String,
        sender: String,
        receiver: String,
        result: serde_json::Value,
    },
    Error {
        req_id: Option<String>,
        sender: String,
        receiver: String,
        message: String,
    },
}
