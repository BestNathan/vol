//! Wraps `vol-llm-cli-tool::CliTool` as an `ExecutableTool` for the
//! direct-load (path A) deployment.
//!
//! The `name` and `description` fields from the TOML config are leaked
//! once at construction time so that the `&'static str` lifetime
//! required by `ExecutableTool` is satisfied.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_cli_tool::{CliTool, CliToolConfig, CliToolError};
use vol_llm_sandbox::registry::SandboxRegistry;
use vol_llm_tool::{ExecutableTool, ToolRegistry, ToolResult, ToolResultType, ToolSensitivity};

pub struct CliToolExecutable {
    name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
    inner: CliTool,
}

impl CliToolExecutable {
    pub fn from_config(
        config: CliToolConfig,
        sandbox: Arc<dyn vol_llm_sandbox::Sandbox>,
    ) -> Self {
        let name: &'static str =
            Box::leak(config.name.clone().into_boxed_str());
        let description: &'static str =
            Box::leak(config.description.clone().into_boxed_str());
        let parameters = serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "CLI command to run. First token must be one of this tool's declared binaries."
                }
            },
            "required": ["command"]
        });
        let inner = CliTool::new(config, sandbox);
        Self { name, description, parameters, inner }
    }
}

#[async_trait]
impl ExecutableTool for CliToolExecutable {
    fn name(&self) -> &'static str { self.name }
    fn description(&self) -> &'static str { self.description }
    fn parameters(&self) -> serde_json::Value { self.parameters.clone() }
    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe // MVP: no approval gates per spec
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &vol_llm_tool::ToolContext,
    ) -> ToolResultType<ToolResult> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| vol_llm_tool::ToolError::InvalidArguments(
                "missing required parameter: 'command'".into(),
            ))?;

        match self.inner.run(command).await {
            Ok(output) => {
                let mut result = if output.success {
                    ToolResult::success(output.content)
                } else {
                    ToolResult::failure(output.content)
                };
                result.call_id = args
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(result)
            }
            Err(e @ CliToolError::InvalidArguments(_))
            | Err(e @ CliToolError::BinaryNotAllowed { .. }) => {
                Err(vol_llm_tool::ToolError::InvalidArguments(e.to_string()))
            }
            Err(e) => Err(vol_llm_tool::ToolError::ExecutionFailed(e.to_string())),
        }
    }
}

/// Load every `*.toml` from `dir` and register each as a `CliToolExecutable`.
///
/// Silently returns Ok(0) if `dir` does not exist (no cli-tools configured).
/// Fails hard on parse errors, name collisions, or missing sandbox refs.
pub async fn register_all(
    registry: &mut ToolRegistry,
    sandbox_registry: &SandboxRegistry,
    dir: &Path,
) -> Result<usize, String> {
    let tools = vol_llm_cli_tool::load_dir(dir, sandbox_registry)
        .await
        .map_err(|e| e.to_string())?;
    let count = tools.len();
    for tool in tools {
        let (config, sandbox) = tool.into_parts();
        let exe = CliToolExecutable::from_config(config, sandbox);
        registry.register(exe);
    }
    Ok(count)
}
