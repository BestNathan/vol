use vol_llm_agent_channel::agent_server_protocol::{
    AgentServerMessage, FileOperation, FilePayload, LogOperation, LogPayload, Operation, Payload,
    SessionOperation, SessionPayload, SkillOperation, SkillPayload,
};
use vol_llm_agent_channel::server_core::AgentServerCore;

#[tokio::test]
async fn core_dispatches_file_read_to_file_domain() {
    let core = AgentServerCore::for_test();
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

#[tokio::test]
async fn core_dispatches_skill_get_to_skill_domain() {
    let core = AgentServerCore::for_test();
    let msg = AgentServerMessage::new_command(
        "msg_2",
        Operation::Skill(SkillOperation::Get),
        Payload::Skill(SkillPayload::Get {
            name: "test-skill".to_string(),
        }),
    );

    let outputs = core.handle(msg).await.unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].operation.method_name(), "skill.get");
}

#[tokio::test]
async fn core_dispatches_session_list_to_session_domain() {
    let core = AgentServerCore::for_test();
    let msg = AgentServerMessage::new_command(
        "msg_3",
        Operation::Session(SessionOperation::List),
        Payload::Session(SessionPayload::List),
    );

    let outputs = core.handle(msg).await.unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].operation.method_name(), "session.list");
}

#[tokio::test]
async fn core_dispatches_log_list_to_log_domain() {
    let core = AgentServerCore::for_test();
    let msg = AgentServerMessage::new_command(
        "msg_4",
        Operation::Log(LogOperation::List),
        Payload::Log(LogPayload::List),
    );

    let outputs = core.handle(msg).await.unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].operation.method_name(), "log.list");
}
