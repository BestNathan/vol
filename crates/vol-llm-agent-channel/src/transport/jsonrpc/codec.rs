//! JSON-RPC WebSocket gateway codec.

use crate::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, ErrorPayload, FileOperation, MessageKind, Operation, Payload,
};
use crate::error::ConnectionError;
use crate::operation_codec::{decode_payload, method_to_operation};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    jsonrpc: Option<String>,
    id: Option<serde_json::Value>,
    method: Option<String>,
    params: Option<serde_json::Value>,
}

pub fn decode_jsonrpc_frame(text: &str) -> Result<AgentServerMessage, ConnectionError> {
    let envelope: JsonRpcEnvelope = serde_json::from_str(text)
        .map_err(|e| ConnectionError::ParseError(format!("invalid JSON: {e}")))?;

    if envelope.jsonrpc.as_deref() != Some("2.0") {
        return Err(ConnectionError::ParseError(
            "missing or invalid jsonrpc field".into(),
        ));
    }

    let message_id = match envelope.id {
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::String(s)) => s,
        Some(_) => return Err(ConnectionError::ParseError("unsupported id type".into())),
        None => return Err(ConnectionError::ParseError("missing id".into())),
    };

    let method = envelope
        .method
        .ok_or_else(|| ConnectionError::ParseError("missing method".into()))?;
    let operation = method_to_operation(&method)
        .map_err(|e| ConnectionError::ParseError(e.to_string()))?;
    let payload = decode_payload(operation.clone(), envelope.params.unwrap_or(serde_json::json!({})))
        .map_err(|e| ConnectionError::ParseError(e.to_string()))?;

    Ok(AgentServerMessage {
        protocol: "agent-server/1".to_string(),
        message_id,
        sender: "client".to_string(),
        receiver: "server".to_string(),
        kind: MessageKind::Command,
        operation,
        payload,
        meta: Default::default(),
    })
}

pub fn encode_jsonrpc_message(msg: AgentServerMessage) -> Result<String, ConnectionError> {
    match msg.kind {
        MessageKind::Ack | MessageKind::Result => {
            let id = parse_message_id_for_jsonrpc(&msg.message_id);
            let result = msg.payload.data_json();
            serde_json::to_string(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result,
            }))
            .map_err(|e| ConnectionError::ParseError(format!("serialization error: {e}")))
        }
        MessageKind::Event => {
            let params = msg.payload.data_json();
            serde_json::to_string(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": msg.operation.method_name(),
                "params": params,
            }))
            .map_err(|e| ConnectionError::ParseError(format!("serialization error: {e}")))
        }
        MessageKind::Error => {
            let id = parse_message_id_for_jsonrpc(&msg.message_id);
            let error = match msg.payload {
                Payload::Error(err) => serde_json::to_value(err)
                    .map_err(|e| ConnectionError::ParseError(format!("serialization error: {e}")))?,
                _ => serde_json::to_value(ErrorPayload {
                    code: "internal_error".to_string(),
                    message: "error message missing error payload".to_string(),
                    detail: None,
                    terminal: true,
                })
                .map_err(|e| ConnectionError::ParseError(format!("serialization error: {e}")))?,
            };
            serde_json::to_string(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": error,
            }))
            .map_err(|e| ConnectionError::ParseError(format!("serialization error: {e}")))
        }
        MessageKind::Command => {
            let id = parse_message_id_for_jsonrpc(&msg.message_id);
            let params = msg.payload.data_json();
            serde_json::to_string(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": msg.operation.method_name(),
                "params": params,
            }))
            .map_err(|e| ConnectionError::ParseError(format!("serialization error: {e}")))
        }
    }
}

fn parse_message_id_for_jsonrpc(message_id: &str) -> serde_json::Value {
    if let Ok(i) = message_id.parse::<i64>() {
        serde_json::Value::Number(i.into())
    } else {
        serde_json::Value::String(message_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_agent_submit_maps_id_to_message_id() {
        let msg = decode_jsonrpc_frame(
            r#"{"jsonrpc":"2.0","id":42,"method":"agent.submit","params":{"input":"hello","target":"coding"}}"#,
        )
        .unwrap();

        assert_eq!(msg.message_id, "42");
        assert_eq!(msg.kind, MessageKind::Command);
        assert_eq!(msg.operation, Operation::Agent(AgentOperation::Submit));
    }

    #[test]
    fn decode_file_list_operation() {
        let msg = decode_jsonrpc_frame(
            r#"{"jsonrpc":"2.0","id":7,"method":"file.list","params":{"path":"/tmp"}}"#,
        )
        .unwrap();

        assert_eq!(msg.message_id, "7");
        assert_eq!(msg.operation, Operation::File(FileOperation::List));
    }

    #[test]
    fn encode_result_maps_message_id_back_to_jsonrpc_id() {
        let out = encode_jsonrpc_message(AgentServerMessage::new_result(
            "42",
            Operation::Agent(AgentOperation::Submit),
            Payload::Agent(AgentPayload::SubmitResult {
                run_id: "run_1".to_string(),
                response: serde_json::json!({"agents": []}),
            }),
        ))
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert!(parsed.get("result").is_some());
    }
}
