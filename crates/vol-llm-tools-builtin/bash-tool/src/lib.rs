//! vol-llm-tools-builtin-bash: Bash tool implementation for executing shell commands with security checks.

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

/// Error type for builtin tools
/// Re-exported from vol_llm_tool for convenience
pub use vol_llm_tool::ToolError as BuiltinToolError;

/// Maximum output size (1MB)
const MAX_OUTPUT_SIZE: usize = 1024 * 1024;

/// Default timeout in milliseconds (120 seconds)
const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Dangerous command patterns that are blocked
const DANGEROUS_PATTERNS: &[&str] = &[
    r"rm\s+(-[a-zA-Z]*r[a-zA-Z]*f|[a-zA-Z]*f[a-zA-Z]*r).*\s+/",  // rm -rf /
    r":\s*\(\s*\)\s*\{",                                          // Fork bomb start :(){
    r"mkfs",                                                      // Format disk
    r"dd\s+of=/dev/(zero|sda|nvme)",                              // Write to device
    r">\s*/dev/sd[a-z]",                                          // Redirect to device
    r"curl\s+[^|]*\|\s*(?:bash|sh)",                              // Curl pipe bash
    r"wget\s+[^|]*-O[^|]*\|\s*(?:bash|sh)",                       // Wget pipe bash
    r"nc\s+-e\s+",                                                // Netcat reverse shell
    r"bash\s+-i\s+>&\s+/dev/tcp",                                 // Bash reverse shell
];

/// Error type for bash tool operations
#[derive(Debug, thiserror::Error)]
pub enum BashToolError {
    #[error("Security violation: {0}")]
    SecurityViolation(String),
    #[error("Command timed out after {0:?}")]
    Timeout(Duration),
    #[error("Output exceeded maximum size ({0} bytes), truncated")]
    OutputTooLarge(usize),
}

/// Parameters for the Bash tool
#[derive(Debug, Deserialize, Serialize)]
pub struct BashParams {
    /// Command to execute
    pub command: String,
    /// Timeout in milliseconds (default: 120000)
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Working directory for the command
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Whether to run in background (not implemented, for future use)
    #[serde(default)]
    pub run_in_background: Option<bool>,
}

/// The Bash tool for executing shell commands with security checks
pub struct BashTool {
    dangerous_patterns: Vec<Regex>,
    default_timeout: Duration,
    max_output_size: usize,
}

impl BashTool {
    /// Create a new BashTool instance
    pub fn new() -> Self {
        let dangerous_patterns: Vec<Regex> = DANGEROUS_PATTERNS
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();

        Self {
            dangerous_patterns,
            default_timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            max_output_size: MAX_OUTPUT_SIZE,
        }
    }

    /// Check if a command matches any dangerous patterns
    fn check_security(&self, command: &str) -> Result<(), BashToolError> {
        for pattern in &self.dangerous_patterns {
            if pattern.is_match(command) {
                return Err(BashToolError::SecurityViolation(format!(
                    "Command matches dangerous pattern and was blocked"
                )));
            }
        }
        Ok(())
    }

    /// Truncate output if it exceeds maximum size
    fn truncate_output(&self, output: String) -> String {
        if output.len() <= self.max_output_size {
            output
        } else {
            let truncated = output[..self.max_output_size].to_string();
            format!("{}...", truncated)
        }
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for BashTool {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        "Execute shell commands with security checks. Blocks dangerous patterns like 'rm -rf /', fork bombs, disk formatting, and reverse shells. Use with caution."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 120000)",
                    "default": 120000
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for the command"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Whether to run in background (not implemented)",
                    "default": false
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        // Parse arguments
        let params: BashParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        // Security check - block dangerous patterns BEFORE execution
        if let Err(e) = self.check_security(&params.command) {
            return Err(ToolError::ExecutionFailed(e.to_string()));
        }

        // Build command
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&params.command);

        // Set working directory if provided
        if let Some(ref working_dir) = params.working_dir {
            cmd.current_dir(working_dir);
        }

        // Determine timeout
        let timeout_duration = params
            .timeout
            .map(Duration::from_millis)
            .unwrap_or(self.default_timeout);

        // Execute with timeout
        let output = match timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Err(ToolError::ExecutionFailed(format!(
                    "Failed to execute command: {}",
                    e
                )));
            }
            Err(_) => {
                return Err(ToolError::ExecutionFailed(format!(
                    "Command timed out after {:?}",
                    timeout_duration
                )));
            }
        };

        // Convert output to strings
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Truncate if too large
        let stdout = self.truncate_output(stdout);
        let stderr = self.truncate_output(stderr);

        // Build result content
        let mut content = String::new();
        if !stdout.is_empty() {
            content.push_str("stdout:\n");
            content.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !stdout.is_empty() {
                content.push_str("\n");
            }
            content.push_str("stderr:\n");
            content.push_str(&stderr);
        }
        if content.is_empty() {
            content = "Command executed successfully (no output)".to_string();
        }

        Ok(ToolResult::success(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_bash_simple_command() {
        let tool = BashTool::new();
        let args = json!({
            "command": "echo hello"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_rm_rf_blocked() {
        let tool = BashTool::new();
        let args = json!({
            "command": "rm -rf /"
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
        // Check that the error message contains security-related text
        let err_str = format!("{}", err);
        // The error is wrapped in ExecutionFailed, but the inner error is SecurityViolation
        // We need to check the actual error chain - for now just verify it fails
        assert!(err_str.contains("blocked") || err_str.contains("Security"));
    }

    #[tokio::test]
    async fn test_bash_fork_bomb_blocked() {
        let tool = BashTool::new();
        let args = json!({
            "command": ":(){:|:&}:"
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn test_bash_rm_file_allowed() {
        let tool = BashTool::new();
        // rm with a specific file (not /) should be allowed
        let args = json!({
            "command": "rm /tmp/nonexistent_file_12345"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        // The command runs but fails because file doesn't exist - that's expected
        // The important thing is it's NOT blocked by security check
        assert!(result.success || result.content.contains("No such file"));
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let tool = BashTool::new();
        let args = json!({
            "command": "sleep 5",
            "timeout": 100
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
        let err_str = format!("{}", err);
        assert!(err_str.contains("timed out") || err_str.contains("Timeout"));
    }
}
