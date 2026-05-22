use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use vol_llm_agent::AgentInput;

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
        input: AgentInput,
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

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::{AgentInput, InputPart};

    #[test]
    fn submit_accepts_legacy_string_input() {
        let message: Message = serde_json::from_str(
            r#"
        {
          "type": "submit",
          "req_id": "req-1",
          "sender": "client",
          "receiver": "agent",
          "input": "hello"
        }
        "#,
        )
        .unwrap();

        match message {
            Message::Submit { input, .. } => assert_eq!(input, AgentInput::text("hello")),
            other => panic!("expected submit message, got {other:?}"),
        }
    }

    #[test]
    fn submit_accepts_structured_input() {
        let message: Message = serde_json::from_str(
            r#"
        {
          "type": "submit",
          "req_id": "req-1",
          "sender": "client",
          "receiver": "agent",
          "input": {
            "run_id": "run-1",
            "parts": [
              { "type": "text", "text": "look" },
              { "type": "image_url", "url": "data:image/png;base64,AAAA" }
            ]
          }
        }
        "#,
        )
        .unwrap();

        match message {
            Message::Submit { input, .. } => {
                assert_eq!(input.run_id.as_deref(), Some("run-1"));
                assert!(matches!(input.parts[1], InputPart::ImageUrl { .. }));
            }
            other => panic!("expected submit message, got {other:?}"),
        }
    }
}
