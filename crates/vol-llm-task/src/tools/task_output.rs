//! TaskOutput tool — reads a task's output file.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolResultType};

use crate::model::TaskId;
use crate::store::TaskStore;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TaskOutputParams {
    task_id: String,
    #[serde(default = "default_block")]
    block: bool,
    #[serde(default = "default_timeout")]
    timeout: u64,
}

fn default_block() -> bool {
    true
}

fn default_timeout() -> u64 {
    30000
}

pub struct TaskOutput {
    store: Arc<dyn TaskStore>,
}

impl TaskOutput {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskOutput {
    fn name(&self) -> &'static str {
        "task_output"
    }

    fn description(&self) -> &'static str {
        "Reads a task's output file. Use this to see the full output of a completed or running task."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to get output from"
                },
                "block": {
                    "type": "boolean",
                    "description": "Whether to wait for completion (not supported for file-based tasks, always returns immediately)",
                    "default": true
                },
                "timeout": {
                    "type": "integer",
                    "description": "Max wait time in ms (not supported for file-based tasks)",
                    "default": 30000
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let _params: TaskOutputParams = serde_json::from_value(args.clone()).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        let task_id_str = args
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                vol_llm_tool::ToolError::InvalidArguments(
                    "Missing required parameter: task_id".to_string(),
                )
            })?;

        let task_id: TaskId = task_id_str.parse::<u64>().map(TaskId).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Invalid task ID: {}", e))
        })?;

        let task = self.store.get(&task_id).await.map_err(|e| {
            vol_llm_tool::ToolError::ExecutionFailed(format!("Failed to get task: {}", e))
        })?;

        let task = match task {
            Some(t) => t,
            None => {
                return Ok(ToolResult::failure(format!(
                    "Task #{} not found",
                    task_id.0
                )));
            }
        };

        let output_path = match &task.result {
            Some(result) => &result.output_file,
            None => {
                return Ok(ToolResult::failure(format!(
                    "Task #{} has no output file",
                    task_id.0
                )));
            }
        };

        let content = match tokio::fs::read_to_string(output_path).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult::failure(format!(
                    "Failed to read output file: {}",
                    e
                )));
            }
        };

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let display_lines = lines.iter().rev().take(2000).rev().collect::<Vec<_>>();
        let truncated = total_lines > 2000;

        let output = display_lines
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:5}  |  {}", i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult {
            success: true,
            content: if truncated {
                format!("Showing last 2000 of {} lines", total_lines)
            } else {
                format!("{} lines", total_lines)
            },
            error: None,
            data: Some(serde_json::json!({
                "task_id": task_id.0.to_string(),
                "output": output,
                "total_lines": total_lines,
                "truncated": truncated
            })),
            call_id: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Task, TaskId, TaskKind, TaskResult, TaskStatus};
    use crate::stores::InMemoryTaskStore;
    use vol_llm_tool::ExecutableTool;

    fn tool() -> TaskOutput {
        TaskOutput::new(Arc::new(InMemoryTaskStore::new()))
    }

    #[tokio::test]
    async fn test_output_reads_file() {
        let store = Arc::new(InMemoryTaskStore::new());

        let output_path = "/tmp/vol-task-output-test.txt";
        tokio::fs::write(output_path, "line 1\nline 2\nline 3\n")
            .await
            .unwrap();

        let mut task = Task::new(TaskKind::Agent, "test".to_string(), vec![]);
        task.id = TaskId(1);
        task.status = TaskStatus::Completed;
        task.result = Some(TaskResult {
            success: true,
            output_truncated: "line 1\nline 2\nline 3\n".to_string(),
            output_file: output_path.into(),
        });
        store.create(task).await.unwrap();

        let t = TaskOutput::new(store);
        let args = serde_json::json!({ "task_id": "1" });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        let data = result.data.unwrap();
        let output = data.get("output").unwrap().as_str().unwrap();
        assert!(output.contains("line 1"));
        assert!(output.contains("line 2"));

        let _ = tokio::fs::remove_file(output_path).await;
    }

    #[tokio::test]
    async fn test_output_task_not_found() {
        let t = tool();
        let args = serde_json::json!({ "task_id": "999" });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_output_no_output_file() {
        let store = Arc::new(InMemoryTaskStore::new());
        store
            .create(Task::new(TaskKind::Agent, "no output".to_string(), vec![]))
            .await
            .unwrap();

        let t = TaskOutput::new(store);
        let args = serde_json::json!({ "task_id": "1" });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(!result.success);
    }
}
