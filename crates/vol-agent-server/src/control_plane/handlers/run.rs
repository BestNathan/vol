use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, Operation, Payload, ProtocolError,
    RunStatusResult,
};
use vol_llm_agent_protocol::DomainHandler;

use crate::control_plane::core::make_result;
use crate::control_plane::state::ControlPlaneState;

pub struct RunHandler {
    state: Arc<ControlPlaneState>,
}

impl RunHandler {
    pub fn new(state: Arc<ControlPlaneState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl DomainHandler for RunHandler {
    fn name(&self) -> &str {
        "run"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![Operation::Control(ControlOperation::RunStatus)]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match message.payload.clone() {
            Payload::Control(ControlPayload::RunStatus(req)) => {
                let run = self.state.runs.get(&req.run_id);
                let result = match run {
                    Some(run) => RunStatusResult {
                        run_id: run.run_id,
                        status: run.status,
                        node_id: Some(run.node_id),
                    },
                    None => RunStatusResult {
                        run_id: req.run_id,
                        status: "not_found".to_string(),
                        node_id: None,
                    },
                };
                Ok(vec![make_result(
                    message,
                    ControlOperation::RunStatus,
                    ControlPayload::RunStatusResult(result),
                )])
            }
            _ => Err(ProtocolError::PayloadDecodeFailedOwned(
                "expected control.run_status payload".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, ControlOperation, ControlPayload, MessageKind, Operation, Payload,
        RunStatusRequest,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use crate::control_plane::handlers::run::RunHandler;
    use crate::control_plane::state::ControlPlaneState;
    use crate::control_plane::store::RunRecord;

    #[tokio::test]
    async fn run_status_returns_stored_run() {
        let state = Arc::new(ControlPlaneState::new());
        state.runs.insert(RunRecord {
            run_id: "run-1".to_string(),
            command_id: Some("cmd-1".to_string()),
            node_id: "node-a".to_string(),
            agent_id: "coding".to_string(),
            status: "running".to_string(),
        });

        let handler = RunHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::RunStatus),
            payload: Payload::Control(ControlPayload::RunStatus(RunStatusRequest {
                run_id: "run-1".to_string(),
            })),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        let json = replies[0].payload.data_json();
        assert_eq!(json["run_id"], "run-1");
        assert_eq!(json["status"], "running");
        assert_eq!(json["node_id"], "node-a");
    }

    #[tokio::test]
    async fn run_status_returns_not_found_for_missing_run() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = RunHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::RunStatus),
            payload: Payload::Control(ControlPayload::RunStatus(RunStatusRequest {
                run_id: "nonexistent".to_string(),
            })),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        let json = replies[0].payload.data_json();
        assert_eq!(json["run_id"], "nonexistent");
        assert_eq!(json["status"], "not_found");
        assert!(json["node_id"].is_null());
    }

    #[tokio::test]
    async fn run_status_returns_error_on_wrong_payload() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = RunHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::RunStatus),
            payload: Payload::Control(ControlPayload::NodeList(
                vol_llm_agent_protocol::agent_server_protocol::NodeListRequest {},
            )),
            meta: Default::default(),
        };

        let err = handler.handle(msg).await.unwrap_err();
        assert!(err
            .to_string()
            .contains("expected control.run_status payload"));
    }
}
