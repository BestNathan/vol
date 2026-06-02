use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_core::ToolCall;
use vol_llm_tool::ToolContext;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, ToolOperation, ToolPayload,
};
use crate::domain::handler::DomainHandler;

/// Handler for tool-domain operations.
pub struct ToolHandler {
    tool_registry: Arc<vol_llm_tool::ToolRegistry>,
}

impl ToolHandler {
    pub fn new(tool_registry: Arc<vol_llm_tool::ToolRegistry>) -> Self {
        Self { tool_registry }
    }
}

#[async_trait]
impl DomainHandler for ToolHandler {
    fn name(&self) -> &str {
        "tool"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Tool(ToolOperation::List),
            Operation::Tool(ToolOperation::Call),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Tool(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("tool")),
        };
        match (op, message.payload) {
            (ToolOperation::List, Payload::Tool(ToolPayload::List)) => {
                let mut tools: Vec<serde_json::Value> = self
                    .tool_registry
                    .definitions()
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "name": d.name,
                            "description": d.description,
                            "parameters": d.parameters,
                        })
                    })
                    .collect();
                tools.sort_by(|a, b| {
                    a.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .cmp(b.get("name").and_then(|v| v.as_str()).unwrap_or(""))
                });
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Tool(ToolOperation::List),
                    Payload::Tool(ToolPayload::ListResult { tools }),
                )])
            }
            (ToolOperation::Call, Payload::Tool(ToolPayload::Call { tool_name, arguments })) => {
                let call = ToolCall {
                    id: uuid::Uuid::new_v4().simple().to_string(),
                    name: tool_name.clone(),
                    arguments: serde_json::to_string(&arguments)
                        .unwrap_or_else(|_| "{}".to_string()),
                    r#type: "function".to_string(),
                };
                let context = ToolContext::default();
                match self.tool_registry.execute(&call, &context).await {
                    Ok(result) => {
                        let value = serde_json::json!({
                            "success": result.success,
                            "content": result.content,
                            "error": result.error,
                            "data": result.data,
                        });
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Tool(ToolOperation::Call),
                            Payload::Tool(ToolPayload::CallResult {
                                tool_name,
                                result: value,
                            }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Tool(ToolOperation::Call),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "tool_call_failed".to_string(),
                            message: e,
                            detail: None,
                            terminal: false,
                        },
                    )]),
                }
            }
            (ToolOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("tool.list")),
            (ToolOperation::Call, _) => Err(ProtocolError::PayloadDecodeFailed("tool.call")),
        }
    }
}
