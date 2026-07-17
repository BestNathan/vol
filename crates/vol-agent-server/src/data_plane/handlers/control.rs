use async_trait::async_trait;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, Operation, Payload, ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;

use crate::data_plane::command::accept_control_command;

pub struct DataPlaneControlHandler;

impl Default for DataPlaneControlHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl DataPlaneControlHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DomainHandler for DataPlaneControlHandler {
    fn name(&self) -> &str {
        "data-plane-control"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![Operation::Control(ControlOperation::Command)]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match message.payload.clone() {
            Payload::Control(ControlPayload::Command(command)) => {
                let ack = accept_control_command(&command).await;
                Ok(vec![AgentServerMessage {
                    sender: "node".to_string(),
                    receiver: message.sender,
                    ..AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Control(ControlOperation::Command),
                        Payload::Control(ControlPayload::CommandAck(ack)),
                    )
                }])
            }
            _ => Err(ProtocolError::PayloadDecodeFailedOwned(
                "expected control.command payload".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, ControlCommand, ControlCommandOperation, ControlOperation,
        ControlPayload, MessageKind, Operation, Payload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use super::DataPlaneControlHandler;

    #[tokio::test]
    async fn control_command_health_check_returns_ack() {
        let handler = DataPlaneControlHandler::new();
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "cmd-1".to_string(),
            sender: "control".to_string(),
            receiver: "node".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::Command),
            payload: Payload::Control(ControlPayload::Command(ControlCommand {
                command_id: "cmd-1".to_string(),
                node_id: "node-a".to_string(),
                operation: ControlCommandOperation::HealthCheck,
                deadline_ms: None,
            })),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        let json = replies[0].payload.data_json();
        assert_eq!(json["accepted"], true);
        assert_eq!(json["command_id"], "cmd-1");
    }
}
