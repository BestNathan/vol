//! Tool calling types.

use serde::{Deserialize, Serialize};

/// Tool definition for LLM
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Parameters schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Tool call from LLM
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    /// Tool call ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Tool arguments (JSON string)
    pub arguments: String,
    /// Tool type (always "function" for function calling)
    #[serde(default = "default_tool_type")]
    pub r#type: String,
}

fn default_tool_type() -> String {
    "function".to_string()
}

/// Tool choice strategy
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    /// Auto-decide whether to use tools
    Auto,
    /// Must use at least one tool
    Required,
    /// Do not use tools
    None,
    /// Force use of specific tool
    Specific { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition_creation() {
        let tool = ToolDefinition {
            name: "test_tool".to_string(),
            description: Some("A test tool".to_string()),
            parameters: None,
        };
        assert_eq!(tool.name, "test_tool");
    }

    #[test]
    fn test_tool_call_creation() {
        let call = ToolCall {
            id: "call_123".to_string(),
            name: "test".to_string(),
            arguments: "{}".to_string(),
            r#type: default_tool_type(),
        };
        assert_eq!(call.id, "call_123");
    }
}
