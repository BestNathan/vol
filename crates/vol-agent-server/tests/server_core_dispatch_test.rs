use vol_agent_server::data_plane::DataPlaneServerCore;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, FileOperation, FilePayload, Operation, Payload,
};

#[tokio::test]
async fn core_dispatches_file_read_to_file_domain() {
    let core = DataPlaneServerCore::for_test().await;
    let msg = AgentServerMessage::new_command(
        "msg_1",
        Operation::File(FileOperation::Read),
        Payload::File(FilePayload::Read {
            path: "Cargo.toml".to_string(),
        }),
    );

    let outputs = core.handle(msg).await.unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].operation.method_name(), "file.read");
}
