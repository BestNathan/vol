//! The `fs` tool — unified CLI-style entry point for file operations.

use async_trait::async_trait;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

use crate::cli::{commands::ParsedCommand, parser};

pub struct FsCliTool;

impl FsCliTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsCliTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for FsCliTool {
    fn name(&self) -> &'static str {
        "fs"
    }

    fn description(&self) -> &'static str {
        "File system CLI. Single entry point for all file operations.\n\n\
         Usage: fs <subcommand> [--flags]\n\n\
         Subcommands:\n  \
           read    Read file contents with line numbers (--file_path required)\n  \
           write   Create or overwrite a file (--file_path, --content required)\n  \
           edit    Replace exact strings in a file (--file_path, --old_string, --new_string required)\n  \
           grep    Search file content using regex (--pattern required)\n  \
           glob    Find files/directories by glob patterns (--pattern required)\n  \
           scheme  Show parameter definitions [<subcommand>]\n\n\
         Global flags:\n  \
           --json, -o  Output as JSON instead of CLI text\n\n\
         Examples:\n  \
           fs read --file_path src/main.rs --limit 50\n  \
           fs write --file_path notes.txt --content 'hello'\n  \
           fs edit --file_path a.txt --old_string x --new_string y\n  \
           fs grep --pattern 'TODO' --output_mode content\n  \
           fs glob --pattern '**/*.rs'\n  \
           fs scheme read"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The CLI command to execute, e.g. 'read --file_path src/main.rs --limit 50'"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidArguments(
                    "Missing required parameter: 'command'. Usage: fs <subcommand> [--flags]"
                        .to_string(),
                )
            })?;

        let cmd: ParsedCommand = parser::parse(command).map_err(ToolError::InvalidArguments)?;

        crate::cli::executor::execute(cmd, context)
            .await
            .map_err(ToolError::ExecutionFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> FsCliTool {
        FsCliTool::new()
    }

    #[tokio::test]
    async fn test_name_and_description() {
        let t = tool();
        assert_eq!(t.name(), "fs");
        assert!(t.description().contains("read"));
        assert!(t.description().contains("scheme"));
    }

    #[tokio::test]
    async fn test_parameters_require_command() {
        let t = tool();
        let params = t.parameters();
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("command")));
    }

    #[tokio::test]
    async fn test_execute_missing_command() {
        let t = tool();
        let result = t
            .execute(&serde_json::json!({}), &ToolContext::default())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_command() {
        let t = tool();
        let result = t
            .execute(
                &serde_json::json!({"command": "invalid_subcommand"}),
                &ToolContext::default(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_write_read_flow() {
        let t = tool();
        let ctx = ToolContext::for_test();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("e2e.txt");
        let p = path.display().to_string();

        let r = t
            .execute(
                &serde_json::json!({"command": format!("write --file_path {p} --content 'e2e'")}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(r.success);

        let r = t
            .execute(
                &serde_json::json!({"command": format!("read --file_path {p}")}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(r.content.contains("e2e"));
    }

    #[tokio::test]
    async fn test_json_output() {
        let t = tool();
        let ctx = ToolContext::for_test();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("json.txt");
        std::fs::write(&path, "j").unwrap();
        let p = path.display().to_string();

        let result = t
            .execute(
                &serde_json::json!({"command": format!("read --file_path {p} --json")}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(result.content.starts_with('{'));
    }

    #[tokio::test]
    async fn test_sensitivity_safe_for_all_subcommands() {
        let t = tool();
        for cmd in [
            "read --file_path a",
            "write --file_path a --content b",
            "edit --file_path a --old_string x --new_string y",
            "grep --pattern x",
            "glob --pattern '**/*.rs'",
        ] {
            let s = t.sensitivity(&serde_json::json!({"command": cmd}));
            assert!(matches!(s, vol_llm_tool::ToolSensitivity::Safe));
        }
    }
}
