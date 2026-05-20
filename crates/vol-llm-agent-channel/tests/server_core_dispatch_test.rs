use vol_llm_agent_channel::agent_server_protocol::{
    AgentServerMessage, FileOperation, FilePayload, Operation, Payload,
};
use vol_llm_agent_channel::server_core::AgentServerCore;

#[tokio::test]
async fn core_dispatches_file_read_to_file_domain() {
    let core = AgentServerCore::for_test().await;
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
