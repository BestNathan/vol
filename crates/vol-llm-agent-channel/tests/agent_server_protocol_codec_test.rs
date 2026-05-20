//! Integration tests for agent server protocol types and operation codec.

use vol_llm_agent_channel::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, FileOperation, Operation, Payload,
};
use vol_llm_agent_channel::operation_codec::{decode_payload, method_to_operation};

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
fn agent_server_protocol_codec_test_message_id_reused_across_submit_ack_not_equal_run_id() {
    let submit = AgentServerMessage::new_command(
        "msg_1",
        Operation::Agent(AgentOperation::Submit),
        Payload::Agent(AgentPayload::Submit {
            input: "hello".to_string(),
            target: None,
            metadata: None,
        }),
    );

    let ack = AgentServerMessage::new_ack(
        "msg_1",
        Operation::Agent(AgentOperation::Submit),
        Payload::Agent(AgentPayload::SubmitAck {
            run_id: "run_abc".to_string(),
            accepted: true,
        }),
    );

    assert_eq!(submit.message_id, ack.message_id);
    assert_ne!(submit.message_id.as_str(), "run_abc");
}
