use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolResultType, ToolSensitivity};

use crate::cli::{commands::ParsedCommand, parser};
use crate::store::TaskStore;

pub struct TaskCliTool {
    store: Arc<dyn TaskStore>,
}

impl TaskCliTool {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskCliTool {
    fn name(&self) -> &'static str {
        "task"
    }

    fn description(&self) -> &'static str {
        "Task management CLI. Single entry point for all task operations.\n\n\
         Usage: task <subcommand> [--flags]\n\n\
         Subcommands:\n  \
           create   Create a new task (--name, --desc required)\n  \
           update   Update a task (--id required)\n  \
           get      Get task details by --id\n  \
           list     List tasks [--status S] [--assignee A]\n  \
           stop     Stop a running task (--id required)\n  \
           output   Read task output (--id required)\n  \
           claim    Claim a pending task [--id ID]\n  \
           scheme   Show parameter definitions [<subcommand>]\n\n\
         Shortcuts (minimal params, smart defaults):\n  \
           +task    Quick create (--name required, rest auto-filled)\n  \
           +done    Quick complete (--id required)\n  \
           +claim   Quick claim first ready pending task\n\n\
         Global flags:\n  \
           --json, -o  Output as JSON instead of CLI text\n\n\
         Examples:\n  \
           task create --name 'Fix login' --desc 'Handle OAuth error'\n  \
           task +task --name 'Quick fix'\n  \
           task update --id 1 --status completed\n  \
           task list --status pending\n  \
           task get --id 42 --json\n  \
           task scheme create"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The CLI command to execute, e.g. 'create --name FixBug --desc ...'"
                }
            },
            "required": ["command"]
        })
    }

    fn sensitivity(&self, args: &serde_json::Value) -> ToolSensitivity {
        let cmd_str = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
        let first_token = cmd_str.trim().split_whitespace().next().unwrap_or("");

        match first_token {
            "update" | "stop" | "+done" | "+claim" => ToolSensitivity::RequiresApproval {
                reason: "This operation modifies task state".to_string(),
            },
            _ => ToolSensitivity::Safe,
        }
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
                vol_llm_tool::ToolError::InvalidArguments(
                    "Missing required parameter: 'command'. Usage: task <subcommand> [--flags]"
                        .to_string(),
                )
            })?;

        let cmd: ParsedCommand =
            parser::parse(command).map_err(|e| vol_llm_tool::ToolError::InvalidArguments(e))?;

        crate::cli::executor::execute(cmd, &self.store, context)
            .await
            .map_err(vol_llm_tool::ToolError::ExecutionFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::InMemoryTaskStore;

    fn tool() -> TaskCliTool {
        TaskCliTool::new(Arc::new(InMemoryTaskStore::new()))
    }

    #[tokio::test]
    async fn test_name_and_description() {
        let t = tool();
        assert_eq!(t.name(), "task");
        assert!(t.description().contains("create"));
    }

    #[tokio::test]
    async fn test_parameters_require_command() {
        let t = tool();
        let params = t.parameters();
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("command")));
    }

    #[tokio::test]
    async fn test_execute_create_via_cli() {
        let t = tool();
        let args = serde_json::json!({
            "command": "create --name 'test via cli' --desc 'from CLI tool'"
        });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("test via cli"));
    }

    #[tokio::test]
    async fn test_execute_missing_command() {
        let t = tool();
        let args = serde_json::json!({});
        let result = t.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_command() {
        let t = tool();
        let args = serde_json::json!({"command": "invalid_subcommand"});
        let result = t.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_full_flow() {
        let store = Arc::new(InMemoryTaskStore::new());
        let t = TaskCliTool::new(store.clone());
        let ctx = ToolContext::default();

        // Create
        let r = t
            .execute(
                &serde_json::json!({"command": "+task --name 'e2e test'"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(r.success);

        // Get
        let r = t
            .execute(&serde_json::json!({"command": "get --id 1"}), &ctx)
            .await
            .unwrap();
        assert!(r.content.contains("e2e test"));

        // Update
        let r = t
            .execute(
                &serde_json::json!({"command": "update --id 1 --status completed"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(r.success);

        // Verify
        let task = store.get(&crate::model::TaskId(1)).await.unwrap().unwrap();
        assert_eq!(task.status, crate::model::TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_sensitivity_safe_for_read() {
        let t = tool();
        let s = t.sensitivity(&serde_json::json!({"command": "get --id 1"}));
        assert!(matches!(s, ToolSensitivity::Safe));
    }

    #[tokio::test]
    async fn test_sensitivity_approval_for_mutate() {
        let t = tool();
        let s = t.sensitivity(&serde_json::json!({"command": "update --id 1 --status completed"}));
        assert!(matches!(s, ToolSensitivity::RequiresApproval { .. }));
    }

    #[tokio::test]
    async fn test_json_output() {
        let t = tool();
        let args = serde_json::json!({
            "command": "create --name 'json test' --desc 'test' --json"
        });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.starts_with('{'));
    }
}
