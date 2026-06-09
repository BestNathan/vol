//! End-to-end test: exercise every operation through core.handle() directly,
//! verifying full handler registry dispatch for all 22 methods.

use vol_llm_agent::AgentInput;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, FileOperation, LogOperation, McpOperation,
    MessageKind, Operation, Payload, SessionOperation, SkillOperation,
};
use vol_llm_agent_channel::AgentServerCore;
use vol_llm_core::AgentDef;

fn command(id: &str, op: Operation, payload: Payload) -> AgentServerMessage {
    AgentServerMessage::new_command(id, op, payload)
}

#[tokio::test]
async fn session_domain_works_with_sqlite_session_store() {
    let temp = tempfile::tempdir().unwrap();
    let db_url = format!("sqlite://{}", temp.path().join("sessions.db").display());

    let core = match AgentServerCore::builder(temp.path(), temp.path())
        .with_session_store_config(Some(vol_llm_runtime::SessionStoreConfig {
            store_type: vol_llm_runtime::SessionStoreType::Database,
            url: Some(db_url),
        }))
        .build()
        .await
    {
        Ok(core) => core,
        Err(e) if e.contains("No LLM provider configured") => return,
        Err(e) => panic!("failed to build AgentServerCore: {e}"),
    };

    let manager = core.runtime.session_manager.clone();
    manager
        .entry_store_for_agent("alpha")
        .save(vol_session::SessionEntry::new_summary(
            "session-a".to_string(),
            "database summary".to_string(),
        ))
        .await
        .unwrap();

    let resp = core
        .handle(AgentServerMessage::new_command(
            "sqlite-session-list",
            Operation::Session(SessionOperation::List),
            Payload::Session(
                vol_llm_agent_channel::agent_server_protocol::SessionPayload::List {
                    agent_id: Some("alpha".to_string()),
                },
            ),
        ))
        .await
        .unwrap();

    assert_eq!(resp.len(), 1);
    assert_eq!(resp[0].kind, MessageKind::Result);
    let Payload::Session(
        vol_llm_agent_channel::agent_server_protocol::SessionPayload::ListResult { sessions },
    ) = &resp[0].payload
    else {
        panic!("expected session list result, got {:?}", resp[0].payload);
    };
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["agent_id"], "alpha");
    assert_eq!(sessions[0]["session_id"], "session-a");
    assert_eq!(sessions[0]["entry_count"], 1);
}

#[tokio::test]
async fn register_agent_uses_configured_sqlite_session_manager() {
    let temp = tempfile::tempdir().unwrap();
    let db_url = format!("sqlite://{}", temp.path().join("sessions.db").display());

    let core = match AgentServerCore::builder(temp.path(), temp.path())
        .with_session_store_config(Some(vol_llm_runtime::SessionStoreConfig {
            store_type: vol_llm_runtime::SessionStoreType::Database,
            url: Some(db_url),
        }))
        .build()
        .await
    {
        Ok(core) => core,
        Err(e) if e.contains("No LLM provider configured") => return,
        Err(e) => panic!("failed to build AgentServerCore: {e}"),
    };

    let def = AgentDef::new("registered-agent", "You are a test agent.").with_type("test-agent");
    core.register_agent("registered-agent", def).await.unwrap();

    let agent = core
        .router()
        .get_agent("registered-agent")
        .await
        .expect("registered agent should be routed");
    let session_id = agent.session().id.clone();
    agent
        .session()
        .add_summary("summary written through registered agent session".to_string())
        .await
        .unwrap();

    let sessions = core
        .runtime
        .session_manager
        .list_sessions(Some("registered-agent"))
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].agent_id, "registered-agent");
    assert_eq!(sessions[0].session_id, session_id);
    assert_eq!(sessions[0].entry_count, 1);
}

#[tokio::test]
async fn test_e2e_all_methods() {
    let core = AgentServerCore::for_test().await;
    let def = AgentDef::new("test-agent", "You are a test agent.").with_type("test-agent");
    core.register_agent("test-agent", def).await.unwrap();

    let handle = |msg| core.handle(msg);

    // ── 1. agent.list ──
    let resp = handle(command(
        "1",
        Operation::Agent(AgentOperation::List),
        Payload::Agent(AgentPayload::ListResult { agents: vec![] }),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);
    assert_eq!(resp.len(), 1);

    // ── 2. agent.subscribe ──
    let resp = handle(command(
        "2",
        Operation::Agent(AgentOperation::Subscribe),
        Payload::Agent(AgentPayload::Subscribe { target: None }),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 3. agent.unsubscribe ──
    let resp = handle(command(
        "3",
        Operation::Agent(AgentOperation::Unsubscribe),
        Payload::Agent(AgentPayload::Unsubscribe {
            subscription_id: "sub_1".into(),
        }),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 4. agent.approve ──
    let resp = handle(command(
        "4",
        Operation::Agent(AgentOperation::Approve),
        Payload::Agent(AgentPayload::Approve {
            run_id: "r1".into(),
            approved: true,
            reason: None,
        }),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 5. agent.submit ──
    let resp = handle(command(
        "5",
        Operation::Agent(AgentOperation::Submit),
        Payload::Agent(AgentPayload::Submit {
            input: AgentInput::text("hello"),
            target: Some("test-agent".into()),
        }),
    ))
    .await
    .unwrap();
    assert_eq!(resp.len(), 2);
    assert_eq!(resp[0].kind, MessageKind::Ack);
    assert_eq!(resp[1].kind, MessageKind::Result);

    // ── 6. agent.cancel ──
    let resp = handle(command(
        "6",
        Operation::Agent(AgentOperation::Cancel),
        Payload::Agent(AgentPayload::Cancel {
            run_id: "nonexistent".into(),
        }),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 7. agent.event ──
    let resp = handle(command(
        "7",
        Operation::Agent(AgentOperation::Event),
        Payload::Agent(AgentPayload::Event {
            run_id: "r1".into(),
            event: serde_json::json!({"type": "thought"}),
        }),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].operation, Operation::Agent(AgentOperation::Event));

    // ── 8. file.list ──
    let resp = handle(command(
        "8",
        Operation::File(FileOperation::List),
        Payload::File(
            vol_llm_agent_channel::agent_server_protocol::FilePayload::List { path: ".".into() },
        ),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 9. file.read ──
    let resp = handle(command(
        "9",
        Operation::File(FileOperation::Read),
        Payload::File(
            vol_llm_agent_channel::agent_server_protocol::FilePayload::Read {
                path: "Cargo.toml".into(),
            },
        ),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 10. session.list ──
    let resp = handle(command(
        "10",
        Operation::Session(SessionOperation::List),
        Payload::Session(
            vol_llm_agent_channel::agent_server_protocol::SessionPayload::List { agent_id: None },
        ),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 11. session.resume (nonexistent → error message) ──
    let resp = handle(command(
        "11",
        Operation::Session(SessionOperation::Resume),
        Payload::Session(
            vol_llm_agent_channel::agent_server_protocol::SessionPayload::Resume {
                session_id: "n".into(),
                agent_id: None,
            },
        ),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Error);
    let Payload::Error(err) = &resp[0].payload else {
        panic!(
            "session.resume should return Error payload, got {:?}",
            resp[0].payload
        );
    };
    assert_eq!(err.code, "session_not_found");
    assert!(err.terminal);

    // ── 12. session.entries (nonexistent → error message) ──
    let resp = handle(command(
        "12",
        Operation::Session(SessionOperation::Entries),
        Payload::Session(
            vol_llm_agent_channel::agent_server_protocol::SessionPayload::Entries {
                session_id: "n".into(),
                agent_id: None,
            },
        ),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Error);
    let Payload::Error(err) = &resp[0].payload else {
        panic!(
            "session.entries should return Error payload, got {:?}",
            resp[0].payload
        );
    };
    assert_eq!(err.code, "session_not_found");
    assert!(err.terminal);

    // ── 13. log.list ──
    let resp = handle(command(
        "13",
        Operation::Log(LogOperation::List),
        Payload::Log(vol_llm_agent_channel::agent_server_protocol::LogPayload::List),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 14. log.read ──
    let resp = handle(command(
        "14",
        Operation::Log(LogOperation::Read),
        Payload::Log(
            vol_llm_agent_channel::agent_server_protocol::LogPayload::Read { run_id: "n".into() },
        ),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 15. skill.list ──
    let resp = handle(command(
        "15",
        Operation::Skill(SkillOperation::List),
        Payload::Skill(vol_llm_agent_channel::agent_server_protocol::SkillPayload::List),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);

    // ── 16. skill.get (nonexistent → error) ──
    let resp = handle(command(
        "16",
        Operation::Skill(SkillOperation::Get),
        Payload::Skill(
            vol_llm_agent_channel::agent_server_protocol::SkillPayload::Get { name: "n".into() },
        ),
    ))
    .await;
    if let Ok(messages) = resp {
        assert_eq!(
            messages[0].kind,
            MessageKind::Error,
            "skill.get for nonexistent should return Error"
        );
    }

    // ── 17-22. mcp.* (no MCP manager configured → Err) ──
    for (id, op, payload) in [
        (
            "17",
            Operation::Mcp(McpOperation::ListServers),
            Payload::Mcp(vol_llm_agent_channel::agent_server_protocol::McpPayload::ListServers),
        ),
        (
            "18",
            Operation::Mcp(McpOperation::ListTools),
            Payload::Mcp(
                vol_llm_agent_channel::agent_server_protocol::McpPayload::ListTools {
                    server: None,
                },
            ),
        ),
        (
            "19",
            Operation::Mcp(McpOperation::ListResources),
            Payload::Mcp(
                vol_llm_agent_channel::agent_server_protocol::McpPayload::ListResources {
                    server: None,
                },
            ),
        ),
        (
            "20",
            Operation::Mcp(McpOperation::ListResourceTemplates),
            Payload::Mcp(
                vol_llm_agent_channel::agent_server_protocol::McpPayload::ListResourceTemplates {
                    server: None,
                },
            ),
        ),
        (
            "21",
            Operation::Mcp(McpOperation::ListPrompts),
            Payload::Mcp(
                vol_llm_agent_channel::agent_server_protocol::McpPayload::ListPrompts {
                    server: None,
                },
            ),
        ),
        (
            "22",
            Operation::Mcp(McpOperation::ServerStatus),
            Payload::Mcp(
                vol_llm_agent_channel::agent_server_protocol::McpPayload::ServerStatus {
                    server: None,
                },
            ),
        ),
    ] {
        let resp = handle(command(id, op, payload)).await;
        assert!(
            resp.is_err(),
            "mcp {} should fail without MCP manager, got {:?}",
            id,
            resp
        );
    }

    // ── 23. system.connected ──
    let resp = handle(command(
        "23",
        Operation::System(vol_llm_agent_channel::agent_server_protocol::SystemOperation::Connected),
        Payload::System(vol_llm_agent_channel::agent_server_protocol::SystemPayload::Empty),
    ))
    .await
    .unwrap();
    assert_eq!(resp[0].kind, MessageKind::Result);
}
