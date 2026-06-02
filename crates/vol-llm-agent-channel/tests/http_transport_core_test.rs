use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::util::ServiceExt;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentServerMessage, FileOperation, FilePayload, MessageKind, Operation, Payload,
};
use vol_llm_agent_channel::server_core::AgentServerCore;
use vol_llm_agent_channel::transport::HttpTransport;

#[tokio::test]
async fn http_transport_posts_protocol_message_to_core() {
    let core = std::sync::Arc::new(AgentServerCore::for_test().await);
    let app = HttpTransport::new(core).into_axum_router();
    let command = AgentServerMessage::new_command(
        "msg_http_file_read",
        Operation::File(FileOperation::Read),
        Payload::File(FilePayload::Read {
            path: "Cargo.toml".to_string(),
        }),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&command).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let messages: Vec<AgentServerMessage> = serde_json::from_slice(&body).unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id, "msg_http_file_read");
    assert_eq!(messages[0].kind, MessageKind::Result);
    assert_eq!(messages[0].operation, Operation::File(FileOperation::Read));
}

#[tokio::test]
async fn http_transport_invalid_json_returns_bad_request() {
    let core = std::sync::Arc::new(AgentServerCore::for_test().await);
    let app = HttpTransport::new(core).into_axum_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from("not json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
