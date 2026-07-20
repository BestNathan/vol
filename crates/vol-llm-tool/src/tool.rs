//! Tool trait and types.

use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_core::{Message, ToolDefinition};
use vol_llm_sandbox::SandboxRef;

use std::error::Error;

/// Tool execution result
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub call_id: String,
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
    pub data: Option<serde_json::Value>,
}

impl ToolResult {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            error: None,
            data: None,
            call_id: String::new(),
        }
    }

    pub fn failure(content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            success: false,
            content: content_str.clone(),
            error: Some(content_str),
            data: None,
            call_id: String::new(),
        }
    }
}

/// Tool execution context
#[derive(Clone)]
pub struct ToolContext {
    pub messages: Vec<Message>,
    pub sandbox: SandboxRef, // Always set
    pub agent_def: Option<vol_llm_core::AgentDef>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            sandbox: Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None)),
            agent_def: None,
        }
    }
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("messages", &self.messages)
            .field(
                "sandbox",
                &format_args!("{}:{}", self.sandbox.kind(), self.sandbox.name()),
            )
            .field("agent_def", &self.agent_def)
            .finish()
    }
}

impl ToolContext {
    /// Create a permissive ToolContext for testing — sandbox rooted at `/`
    /// so that absolute paths from `tempfile` are accepted.
    pub fn for_test() -> Self {
        Self {
            messages: Vec::new(),
            sandbox: Arc::new(vol_llm_sandbox::local::LocalSandbox::new(Some(
                std::path::PathBuf::from("/"),
            ))),
            agent_def: None,
        }
    }

    /// Set the sandbox for this tool context
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        self.sandbox = sandbox;
        self
    }

    /// Set the agent definition for this tool context.
    pub fn with_agent_def(mut self, def: vol_llm_core::AgentDef) -> Self {
        self.agent_def = Some(def);
        self
    }

    /// Resolve a path through the sandbox.
    pub fn resolve_path(
        &self,
        rel: &str,
    ) -> std::result::Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        self.sandbox
            .resolve_path(rel)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

/// Tool error
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Tool not found: {0}")]
    NotFound(String),
}

/// Result type alias
pub type ToolResultType<T> = std::result::Result<T, ToolError>;

/// Result type alias for backward compatibility
pub type Result<T> = ToolResultType<T>;

/// Tool execution sensitivity level.
/// Tools declare whether they require human approval before execution.
#[derive(Debug, Clone)]
pub enum ToolSensitivity {
    /// Safe operation, no approval needed
    Safe,
    /// Requires human approval with the given reason
    RequiresApproval { reason: String },
}

/// Executable tool trait for legacy compatibility
#[async_trait]
pub trait ExecutableTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    /// Declare sensitivity level for the given arguments.
    /// Override to return RequiresApproval for dangerous operations.
    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult>;
}

/// Tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Option<serde_json::Value>;

    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: Some(self.description().to_string()),
            parameters: self.parameters(),
        }
    }

    async fn execute(
        &self,
        args: &str,
        context: &ToolContext,
    ) -> std::result::Result<ToolResult, Box<dyn Error + Send>>;
}

/// Blanket implementation of Tool for any type that implements ExecutableTool
#[async_trait]
impl<T: ?Sized + ExecutableTool + Send + Sync> Tool for T {
    fn name(&self) -> &str {
        self.name()
    }

    fn description(&self) -> &str {
        self.description()
    }

    fn parameters(&self) -> Option<serde_json::Value> {
        Some(self.parameters())
    }

    async fn execute(
        &self,
        args: &str,
        context: &ToolContext,
    ) -> std::result::Result<ToolResult, Box<dyn Error + Send>> {
        // Parse JSON arguments
        let json_args: serde_json::Value =
            serde_json::from_str(args).map_err(|e| -> Box<dyn Error + Send> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Invalid JSON: {e}"),
                ))
            })?;

        self.execute(&json_args, context)
            .await
            .map_err(|e| -> Box<dyn Error + Send> {
                Box::new(std::io::Error::other(format!("Tool execution failed: {e}")))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ToolResult tests ──────────────────────────────────────────────

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("done");
        assert!(result.success);
        assert_eq!(result.content, "done");
        assert!(result.error.is_none());
        assert!(result.data.is_none());
    }

    #[test]
    fn test_tool_result_failure() {
        let result = ToolResult::failure("boom");
        assert!(!result.success);
        assert_eq!(result.content, "boom");
        assert_eq!(result.error, Some("boom".to_string()));
    }

    #[test]
    fn test_tool_result_success_string() {
        let result = ToolResult::success(String::from("owned"));
        assert!(result.success);
        assert_eq!(result.content, "owned");
    }

    // ── ToolContext tests ─────────────────────────────────────────────

    #[test]
    fn test_tool_context_default() {
        let ctx = ToolContext::default();
        assert!(ctx.messages.is_empty());
        assert!(ctx.agent_def.is_none());
        assert_eq!(ctx.sandbox.kind(), "local");
    }

    #[test]
    fn test_tool_context_for_test() {
        let ctx = ToolContext::for_test();
        assert!(ctx.messages.is_empty());
        assert_eq!(ctx.sandbox.kind(), "local");
    }

    #[test]
    fn test_tool_context_with_sandbox() {
        let ctx = ToolContext::default();
        let local = Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None));
        let ctx = ctx.with_sandbox(local);
        assert_eq!(ctx.sandbox.kind(), "local");
    }

    #[test]
    fn test_tool_context_with_agent_def() {
        let ctx = ToolContext::default();
        let def = vol_llm_core::AgentDef {
            id: "repo:test-agent".into(),
            name: "test-agent".into(),
            r#type: "test-agent".into(),
            description: "test agent".into(),
            scope: vol_llm_core::AgentScope::Repo,
            tools: None,
            disallowed_tools: None,
            model: Some("gpt-4".into()),
            max_iterations: None,
            max_history_messages: None,
            prompt: "You are a test agent.".into(),
            working_dir: None,
            context_files: vec![],
            sandbox: None,
            tool_config: None,
            mcps: None,
        };
        let ctx = ctx.with_agent_def(def);
        assert!(ctx.agent_def.is_some());
        assert_eq!(ctx.agent_def.unwrap().name, "test-agent");
    }

    #[test]
    fn test_tool_context_resolve_path() {
        let ctx = ToolContext::for_test();
        let path = ctx.resolve_path("/tmp/test").unwrap();
        assert!(path.is_absolute() || path.starts_with("/"));
    }

    #[test]
    fn test_tool_context_debug_format() {
        let ctx = ToolContext::for_test();
        let debug_str = format!("{ctx:?}");
        assert!(debug_str.contains("ToolContext"));
        assert!(debug_str.contains("sandbox"));
    }

    // ── ToolError tests ───────────────────────────────────────────────

    #[test]
    fn test_tool_error_display_invalid_arguments() {
        let err = ToolError::InvalidArguments("missing field".into());
        assert!(err.to_string().contains("missing field"));
    }

    #[test]
    fn test_tool_error_display_execution_failed() {
        let err = ToolError::ExecutionFailed("timeout".into());
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_tool_error_display_not_found() {
        let err = ToolError::NotFound("tool_x".into());
        assert!(err.to_string().contains("tool_x"));
    }

    // ── ToolSensitivity tests ─────────────────────────────────────────

    #[test]
    fn test_tool_sensitivity_safe() {
        let sensitivity = ToolSensitivity::Safe;
        // Verify it's a valid variant
        match sensitivity {
            ToolSensitivity::Safe => {}
            _ => panic!("expected Safe"),
        }
    }

    #[test]
    fn test_tool_sensitivity_requires_approval() {
        let sensitivity = ToolSensitivity::RequiresApproval {
            reason: "delete files".into(),
        };
        match &sensitivity {
            ToolSensitivity::RequiresApproval { reason } => {
                assert_eq!(reason, "delete files");
            }
            _ => panic!("expected RequiresApproval"),
        }
    }

    // ── Tool blanket impl tests ───────────────────────────────────────

    struct StubTool {
        name: &'static str,
        desc: &'static str,
        params: serde_json::Value,
    }

    #[async_trait]
    impl ExecutableTool for StubTool {
        fn name(&self) -> &'static str {
            self.name
        }
        fn description(&self) -> &'static str {
            self.desc
        }
        fn parameters(&self) -> serde_json::Value {
            self.params.clone()
        }
        async fn execute(
            &self,
            _args: &serde_json::Value,
            _context: &ToolContext,
        ) -> ToolResultType<ToolResult> {
            Ok(ToolResult::success("stub ok"))
        }
    }

    #[test]
    fn test_tool_blanket_name() {
        let stub = StubTool {
            name: "stub",
            desc: "a stub tool",
            params: serde_json::json!({"type": "object"}),
        };
        let tool: &dyn Tool = &stub;
        assert_eq!(tool.name(), "stub");
    }

    #[test]
    fn test_tool_blanket_description() {
        let stub = StubTool {
            name: "stub",
            desc: "a stub tool",
            params: serde_json::json!({"type": "object"}),
        };
        let tool: &dyn Tool = &stub;
        assert_eq!(tool.description(), "a stub tool");
    }

    #[test]
    fn test_tool_blanket_parameters() {
        let stub = StubTool {
            name: "stub",
            desc: "desc",
            params: serde_json::json!({"type": "object"}),
        };
        let tool: &dyn Tool = &stub;
        assert!(tool.parameters().is_some());
    }

    #[test]
    fn test_tool_blanket_to_definition() {
        let stub = StubTool {
            name: "stub",
            desc: "desc",
            params: serde_json::json!({"type": "object"}),
        };
        let tool: &dyn Tool = &stub;
        let def = tool.to_definition();
        assert_eq!(def.name, "stub");
        assert_eq!(def.description, Some("desc".to_string()));
    }

    #[tokio::test]
    async fn test_tool_blanket_execute_valid_json() {
        let stub = StubTool {
            name: "stub",
            desc: "desc",
            params: serde_json::json!({"type": "object"}),
        };
        let tool: &dyn Tool = &stub;
        let ctx = ToolContext::for_test();
        let result = tool.execute(r#"{"key": "value"}"#, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.content, "stub ok");
    }

    #[tokio::test]
    async fn test_tool_blanket_execute_invalid_json() {
        let stub = StubTool {
            name: "stub",
            desc: "desc",
            params: serde_json::json!({"type": "object"}),
        };
        let tool: &dyn Tool = &stub;
        let ctx = ToolContext::for_test();
        let result = tool.execute("not json", &ctx).await;
        assert!(result.is_err());
    }
}
