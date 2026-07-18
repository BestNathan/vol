//! JSON-RPC WebSocket gateway codec.

use crate::agent_server_protocol::{AgentServerMessage, ErrorPayload, MessageKind, Payload};
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

    let has_id = envelope.id.is_some();

    let method = envelope
        .method
        .ok_or_else(|| ConnectionError::ParseError("missing method".into()))?;
    let message_id = match envelope.id {
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::String(s)) => s,
        Some(_) => return Err(ConnectionError::ParseError("unsupported id type".into())),
        None => format!("notification:{method}"),
    };
    let operation =
        method_to_operation(&method).map_err(|e| ConnectionError::ParseError(e.to_string()))?;
    let payload = decode_payload(
        operation.clone(),
        envelope.params.unwrap_or(serde_json::json!({})),
    )
    .map_err(|e| ConnectionError::ParseError(e.to_string()))?;

    Ok(AgentServerMessage {
        protocol: "agent-server/1".to_string(),
        message_id,
        sender: "client".to_string(),
        receiver: "server".to_string(),
        kind: if has_id {
            MessageKind::Command
        } else {
            MessageKind::Event
        },
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
                Payload::Error(err) => serde_json::to_value(err).map_err(|e| {
                    ConnectionError::ParseError(format!("serialization error: {e}"))
                })?,
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
    use crate::agent_server_protocol::{
        AgentOperation, AgentPayload, ControlOperation, ControlPayload, FileOperation,
        NodeRegistration, Operation, RegisterAck,
    };

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
    fn decode_control_register() {
        let msg = decode_jsonrpc_frame(
            r#"{"jsonrpc":"2.0","id":"reg-1","method":"control.register","params":{"node_id":"node-a","name":"Node A","version":"0.1.0"}}"#,
        )
        .unwrap();

        assert_eq!(msg.message_id, "reg-1");
        assert_eq!(
            msg.operation,
            Operation::Control(ControlOperation::Register)
        );
        match msg.payload {
            Payload::Control(ControlPayload::Register(p)) => {
                assert_eq!(p.node_id, "node-a");
                assert_eq!(p.name, "Node A");
                assert_eq!(p.version, "0.1.0");
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn decode_control_heartbeat_notification() {
        let msg = decode_jsonrpc_frame(
            r#"{"jsonrpc":"2.0","id":"hb-1","method":"control.heartbeat","params":{"node_id":"node-a","status":"online","load":{"running":1,"queued":2}}}"#,
        )
        .unwrap();

        assert_eq!(
            msg.operation,
            Operation::Control(ControlOperation::Heartbeat)
        );
    }

    #[test]
    fn decode_control_heartbeat_without_id_as_notification() {
        let msg = decode_jsonrpc_frame(
            r#"{"jsonrpc":"2.0","method":"control.heartbeat","params":{"node_id":"node-a","status":"online","load":{"running":1,"queued":0}}}"#,
        )
        .unwrap();

        assert_eq!(msg.kind, MessageKind::Event);
        assert_eq!(
            msg.operation,
            Operation::Control(ControlOperation::Heartbeat)
        );
        assert!(msg.message_id.starts_with("notification:"));
    }

    #[test]
    fn encode_control_register_command_uses_flat_params() {
        let out = encode_jsonrpc_message(AgentServerMessage::new_command(
            "reg-1",
            Operation::Control(ControlOperation::Register),
            Payload::Control(ControlPayload::Register(NodeRegistration {
                node_id: "node-a".to_string(),
                name: "Node A".to_string(),
                version: "0.1.0".to_string(),
            })),
        ))
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["method"], "control.register");
        assert_eq!(parsed["params"]["node_id"], "node-a");
        assert_eq!(parsed["params"]["name"], "Node A");
        assert_eq!(parsed["params"]["version"], "0.1.0");
        assert!(parsed["params"].get("type").is_none());
        assert!(parsed["params"].get("data").is_none());
    }

    #[test]
    fn encode_control_register_ack_result_uses_flat_result() {
        let out = encode_jsonrpc_message(AgentServerMessage::new_result(
            "reg-1",
            Operation::Control(ControlOperation::Register),
            Payload::Control(ControlPayload::RegisterAck(RegisterAck {
                node_id: "node-a".to_string(),
                accepted: true,
                generation: 7,
            })),
        ))
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["id"], "reg-1");
        assert_eq!(parsed["result"]["node_id"], "node-a");
        assert_eq!(parsed["result"]["accepted"], true);
        assert_eq!(parsed["result"]["generation"], 7);
        assert!(parsed["result"].get("type").is_none());
        assert!(parsed["result"].get("data").is_none());
    }

    #[test]
    fn decode_rejects_missing_jsonrpc_field() {
        let err = decode_jsonrpc_frame(r#"{"id":1,"method":"test","params":{}}"#).unwrap_err();
        assert!(matches!(err, ConnectionError::ParseError(_)));
    }

    #[test]
    fn decode_rejects_unsupported_id_type() {
        let err = decode_jsonrpc_frame(
            r#"{"jsonrpc":"2.0","id":true,"method":"agent.submit","params":{}}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("unsupported id type"));
    }

    #[test]
    fn encode_error_with_non_error_payload_falls_back() {
        // When kind is Error but payload is not Payload::Error, it should fall back
        // to a synthetic ErrorPayload.
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "server".to_string(),
            kind: MessageKind::Error,
            operation: Operation::Agent(AgentOperation::List),
            payload: Payload::Agent(AgentPayload::ListResult { agents: vec![] }),
            meta: Default::default(),
        };
        let out = encode_jsonrpc_message(msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["error"]["code"], "internal_error");
        assert_eq!(parsed["error"]["terminal"], true);
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
