//! HITL (Human In The Loop) confirmation mechanism.

use serde::{Deserialize, Serialize};
use crate::coding::error::HITLError;

/// HITL decision
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum HITLDecision {
    Approve,
    Reject { reason: String },
    Modify { new_command: String },
}

/// HITL handler - checks if operations require user confirmation
pub struct HITLHandler {
    enabled: bool,
}

impl HITLHandler {
    /// Create new HITL handler
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Check if an operation requires HITL confirmation
    pub async fn check_operation(
        &self,
        tool_name: &str,
        arguments: &str,
    ) -> Result<HITLDecision, HITLError> {
        if !self.enabled {
            return Ok(HITLDecision::Approve);
        }

        // Check for dangerous patterns
        if self.is_dangerous(tool_name, arguments) {
            return Ok(HITLDecision::Reject {
                reason: "Dangerous operation detected".to_string(),
            });
        }

        // For MVP, auto-approve non-dangerous operations
        // In production, this would prompt the user via HTTP/CLI
        Ok(HITLDecision::Approve)
    }

    /// Check if operation matches dangerous patterns
    fn is_dangerous(&self, tool_name: &str, arguments: &str) -> bool {
        // Check bash tool for dangerous commands
        if tool_name == "bash" {
            let dangerous_patterns = [
                "rm -rf",
                "rm -fr",
                "rm -r /",
                ":(){:|:&};:",  // fork bomb
                "mkfs",
                "dd of=/dev/",
                "> /dev/sd",
            ];

            for pattern in dangerous_patterns {
                if arguments.contains(pattern) {
                    return true;
                }
            }
        }

        // Check for DeleteTool (if implemented in future)
        if tool_name == "delete_file" {
            return true;
        }

        false
    }
}
