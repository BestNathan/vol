//! Task CLI executor — execute ParsedCommand against TaskStore.
//!
//! This is the core execution module.  It takes a [`ParsedCommand`] enum,
//! executes it against a [`TaskStore`], applies smart defaults for
//! shortcut commands, and formats the output.

use std::sync::Arc;

use vol_llm_tool::{ToolContext, ToolResult};

use super::commands::ParsedCommand;
use crate::model::{Task, TaskId, TaskKind, TaskStatus};
use crate::store::TaskStore;

/// Execute a parsed command against the task store.
pub(crate) async fn execute(
    cmd: ParsedCommand,
    store: &Arc<dyn TaskStore>,
    context: &ToolContext,
) -> Result<ToolResult, String> {
    match cmd {
        ParsedCommand::Create {
            name,
            desc,
            assignee,
            active_form,
            deps,
            blocks,
            json,
        } => {
            let mut task = Task::new(
                TaskKind::Agent,
                name.clone(),
                deps.into_iter().map(TaskId).collect(),
            );
            task.description = desc;
            task.active_form = active_form;
            task.assignee = assignee;
            task.blocks = blocks.into_iter().map(TaskId).collect();

            let id = store
                .create(task)
                .await
                .map_err(|e| format!("Failed to create task: {e}"))?;
            let created = store
                .get(&id)
                .await
                .map_err(|e| format!("Failed to read task: {e}"))?
                .ok_or_else(|| "Task not found after creation".to_string())?;

            let content = if json {
                super::format::to_json(&created)
            } else {
                super::format::fmt_create_confirm(&created)
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(to_value(&created)),
                call_id: String::new(),
            })
        }

        ParsedCommand::Update {
            id,
            status,
            subject,
            desc,
            assignee,
            active_form,
            add_deps,
            add_blocks,
            json,
        } => {
            let task_id = TaskId(id);
            let task = store
                .get(&task_id)
                .await
                .map_err(|e| format!("Failed to get task: {e}"))?
                .ok_or_else(|| format!("Task {task_id} not found"))?;

            let mut task = task;
            let mut updated = Vec::new();

            if let Some(s) = subject {
                task.subject = s;
                updated.push("subject");
            }
            if let Some(d) = desc {
                task.description = d;
                updated.push("description");
            }
            if let Some(a) = assignee {
                task.assignee = Some(a);
                updated.push("assignee");
            }
            if let Some(af) = active_form {
                task.active_form = Some(af);
                updated.push("activeForm");
            }
            if let Some(s) = status {
                task.status = parse_status(&s)?;
                updated.push("status");
            }
            if !add_deps.is_empty() {
                for dep_id in add_deps {
                    let tid = TaskId(dep_id);
                    if !task.dependencies.contains(&tid) {
                        task.dependencies.push(tid);
                    }
                }
                updated.push("dependencies");
            }
            if !add_blocks.is_empty() {
                for block_id in add_blocks {
                    let tid = TaskId(block_id);
                    if !task.blocks.contains(&tid) {
                        task.blocks.push(tid);
                    }
                }
                updated.push("blocks");
            }

            store
                .update(task)
                .await
                .map_err(|e| format!("Failed to update task: {e}"))?;

            let updated_task = store
                .get(&task_id)
                .await
                .map_err(|e| format!("Failed to read task: {e}"))?
                .ok_or_else(|| "Task not found after update".to_string())?;

            let content = if json {
                super::format::to_json(&updated_task)
            } else {
                format!("Task {} updated: {}", task_id, updated.join(", "))
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(serde_json::json!({
                    "taskId": task_id.to_string(),
                    "updatedFields": updated,
                })),
                call_id: String::new(),
            })
        }

        ParsedCommand::Get { id, json } => {
            let task_id = TaskId(id);
            let task = store
                .get(&task_id)
                .await
                .map_err(|e| format!("Failed to get task: {e}"))?
                .ok_or_else(|| format!("Task {task_id} not found"))?;

            let content = if json {
                super::format::to_json(&task)
            } else {
                super::format::fmt_task_detail(&task)
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(to_value(&task)),
                call_id: String::new(),
            })
        }

        ParsedCommand::List {
            status,
            assignee,
            json,
        } => {
            let status_filter = status.map(|s| parse_status(&s)).transpose()?;
            let all = store
                .list(status_filter)
                .await
                .map_err(|e| format!("Failed to list tasks: {e}"))?;

            let tasks: Vec<Task> = if let Some(ref a) = assignee {
                all.into_iter()
                    .filter(|t| t.assignee.as_deref() == Some(a.as_str()))
                    .collect()
            } else {
                all
            };

            let content = if json {
                super::format::to_json(&tasks)
            } else {
                super::format::fmt_task_list(&tasks)
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(to_value(&tasks)),
                call_id: String::new(),
            })
        }

        ParsedCommand::Stop { id, json } => {
            let task_id = TaskId(id);
            let task = store
                .get(&task_id)
                .await
                .map_err(|e| format!("Failed to get task: {e}"))?
                .ok_or_else(|| format!("Task {task_id} not found"))?;

            let mut task = task;
            task.status = TaskStatus::Killed;
            store
                .update(task)
                .await
                .map_err(|e| format!("Failed to stop task: {e}"))?;

            let content = if json {
                format!("{{\"success\": true, \"taskId\": \"{id}\", \"status\": \"killed\"}}")
            } else {
                format!("Task {task_id} stopped (killed)")
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(serde_json::json!({"taskId": id.to_string(), "status": "killed"})),
                call_id: String::new(),
            })
        }

        ParsedCommand::Output {
            id,
            block,
            timeout_ms,
            json: _,
        } => {
            let task_id = TaskId(id);

            if block {
                let start = std::time::Instant::now();
                loop {
                    let task = store
                        .get(&task_id)
                        .await
                        .map_err(|e| format!("Failed to get task: {e}"))?
                        .ok_or_else(|| format!("Task {task_id} not found"))?;

                    match task.status {
                        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Killed => break,
                        _ => {
                            #[allow(clippy::cast_possible_truncation)]
                            let elapsed_ms = start.elapsed().as_millis() as u64;
                            if elapsed_ms >= timeout_ms {
                                return Ok(ToolResult {
                                    success: false,
                                    content: format!(
                                        "Timeout waiting for task {task_id} ({timeout_ms}ms)"
                                    ),
                                    error: Some("timeout".to_string()),
                                    data: Some(serde_json::json!({
                                        "taskId": id.to_string(),
                                        "status": task.status.to_string()
                                    })),
                                    call_id: String::new(),
                                });
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        }
                    }
                }
            }

            // Read output
            let task = store
                .get(&task_id)
                .await
                .map_err(|e| format!("Failed to get task: {e}"))?
                .ok_or_else(|| format!("Task {task_id} not found"))?;

            match (task.output_file.as_ref(), task.result.as_ref()) {
                (Some(path), _) => match tokio::fs::read_to_string(path).await {
                    Ok(output) => Ok(ToolResult {
                        success: true,
                        content: output,
                        error: None,
                        data: Some(serde_json::json!({"taskId": id.to_string()})),
                        call_id: String::new(),
                    }),
                    Err(e) => Ok(ToolResult {
                        success: false,
                        content: format!("Failed to read output file: {e}"),
                        error: Some(format!("IO error: {e}")),
                        data: Some(serde_json::json!({"taskId": id.to_string()})),
                        call_id: String::new(),
                    }),
                },
                (_, Some(result)) => Ok(ToolResult {
                    success: true,
                    content: result.output_truncated.clone(),
                    error: None,
                    data: Some(serde_json::json!({"taskId": id.to_string()})),
                    call_id: String::new(),
                }),
                _ => Ok(ToolResult {
                    success: false,
                    content: format!("No output for task {task_id}"),
                    error: Some("No output available".to_string()),
                    data: Some(serde_json::json!({"taskId": id.to_string()})),
                    call_id: String::new(),
                }),
            }
        }

        ParsedCommand::Claim { id, json } => {
            let task = if let Some(tid) = id {
                let task_id = TaskId(tid);
                store
                    .get(&task_id)
                    .await
                    .map_err(|e| format!("Failed to get task: {e}"))?
                    .ok_or_else(|| format!("Task {task_id} not found"))?
            } else {
                let ready = store
                    .get_ready_tasks()
                    .await
                    .map_err(|e| format!("Failed to get ready tasks: {e}"))?;
                let first = ready
                    .first()
                    .ok_or_else(|| "No pending tasks available to claim".to_string())?;
                store
                    .get(first)
                    .await
                    .map_err(|e| format!("Failed to get task: {e}"))?
                    .ok_or_else(|| "Task not found".to_string())?
            };

            if task.status != TaskStatus::Pending {
                return Err(format!("Task {} is already {:?}", task.id, task.status));
            }

            let mut task = task;
            task.status = TaskStatus::Running;
            task.assignee = Some("agent".to_string());
            task.started_at = Some(std::time::SystemTime::now());
            let task_id = task.id;
            store
                .update(task)
                .await
                .map_err(|e| format!("Failed to claim task: {e}"))?;

            let content = if json {
                format!(
                    "{{\"success\": true, \"taskId\": \"{}\", \"status\": \"running\"}}",
                    task_id.0
                )
            } else {
                format!("Task {task_id} claimed and set to Running")
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(serde_json::json!({
                    "taskId": task_id.0.to_string(),
                    "status": "running"
                })),
                call_id: String::new(),
            })
        }

        ParsedCommand::Scheme { subcommand } => {
            let content = scheme_for(subcommand.as_deref());
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: None,
                call_id: String::new(),
            })
        }

        // Shortcuts -- fill defaults then delegate to standard commands
        ParsedCommand::QuickCreate {
            name,
            desc,
            assignee,
            json,
        } => {
            let cmd = ParsedCommand::Create {
                name,
                desc: desc.unwrap_or_default(),
                assignee,
                active_form: None,
                deps: vec![],
                blocks: vec![],
                json,
            };
            Box::pin(execute(cmd, store, context)).await
        }

        ParsedCommand::QuickDone { id, json } => {
            let cmd = ParsedCommand::Update {
                id,
                status: Some("completed".to_string()),
                subject: None,
                desc: None,
                assignee: None,
                active_form: None,
                add_deps: vec![],
                add_blocks: vec![],
                json,
            };
            Box::pin(execute(cmd, store, context)).await
        }

        ParsedCommand::QuickClaim { json } => {
            let cmd = ParsedCommand::Claim { id: None, json };
            Box::pin(execute(cmd, store, context)).await
        }
    }
}

fn parse_status(s: &str) -> Result<TaskStatus, String> {
    match s.to_lowercase().as_str() {
        "pending" => Ok(TaskStatus::Pending),
        "running" => Ok(TaskStatus::Running),
        "completed" => Ok(TaskStatus::Completed),
        "failed" => Ok(TaskStatus::Failed),
        "killed" => Ok(TaskStatus::Killed),
        _ => Err(format!(
            "Invalid status: {s}. Valid: pending, running, completed, failed, killed"
        )),
    }
}

/// Serialize a Task/List to JSON for the `data` field, with manual fallback.
fn to_value<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or_else(|_| serde_json::json!({}))
}

/// Return scheme (parameter definitions) for a subcommand, or list all subcommands.
fn scheme_for(subcommand: Option<&str>) -> String {
    match subcommand {
        Some("create") => super::format::fmt_scheme(
            "create",
            &[
                ("name", true, "Task subject"),
                ("desc", true, "Task description"),
                ("assignee", false, "Agent type to assign"),
                ("activeForm", false, "Spinner display text"),
                ("deps", false, "Comma-separated dependency task IDs"),
                ("blocks", false, "Comma-separated blocking task IDs"),
            ],
        ),
        Some("update") => super::format::fmt_scheme(
            "update",
            &[
                ("id", true, "Task ID to update"),
                (
                    "status",
                    false,
                    "New status: pending|running|completed|failed|killed",
                ),
                ("subject", false, "New subject"),
                ("desc", false, "New description"),
                ("assignee", false, "Reassign to agent type"),
                ("activeForm", false, "Spinner display text"),
                ("addDeps", false, "Add dependencies (comma-separated IDs)"),
                (
                    "addBlocks",
                    false,
                    "Add blocking tasks (comma-separated IDs)",
                ),
            ],
        ),
        Some("get") => super::format::fmt_scheme("get", &[("id", true, "Task ID to retrieve")]),
        Some("list") => super::format::fmt_scheme(
            "list",
            &[
                ("status", false, "Filter by status"),
                ("assignee", false, "Filter by assignee"),
            ],
        ),
        Some("stop") => super::format::fmt_scheme("stop", &[("id", true, "Task ID to stop")]),
        Some("output") => {
            super::format::fmt_scheme("output", &[("id", true, "Task ID to read output from")])
        }
        Some("claim") => super::format::fmt_scheme(
            "claim",
            &[("id", false, "Task ID to claim (omit to claim first ready)")],
        ),
        Some("+task") => super::format::fmt_scheme(
            "+task",
            &[
                ("name", true, "Task subject"),
                ("desc", false, "Task description"),
                ("assignee", false, "Agent type (defaults to current agent)"),
            ],
        ),
        Some("+done") => {
            super::format::fmt_scheme("+done", &[("id", true, "Task ID to mark completed")])
        }
        Some("+claim") => super::format::fmt_scheme("+claim", &[]),
        _ => {
            let mut out = String::from("Available subcommands:\n");
            for (name, desc) in &[
                ("create", "Create a new task"),
                ("update", "Update a task"),
                ("get", "Get task details"),
                ("list", "List tasks"),
                ("stop", "Stop a running task"),
                ("output", "Read task output"),
                ("claim", "Claim a pending task"),
                ("scheme", "Show parameter definitions"),
                ("+task", "Quick create with smart defaults"),
                ("+done", "Quick complete a task"),
                ("+claim", "Quick claim first ready task"),
            ] {
                out.push_str(&format!("  {name:<12} {desc}\n"));
            }
            out.push_str("\nUse 'task scheme <subcommand>' for detailed parameters.");
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::InMemoryTaskStore;

    fn store() -> Arc<dyn TaskStore> {
        Arc::new(InMemoryTaskStore::new())
    }

    fn ctx() -> ToolContext {
        ToolContext::default()
    }

    #[tokio::test]
    async fn test_execute_create() {
        let s = store();
        let cmd = ParsedCommand::Create {
            name: "test task".into(),
            desc: "do something".into(),
            assignee: None,
            active_form: None,
            deps: vec![],
            blocks: vec![],
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("test task"));

        let tasks = s.list(None).await.unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_execute_create_json() {
        let s = store();
        let cmd = ParsedCommand::Create {
            name: "json task".into(),
            desc: "test".into(),
            assignee: None,
            active_form: None,
            deps: vec![],
            blocks: vec![],
            json: true,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.starts_with('{'));
    }

    #[tokio::test]
    async fn test_execute_get() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "find me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::Get {
            id: id.0,
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("find me"));
    }

    #[tokio::test]
    async fn test_execute_get_not_found() {
        let s = store();
        let cmd = ParsedCommand::Get {
            id: 999,
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_execute_list() {
        let s = store();
        s.create(Task::new(TaskKind::Agent, "task a".into(), vec![]))
            .await
            .unwrap();
        s.create(Task::new(TaskKind::Agent, "task b".into(), vec![]))
            .await
            .unwrap();

        let cmd = ParsedCommand::List {
            status: None,
            assignee: None,
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("task a"));
        assert!(result.content.contains("task b"));
    }

    #[tokio::test]
    async fn test_execute_list_filter_status() {
        let s = store();
        s.create(Task::new(TaskKind::Agent, "pending task".into(), vec![]))
            .await
            .unwrap();
        let cmd = ParsedCommand::List {
            status: Some("completed".into()),
            assignee: None,
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.content.contains("No tasks found"));
    }

    #[tokio::test]
    async fn test_execute_update_status() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "update me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::Update {
            id: id.0,
            status: Some("completed".into()),
            subject: None,
            desc: None,
            assignee: None,
            active_form: None,
            add_deps: vec![],
            add_blocks: vec![],
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let updated = s.get(&id).await.unwrap().unwrap();
        assert_eq!(updated.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_execute_stop() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "kill me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::Stop {
            id: id.0,
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let stopped = s.get(&id).await.unwrap().unwrap();
        assert_eq!(stopped.status, TaskStatus::Killed);
    }

    #[tokio::test]
    async fn test_execute_claim() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "claim me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::Claim {
            id: Some(id.0),
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let claimed = s.get(&id).await.unwrap().unwrap();
        assert_eq!(claimed.status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_execute_quick_create() {
        let s = store();
        let cmd = ParsedCommand::QuickCreate {
            name: "quick one".into(),
            desc: None,
            assignee: None,
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("quick one"));
    }

    #[tokio::test]
    async fn test_execute_quick_done() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "finish me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::QuickDone {
            id: id.0,
            json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let done = s.get(&id).await.unwrap().unwrap();
        assert_eq!(done.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_execute_quick_claim() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "first ready".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::QuickClaim { json: false };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let claimed = s.get(&id).await.unwrap().unwrap();
        assert_eq!(claimed.status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_execute_scheme_create() {
        let s = store();
        let cmd = ParsedCommand::Scheme {
            subcommand: Some("create".into()),
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.content.contains("--name"));
        assert!(result.content.contains("required"));
    }

    #[tokio::test]
    async fn test_execute_scheme_all() {
        let s = store();
        let cmd = ParsedCommand::Scheme { subcommand: None };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.content.contains("create"));
        assert!(result.content.contains("+task"));
    }

    #[tokio::test]
    async fn test_parse_status_invalid() {
        assert!(parse_status("bogus").is_err());
    }
}
