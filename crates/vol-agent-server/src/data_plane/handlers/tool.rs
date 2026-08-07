use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_core::ToolCall;
use vol_llm_tool::ToolContext;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, ToolOperation, ToolPayload,
};
use vol_llm_agent_protocol::DomainHandler;

/// Handler for tool-domain operations.
pub struct ToolHandler {
    tool_registry: Arc<vol_llm_tool::ToolRegistry>,
    working_dir: PathBuf,
}

impl ToolHandler {
    pub fn new(tool_registry: Arc<vol_llm_tool::ToolRegistry>, working_dir: PathBuf) -> Self {
        Self {
            tool_registry,
            working_dir,
        }
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
            (
                ToolOperation::Call,
                Payload::Tool(ToolPayload::Call {
                    tool_name,
                    arguments,
                }),
            ) => {
                let call = ToolCall {
                    id: uuid::Uuid::new_v4().simple().to_string(),
                    name: tool_name.clone(),
                    arguments: serde_json::to_string(&arguments)
                        .unwrap_or_else(|_| "{}".to_string()),
                    r#type: "function".to_string(),
                };
                let context = ToolContext::default().with_sandbox(std::sync::Arc::new(
                    vol_llm_sandbox::local::LocalSandbox::new(Some(self.working_dir.clone())),
                ));
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
                        vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use async_trait::async_trait;
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, MessageKind, Operation, Payload, ToolOperation, ToolPayload,
    };
    use vol_llm_agent_protocol::DomainHandler;
    use vol_llm_tool::ToolResult;
    use vol_llm_tool::ToolResultType;

    use super::ToolHandler;

    /// A simple echo tool for testing.
    struct EchoTool;

    #[async_trait]
    impl vol_llm_tool::ExecutableTool for EchoTool {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn description(&self) -> &'static str {
            "Echoes back the input"
        }
        async fn execute(
            &self,
            args: &serde_json::Value,
            _context: &vol_llm_tool::ToolContext,
        ) -> ToolResultType<ToolResult> {
            Ok(ToolResult {
                call_id: "echo-1".to_string(),
                success: true,
                content: args.to_string(),
                error: None,
                data: Some(args.clone()),
            })
        }
    }

    fn msg(id: &str, op: Operation, payload: Payload) -> AgentServerMessage {
        AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: id.to_string(),
            sender: "client".to_string(),
            receiver: "data-plane".to_string(),
            kind: MessageKind::Command,
            operation: op,
            payload,
            meta: Default::default(),
        }
    }

    #[tokio::test]
    async fn tool_list_returns_registered_tools() {
        let mut registry = vol_llm_tool::ToolRegistry::new();
        registry.register(EchoTool);
        let handler = ToolHandler::new(Arc::new(registry), PathBuf::from("/tmp"));

        let replies = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::List),
                Payload::Tool(ToolPayload::List),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        let tools = json["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "echo");
    }

    #[tokio::test]
    async fn tool_list_returns_empty_when_no_tools() {
        let registry = vol_llm_tool::ToolRegistry::new();
        let handler = ToolHandler::new(Arc::new(registry), PathBuf::from("/tmp"));

        let replies = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::List),
                Payload::Tool(ToolPayload::List),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        let tools = json["tools"].as_array().unwrap();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn tool_call_echoes_back_input() {
        let mut registry = vol_llm_tool::ToolRegistry::new();
        registry.register(EchoTool);
        let handler = ToolHandler::new(Arc::new(registry), PathBuf::from("/tmp"));

        let replies = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::Call),
                Payload::Tool(ToolPayload::Call {
                    tool_name: "echo".to_string(),
                    arguments: serde_json::json!({"message": "hello"}),
                }),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        assert_eq!(json["result"]["success"], true);
        assert_eq!(json["result"]["content"], "{\"message\":\"hello\"}");
    }

    #[tokio::test]
    async fn tool_call_unknown_tool_returns_error() {
        let registry = vol_llm_tool::ToolRegistry::new();
        let handler = ToolHandler::new(Arc::new(registry), PathBuf::from("/tmp"));

        let replies = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::Call),
                Payload::Tool(ToolPayload::Call {
                    tool_name: "nonexistent".to_string(),
                    arguments: serde_json::json!({}),
                }),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        assert_eq!(json["code"], "tool_call_failed");
    }

    #[tokio::test]
    async fn tool_list_with_wrong_payload_returns_error() {
        let registry = vol_llm_tool::ToolRegistry::new();
        let handler = ToolHandler::new(Arc::new(registry), PathBuf::from("/tmp"));

        let err = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::List),
                Payload::Tool(ToolPayload::Call {
                    tool_name: "x".to_string(),
                    arguments: serde_json::json!({}),
                }),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("tool.list"));
    }

    #[tokio::test]
    async fn tool_call_with_wrong_payload_returns_error() {
        let registry = vol_llm_tool::ToolRegistry::new();
        let handler = ToolHandler::new(Arc::new(registry), PathBuf::from("/tmp"));

        let err = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::Call),
                Payload::Tool(ToolPayload::List),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("tool.call"));
    }

    // ── Integration tests: real tools (glob, read_file) via tool.call ──────

    #[tokio::test]
    async fn tool_call_glob_returns_files() {
        let mut registry = vol_llm_tool::ToolRegistry::new();
        registry.register(vol_llm_tools_builtin_glob::GlobTool::new());
        // Use a real temp dir with known contents
        let dir = std::env::temp_dir().join("tool_call_glob_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), b"").unwrap();
        std::fs::write(dir.join("b.rs"), b"").unwrap();
        std::fs::create_dir(dir.join("sub")).unwrap();

        let handler = ToolHandler::new(Arc::new(registry), dir.clone());

        let replies = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::Call),
                Payload::Tool(ToolPayload::Call {
                    tool_name: "glob".to_string(),
                    arguments: serde_json::json!({"pattern": "*.txt", "path": "."}),
                }),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        assert_eq!(
            json["result"]["success"], true,
            "glob should succeed: {json:?}"
        );
        let content = json["result"]["content"].as_str().unwrap();
        assert!(content.contains("a.txt"), "should find a.txt: {content}");
        assert!(
            !content.contains("b.rs"),
            "should not match b.rs: {content}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn tool_call_read_file_returns_content() {
        let mut registry = vol_llm_tool::ToolRegistry::new();
        registry.register(vol_llm_tools_builtin_read::ReadTool::new());
        let dir = std::env::temp_dir().join("tool_call_read_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hello.txt"), b"hello world\nline two\n").unwrap();

        let handler = ToolHandler::new(Arc::new(registry), dir.clone());

        let replies = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::Call),
                Payload::Tool(ToolPayload::Call {
                    tool_name: "read_file".to_string(),
                    arguments: serde_json::json!({"file_path": "hello.txt", "limit": 2}),
                }),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        assert_eq!(
            json["result"]["success"], true,
            "read_file should succeed: {json:?}"
        );
        let content = json["result"]["content"].as_str().unwrap();
        assert!(
            content.contains("hello world"),
            "should contain file content: {content}"
        );
        assert!(
            content.contains("line two"),
            "should contain second line: {content}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn tool_call_glob_empty_dir_returns_success() {
        let mut registry = vol_llm_tool::ToolRegistry::new();
        registry.register(vol_llm_tools_builtin_glob::GlobTool::new());
        let dir = std::env::temp_dir().join("tool_call_glob_empty_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let handler = ToolHandler::new(Arc::new(registry), dir.clone());

        let replies = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::Call),
                Payload::Tool(ToolPayload::Call {
                    tool_name: "glob".to_string(),
                    arguments: serde_json::json!({"pattern": "*", "path": "."}),
                }),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        assert_eq!(
            json["result"]["success"], true,
            "glob empty dir should succeed: {json:?}"
        );
        assert!(
            json["result"]["content"]
                .as_str()
                .unwrap()
                .contains("No matches found"),
            "glob empty dir should return no-matches message: {json:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn tool_call_glob_relative_path_resolves_correctly() {
        // Regression: "." path was normalized to "" causing ENOENT
        let mut registry = vol_llm_tool::ToolRegistry::new();
        registry.register(vol_llm_tools_builtin_glob::GlobTool::new());
        let dir = std::env::temp_dir().join("tool_call_glob_rel_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("x.log"), b"").unwrap();

        let handler = ToolHandler::new(Arc::new(registry), dir.clone());

        // path="." is the key regression case
        let replies = handler
            .handle(msg(
                "1",
                Operation::Tool(ToolOperation::Call),
                Payload::Tool(ToolPayload::Call {
                    tool_name: "glob".to_string(),
                    arguments: serde_json::json!({"pattern": "*.log", "path": "."}),
                }),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        assert_eq!(
            json["result"]["success"], true,
            "glob with '.' path should succeed, got: {json:?}"
        );
        assert!(json["result"]["content"]
            .as_str()
            .unwrap()
            .contains("x.log"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
