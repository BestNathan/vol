//! Tool trait and types.

use async_trait::async_trait;
use vol_llm_core::{Message, SandboxRef, ToolDefinition};

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
#[derive(Clone, Default)]
pub struct ToolContext {
    pub messages: Vec<Message>,
    pub sandbox: Option<SandboxRef>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("messages", &self.messages)
            .field("sandbox", &self.sandbox.as_ref().map(|_| "<sandbox>"))
            .finish()
    }
}

impl ToolContext {
    /// Set the sandbox for this tool context
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// Resolve a path through the sandbox, or return unchanged if no sandbox.
    pub fn resolve_path(&self, rel: &str) -> std::result::Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        match &self.sandbox {
            Some(sandbox) => sandbox.resolve_path(rel).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            }),
            None => Ok(std::path::PathBuf::from(rel)),
        }
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
impl<T: ExecutableTool + Send + Sync> Tool for T {
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
                    format!("Invalid JSON: {}", e),
                ))
            })?;

        self.execute(&json_args, context)
            .await
            .map_err(|e| -> Box<dyn Error + Send> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Tool execution failed: {}", e),
                ))
            })
    }
}
