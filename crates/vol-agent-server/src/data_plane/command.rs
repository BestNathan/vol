use vol_llm_agent_protocol::agent_server_protocol::{
    CommandAck, ControlCommand, ControlCommandOperation,
};

pub async fn accept_control_command(command: &ControlCommand) -> CommandAck {
    let run_id = match &command.operation {
        ControlCommandOperation::SubmitAgent { .. } => Some(format!("run_{}", command.command_id)),
        _ => None,
    };

    CommandAck {
        command_id: command.command_id.clone(),
        accepted: true,
        run_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::AgentInput;

    #[tokio::test]
    async fn submit_agent_command_returns_accepted_with_run_id() {
        let command = ControlCommand {
            command_id: "command_123".to_string(),
            node_id: "node_123".to_string(),
            operation: ControlCommandOperation::SubmitAgent {
                target: None,
                input: AgentInput::text("hello"),
            },
            deadline_ms: None,
        };

        let ack = accept_control_command(&command).await;

        assert!(ack.accepted);
        assert_eq!(ack.command_id, "command_123");
        assert_eq!(ack.run_id, Some("run_command_123".to_string()));
    }

    #[tokio::test]
    async fn health_check_command_returns_accepted_without_run_id() {
        let command = ControlCommand {
            command_id: "command_456".to_string(),
            node_id: "node_123".to_string(),
            operation: ControlCommandOperation::HealthCheck,
            deadline_ms: None,
        };

        let ack = accept_control_command(&command).await;

        assert!(ack.accepted);
        assert_eq!(ack.command_id, "command_456");
        assert_eq!(ack.run_id, None);
    }
}
