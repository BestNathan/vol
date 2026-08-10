use vol_agent_server::data_plane::DataPlaneServerCore;
use vol_llm_agent::AgentInput;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, MessageKind, Operation, Payload,
};

#[tokio::test]
async fn submit_emits_single_result_with_enriched_metadata() {
    let core = DataPlaneServerCore::for_test().await;

    // Ground truth from the registered test agent instance (TestLlm in for_test).
    let agent = core
        .router()
        .get_agent("test_agent")
        .await
        .expect("test_agent registered by for_test");
    let expected_tools: Vec<String> = agent
        .tools()
        .definitions()
        .iter()
        .map(|d| d.name.clone())
        .collect();
    let expected_mcps: Vec<String> = agent.mcps().server_status().keys().cloned().collect();
    let expected_skills: Vec<String> = agent.skills().skill_names().await;

    let msg = AgentServerMessage::new_command(
        "msg_submit_1",
        Operation::Agent(AgentOperation::Submit),
        Payload::Agent(AgentPayload::Submit {
            input: AgentInput::text("hello world").with_run_id("run_supplied_1"),
            target: None,
        }),
    );

    let outputs = core.handle(msg).await.unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].kind, MessageKind::Result);
    assert_eq!(outputs[0].message_id, "msg_submit_1");

    match &outputs[0].payload {
        Payload::Agent(AgentPayload::SubmitResult {
            run_id,
            accepted,
            provider,
            tools,
            mcps,
            skills,
        }) => {
            assert!(*accepted);
            assert_eq!(run_id, "run_supplied_1");
            // Resolved from the test agent instance (TestLlm registered by for_test)
            assert_eq!(provider.name, "anthropic");
            assert_eq!(provider.model, "test");
            assert_eq!(tools, &expected_tools);
            assert_eq!(mcps, &expected_mcps);
            assert_eq!(skills, &expected_skills);
        }
        other => panic!("expected SubmitResult payload, got {other:?}"),
    }
}

#[tokio::test]
async fn cancel_returns_result_with_cancelled_flag() {
    let core = DataPlaneServerCore::for_test().await;
    let msg = AgentServerMessage::new_command(
        "msg_cancel_1",
        Operation::Agent(AgentOperation::Cancel),
        Payload::Agent(AgentPayload::Cancel {
            run_id: "run_target_123".to_string(),
        }),
    );

    let outputs = core.handle(msg).await.unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].kind, MessageKind::Result);
    assert_eq!(outputs[0].message_id, "msg_cancel_1");

    match &outputs[0].payload {
        Payload::Agent(AgentPayload::CancelResult { run_id, cancelled }) => {
            assert!(!run_id.is_empty());
            assert!(!cancelled);
        }
        other => panic!("expected CancelResult payload, got {other:?}"),
    }
}
