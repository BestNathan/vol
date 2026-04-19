//! vol-llm-tools-builtin-bash: Bash tool implementation for executing shell commands with security checks.

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Duration;
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;

use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType, ToolSensitivity};

/// Error type for builtin tools
/// Re-exported from vol_llm_tool for convenience
pub use vol_llm_tool::ToolError as BuiltinToolError;

/// Maximum output size (1MB)
const MAX_OUTPUT_SIZE: usize = 1024 * 1024;

/// Default timeout in milliseconds (120 seconds)
const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Grace period after SIGTERM before escalating to SIGKILL.
const SIGTERM_GRACE_PERIOD: Duration = Duration::from_secs(5);

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
                }
            },
            "required": ["command"]
        })
    }

    fn sensitivity(&self, args: &serde_json::Value) -> ToolSensitivity {
        if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
            // All bash commands require human approval since they execute arbitrary shell code.
            // check_security() in execute() provides defense-in-depth for truly dangerous patterns.
            ToolSensitivity::RequiresApproval {
                reason: format!("Running shell command: {}", cmd),
            }
        } else {
            ToolSensitivity::Safe
        }
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        // Parse arguments
        let params: BashParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        // Security check - block dangerous patterns BEFORE execution
        if let Err(e) = self.check_security(&params.command) {
            return Err(ToolError::ExecutionFailed(e.to_string()));
        }

        // Determine timeout
        let timeout_duration = params
            .timeout
            .map(Duration::from_millis)
            .unwrap_or(self.default_timeout);

        // Spawn on a blocking thread using std::process so we get reliable
        // pipe-based output collection. We create a new process group so we
        // can kill the entire process tree on timeout.
        let output = tokio::task::spawn_blocking({
            let timeout_duration = timeout_duration;
            let command_str = params.command.clone();
            let working_dir = params.working_dir.clone();
            let sandbox_root = context.sandbox.as_ref().map(|s| s.root_path().to_path_buf());
            move || -> Result<std::process::Output, String> {
                let mut std_cmd = std::process::Command::new("bash");
                std_cmd.arg("-c").arg(&command_str);

                #[cfg(unix)]
                std_cmd.process_group(0);

                if let Some(ref wd) = working_dir {
                    std_cmd.current_dir(wd);
                } else if let Some(ref root) = sandbox_root {
                    std_cmd.current_dir(root);
                }

                std_cmd.stdout(std::process::Stdio::piped());
                std_cmd.stderr(std::process::Stdio::piped());

                let mut child = std_cmd.spawn().map_err(|e| format!("Failed to spawn command: {}", e))?;
                let pgid = child.id() as i32;

                // Poll with try_wait to implement timeout
                let deadline = std::time::Instant::now() + timeout_duration;
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            // Exited normally — collect output
                            return Ok(child.wait_with_output().unwrap_or_else(|_| std::process::Output {
                                status: std::process::ExitStatus::default(),
                                stdout: Vec::new(),
                                stderr: Vec::new(),
                            }));
                        }
                        Ok(None) => {
                            if std::time::Instant::now() >= deadline {
                                // Timeout: SIGTERM the process group
                                let _ = nix::sys::signal::kill(
                                    nix::unistd::Pid::from_raw(-pgid),
                                    nix::sys::signal::Signal::SIGTERM,
                                );

                                // Wait for graceful exit during grace period
                                let grace_start = std::time::Instant::now();
                                let mut exited = false;
                                loop {
                                    if grace_start.elapsed() > SIGTERM_GRACE_PERIOD {
                                        break;
                                    }
                                    match child.try_wait() {
                                        Ok(Some(_)) => {
                                            exited = true;
                                            break;
                                        }
                                        Ok(None) => {
                                            std::thread::sleep(Duration::from_millis(100));
                                        }
                                        Err(_) => break,
                                    }
                                }

                                if exited {
                                    let _output = child.wait_with_output().unwrap_or_else(|_| std::process::Output {
                                        status: std::process::ExitStatus::default(),
                                        stdout: Vec::new(),
                                        stderr: Vec::new(),
                                    });
                                    return Err(format!(
                                        "Command timed out after {:?}. Sent SIGTERM to process group {}, process exited.",
                                        timeout_duration, pgid
                                    ));
                                } else {
                                    // Still running — SIGKILL
                                    let _ = nix::sys::signal::kill(
                                        nix::unistd::Pid::from_raw(-pgid),
                                        nix::sys::signal::Signal::SIGKILL,
                                    );
                                    let _ = child.wait();
                                    return Err(format!(
                                        "Command timed out after {:?}. Sent SIGTERM then SIGKILL to process group {}.",
                                        timeout_duration, pgid
                                    ));
                                }
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(e) => {
                            return Err(format!("Failed to wait for command: {}", e));
                        }
                    }
                }
            }
        })
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Blocking task panicked: {}", e)))?
        .map_err(|e| ToolError::ExecutionFailed(e))?;

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
