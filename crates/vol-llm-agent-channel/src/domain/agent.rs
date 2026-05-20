use std::sync::Arc;

use crate::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, Operation, Payload, ProtocolError,
};
use crate::connection::ConnectionHolder;
use crate::router::AgentRouter;

/// Handler for agent-domain operations.
pub struct AgentHandler {
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<std::collections::HashMap<String, Arc<ConnectionHolder>>>>,
}

impl AgentHandler {
    pub fn new(
        router: AgentRouter,
        holders: Arc<std::sync::Mutex<std::collections::HashMap<String, Arc<ConnectionHolder>>>>,
    ) -> Self {
        Self { router, holders }
    }

    pub async fn handle(
        &self,
        operation: AgentOperation,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (operation, message.payload) {
            (AgentOperation::Submit, Payload::Agent(AgentPayload::Submit { .. })) => {
                let run_id = uuid::Uuid::new_v4().to_string();
                Ok(vec![
                    AgentServerMessage::new_ack(
                        message.message_id.clone(),
                        Operation::Agent(AgentOperation::Submit),
                        Payload::Agent(AgentPayload::SubmitAck {
                            run_id: run_id.clone(),
                            accepted: true,
                        }),
                    ),
                    AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Agent(AgentOperation::Submit),
                        Payload::Agent(AgentPayload::SubmitResult {
                            run_id,
                            response: serde_json::json!({ "output": "" }),
                        }),
                    ),
                ])
            }
            (AgentOperation::Cancel, Payload::Agent(AgentPayload::Cancel { run_id })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Cancel),
                    Payload::Agent(AgentPayload::CancelResult {
                        run_id,
                        cancelled: false,
                    }),
                ),
            ]),
            (AgentOperation::Subscribe, Payload::Agent(AgentPayload::Subscribe { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Subscribe),
                    Payload::Agent(AgentPayload::SubscribeResult {
                        subscription_id: uuid::Uuid::new_v4().to_string(),
                    }),
                ),
            ]),
            (AgentOperation::Unsubscribe, Payload::Agent(AgentPayload::Unsubscribe { subscription_id })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Unsubscribe),
                    Payload::Agent(AgentPayload::UnsubscribeResult {
                        subscription_id,
                        removed: true,
                    }),
                ),
            ]),
            (AgentOperation::Approve, Payload::Agent(AgentPayload::Approve { run_id, .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Approve),
                    Payload::Agent(AgentPayload::ApproveResult {
                        run_id,
                        accepted: true,
                    }),
                ),
            ]),
            (AgentOperation::List, _) => {
                let agents: Vec<serde_json::Value> = self
                    .holders
                    .lock()
                    .unwrap()
                    .keys()
                    .map(|k| serde_json::json!({ "id": k, "name": k }))
                    .collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::List),
                    Payload::Agent(AgentPayload::ListResult { agents }),
                )])
            }
            (AgentOperation::Event, Payload::Agent(AgentPayload::Event { run_id, event })) => Ok(vec![
                AgentServerMessage::new_event(
                    message.message_id,
                    Operation::Agent(AgentOperation::Event),
                    Payload::Agent(AgentPayload::Event { run_id, event }),
                ),
            ]),
            (AgentOperation::Submit, _) => Err(ProtocolError::PayloadDecodeFailed("agent.submit")),
            (AgentOperation::Cancel, _) => Err(ProtocolError::PayloadDecodeFailed("agent.cancel")),
            (AgentOperation::Subscribe, _) => Err(ProtocolError::PayloadDecodeFailed("agent.subscribe")),
            (AgentOperation::Unsubscribe, _) => Err(ProtocolError::PayloadDecodeFailed("agent.unsubscribe")),
            (AgentOperation::Approve, _) => Err(ProtocolError::PayloadDecodeFailed("agent.approve")),
            (AgentOperation::Event, _) => Err(ProtocolError::PayloadDecodeFailed("agent.event")),
        }
    }
}
