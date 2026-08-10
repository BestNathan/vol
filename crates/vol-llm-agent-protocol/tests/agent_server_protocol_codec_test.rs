//! Integration tests for agent server protocol types and operation codec.

use vol_llm_agent::AgentInput;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, FileOperation, Operation, Payload,
};
use vol_llm_agent_protocol::operation_codec::{decode_payload, method_to_operation};

#[test]
fn agent_server_protocol_codec_test_method_round_trip_agent_submit() {
    let op = method_to_operation("agent.submit").unwrap();
    assert_eq!(op, Operation::Agent(AgentOperation::Submit));
    assert_eq!(op.method_name(), "agent.submit");
}

#[test]
fn agent_server_protocol_codec_test_method_round_trip_file_list() {
    let op = method_to_operation("file.list").unwrap();
    assert_eq!(op, Operation::File(FileOperation::List));
    assert_eq!(op.method_name(), "file.list");
}

#[test]
fn agent_server_protocol_codec_test_unknown_method_error() {
    let err = method_to_operation("unknown.foo").unwrap_err();
    assert!(err.to_string().contains("unknown method"));
}

#[test]
fn agent_server_protocol_codec_test_decode_payload_rejects_wrong_shape() {
    let op = Operation::File(FileOperation::List);
    let err = decode_payload(op, serde_json::json!({"run_id": "run_1"})).unwrap_err();
    assert!(err.to_string().contains("file.list"));
}

#[test]
fn agent_server_protocol_codec_test_decode_agent_submit_accepts_supplied_run_id() {
    let payload = decode_payload(
        Operation::Agent(AgentOperation::Submit),
        serde_json::json!({
            "input": "hello",
            "target": "agent",
            "run_id": "run_supplied_1"
        }),
    )
    .unwrap();

    assert_eq!(
        payload,
        Payload::Agent(AgentPayload::Submit {
            input: AgentInput::text("hello"),
            target: Some("agent".to_string()),
        })
    );
}

#[test]
fn agent_server_protocol_codec_test_decode_agent_submit_defaults_missing_run_id() {
    let payload = decode_payload(
        Operation::Agent(AgentOperation::Submit),
        serde_json::json!({
            "input": "hello",
            "target": "agent"
        }),
    )
    .unwrap();

    assert_eq!(
        payload,
        Payload::Agent(AgentPayload::Submit {
            input: AgentInput::text("hello"),
            target: Some("agent".to_string()),
        })
    );
}

#[test]
fn agent_server_protocol_codec_test_decode_agent_cancel_uses_run_id() {
    let payload = decode_payload(
        Operation::Agent(AgentOperation::Cancel),
        serde_json::json!({"run_id": "run_123"}),
    )
    .unwrap();

    assert_eq!(
        payload,
        Payload::Agent(AgentPayload::Cancel {
            run_id: "run_123".to_string(),
        })
    );
}

#[test]
fn agent_server_protocol_codec_test_message_id_reused_across_submit_result_not_equal_run_id() {
    let submit = AgentServerMessage::new_command(
        "msg_1",
        Operation::Agent(AgentOperation::Submit),
        Payload::Agent(AgentPayload::Submit {
            input: AgentInput::text("hello"),
            target: None,
        }),
    );

    // agent.submit now answers with a single SubmitResult (merged Ack+Result).
    let result = AgentServerMessage::new_result(
        "msg_1",
        Operation::Agent(AgentOperation::Submit),
        Payload::Agent(AgentPayload::SubmitResult {
            run_id: "run_abc".to_string(),
            accepted: true,
            provider: vol_llm_agent_protocol::agent_server_protocol::ProviderInfo {
                name: "anthropic".to_string(),
                model: "claude-sonnet-5".to_string(),
            },
            tools: vec![],
            mcps: vec![],
            skills: vec![],
        }),
    );

    assert_eq!(submit.message_id, result.message_id);
    assert_eq!(
        result.kind,
        vol_llm_agent_protocol::agent_server_protocol::MessageKind::Result
    );
    assert_ne!(submit.message_id.as_str(), "run_abc");
}

#[test]
fn agent_server_protocol_codec_test_submit_result_flat_wire_shape() {
    // The new SubmitResult shape carries provider info plus the resolved
    // tool/MCP/skill capability lists — verify it encodes to the flat wire
    // format (no variant wrapper) that the server and frontend consume.
    let result = Payload::Agent(AgentPayload::SubmitResult {
        run_id: "run_xyz".to_string(),
        accepted: true,
        provider: vol_llm_agent_protocol::agent_server_protocol::ProviderInfo {
            name: "openai".to_string(),
            model: "gpt-4o".to_string(),
        },
        tools: vec!["bash".to_string(), "read".to_string()],
        mcps: vec!["k8s".to_string()],
        skills: vec!["code-review".to_string()],
    });

    assert_eq!(
        result.data_json(),
        serde_json::json!({
            "run_id": "run_xyz",
            "accepted": true,
            "provider": {"name": "openai", "model": "gpt-4o"},
            "tools": ["bash", "read"],
            "mcps": ["k8s"],
            "skills": ["code-review"]
        })
    );
}
