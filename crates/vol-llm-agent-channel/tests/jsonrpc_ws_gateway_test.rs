use axum::extract::ws::WebSocketUpgrade;
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, ErrorPayload, FileOperation, FilePayload,
    MessageKind, Operation, Payload,
};
use vol_llm_agent_channel::connection::Connection;
use vol_llm_agent_channel::transport::jsonrpc::codec::{
    decode_jsonrpc_frame, encode_jsonrpc_message,
};
use vol_llm_agent_channel::transport::jsonrpc::connection::JsonRpcConnection;

#[test]
fn decode_agent_submit_maps_jsonrpc_id_to_message_id() {
    let msg = decode_jsonrpc_frame(
        r#"{"jsonrpc":"2.0","id":42,"method":"agent.submit","params":{"input":"hello","target":"coding"}}"#,
    )
    .unwrap();

    assert_eq!(msg.message_id, "42");
    assert_eq!(msg.kind, MessageKind::Command);
    assert_eq!(msg.operation, Operation::Agent(AgentOperation::Submit));
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

#[test]
fn encode_event_uses_notification_format() {
    let out = encode_jsonrpc_message(AgentServerMessage::new_event(
        "msg_1",
        Operation::Agent(AgentOperation::Event),
        Payload::Agent(AgentPayload::Event {
            run_id: "run_abc".to_string(),
            event: serde_json::json!({"type": "agent_start"}),
        }),
    ))
    .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(parsed.get("id").is_none());
    assert_eq!(parsed["method"], "agent.event");
}

#[test]
fn encode_error_uses_error_response_format() {
    let out = encode_jsonrpc_message(AgentServerMessage::new_error(
        "5",
        Operation::Agent(AgentOperation::Submit),
        ErrorPayload {
            code: "invalid_request".to_string(),
            message: "invalid request".to_string(),
            detail: Some(serde_json::json!({"code": -32600})),
            terminal: true,
        },
    ))
    .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["id"], 5);
    assert_eq!(parsed["error"]["message"], "invalid request");
}

#[test]
fn encode_ack_uses_result_format() {
    let out = encode_jsonrpc_message(AgentServerMessage::new_ack(
        "99",
        Operation::Agent(AgentOperation::Submit),
        Payload::Agent(AgentPayload::SubmitAck {
            run_id: "run_abc".to_string(),
            accepted: true,
        }),
    ))
    .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["id"], 99);
    assert!(parsed.get("result").is_some());
}

#[test]
fn decode_with_string_id_preserves_as_message_id() {
    let msg = decode_jsonrpc_frame(
        r#"{"jsonrpc":"2.0","id":"req-abc","method":"agent.submit","params":{"input":"hello"}}"#,
    )
    .unwrap();

    assert_eq!(msg.message_id, "req-abc");
}

#[test]
fn encode_string_message_id_produces_string_jsonrpc_id() {
    let out = encode_jsonrpc_message(AgentServerMessage::new_result(
        "req-abc",
        Operation::File(FileOperation::Read),
        Payload::File(FilePayload::ReadResult {
            content: "hello".to_string(),
            metadata: serde_json::json!({}),
        }),
    ))
    .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["id"], "req-abc");
}

#[test]
fn decode_file_list_maps_to_file_operation() {
    let msg = decode_jsonrpc_frame(
        r#"{"jsonrpc":"2.0","id":7,"method":"file.list","params":{"path":"/tmp"}}"#,
    )
    .unwrap();

    assert_eq!(msg.message_id, "7");
    assert_eq!(msg.operation, Operation::File(FileOperation::List));
}

#[test]
fn decode_invalid_json_returns_parse_error() {
    let err = decode_jsonrpc_frame("not json at all").unwrap_err();
    assert!(err.to_string().to_lowercase().contains("invalid json"));
}

#[test]
fn decode_unknown_method_returns_parse_error() {
    let err = decode_jsonrpc_frame(r#"{"jsonrpc":"2.0","id":1,"method":"foo.bar","params":{}}"#)
        .unwrap_err();
    assert!(err.to_string().contains("unknown method"));
}

async fn spawn_jsonrpc_connection_sender(msg: AgentServerMessage) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/ws",
        get(move |ws: WebSocketUpgrade| {
            let msg = msg.clone();
            async move {
                ws.on_upgrade(move |socket| async move {
                    let conn = JsonRpcConnection::new(socket);
                    conn.send(msg).await.unwrap();
                })
            }
        }),
    );

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("ws://{addr}/ws")
}

#[tokio::test]
async fn jsonrpc_connection_send_preserves_error_id_and_payload() {
    let error = AgentServerMessage::new_error(
        "5",
        Operation::Agent(AgentOperation::Submit),
        ErrorPayload {
            code: "session_not_found".to_string(),
            message: "session not found".to_string(),
            detail: Some(serde_json::json!({"session_id": "missing-session"})),
            terminal: true,
        },
    );
    let url = spawn_jsonrpc_connection_sender(error).await;

    let (mut socket, _) = connect_async(url).await.unwrap();
    let frame = socket.next().await.unwrap().unwrap();
    let text = match frame {
        TungsteniteMessage::Text(text) => text,
        other => panic!("expected text WebSocket frame, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 5);
    assert_eq!(parsed["error"]["code"], "session_not_found");
    assert_eq!(parsed["error"]["message"], "session not found");
    assert_eq!(parsed["error"]["detail"]["session_id"], "missing-session");
    assert_eq!(parsed["error"]["terminal"], true);
}
