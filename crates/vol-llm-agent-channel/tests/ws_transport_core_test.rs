use axum::Router;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentServerMessage, FileOperation, FilePayload, MessageKind, Operation, Payload,
};
use vol_llm_agent_channel::server_core::AgentServerCore;
use vol_llm_agent_channel::transport::WsServer;

async fn spawn_ws_app(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("ws://{addr}/ws")
}

#[tokio::test]
async fn ws_transport_forwards_protocol_messages_to_core() {
    let core = std::sync::Arc::new(AgentServerCore::for_test().await);
    let app = WsServer::new(core).into_axum_router();
    let url = spawn_ws_app(app).await;

    let (mut socket, _) = connect_async(url).await.unwrap();
    let command = AgentServerMessage::new_command(
        "msg_ws_file_read",
        Operation::File(FileOperation::Read),
        Payload::File(FilePayload::Read {
            path: "Cargo.toml".to_string(),
        }),
    );

    socket
        .send(TungsteniteMessage::Text(
            serde_json::to_string(&command).unwrap(),
        ))
        .await
        .unwrap();

    let frame = socket.next().await.unwrap().unwrap();
    let text = frame.into_text().unwrap();
    let response: AgentServerMessage = serde_json::from_str(&text).unwrap();

    assert_eq!(response.message_id, "msg_ws_file_read");
    assert_eq!(response.kind, MessageKind::Result);
    assert_eq!(response.operation, Operation::File(FileOperation::Read));
}
