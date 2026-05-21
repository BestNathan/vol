use vol_llm_agent_channel::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, MessageKind, Operation, Payload,
};
use vol_llm_agent_channel::server_core::AgentServerCore;

#[tokio::test]
async fn submit_emits_ack_and_result_with_same_message_id() {
    let core = AgentServerCore::for_test().await;
    let msg = AgentServerMessage::new_command(
        "msg_submit_1",
        Operation::Agent(AgentOperation::Submit),
        Payload::Agent(AgentPayload::Submit {
            input: "hello world".to_string(),
            target: None,
            metadata: None,
            run_id: Some("run_supplied_1".to_string()),
        }),
    );

    let outputs = core.handle(msg).await.unwrap();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].kind, MessageKind::Ack);
    assert_eq!(outputs[1].kind, MessageKind::Result);
    assert_eq!(outputs[0].message_id, "msg_submit_1");
    assert_eq!(outputs[1].message_id, "msg_submit_1");

    let run_id = match &outputs[0].payload {
        Payload::Agent(AgentPayload::SubmitAck { run_id, accepted }) => {
            assert!(*accepted);
            assert_eq!(run_id, "run_supplied_1");
            run_id.clone()
        }
        other => panic!("expected SubmitAck payload, got {:?}", other),
    };

    match &outputs[1].payload {
        Payload::Agent(AgentPayload::SubmitResult {
            run_id: result_run_id,
            ..
        }) => {
            assert_eq!(result_run_id, &run_id);
        }
        other => panic!("expected SubmitResult payload, got {:?}", other),
    }
}

#[tokio::test]
async fn cancel_returns_result_with_cancelled_flag() {
    let core = AgentServerCore::for_test().await;
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
        other => panic!("expected CancelResult payload, got {:?}", other),
    }
}
