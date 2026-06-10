//! CLI text and JSON output formatting for task commands.

use crate::model::{Task, TaskId};

/// Format a single task as a human-readable detail block.
pub(crate) fn fmt_task_detail(task: &Task) -> String {
    let mut out = format!(
        "Task {}: \"{}\"\n\
         Status:       {:?}\n\
         Assignee:     {}\n\
         Created:      {:?}\n\
         Dependencies: {}\n\
         Blocks:       {}",
        task.id,
        task.subject,
        task.status,
        task.assignee.as_deref().unwrap_or("-"),
        task.created_at,
        fmt_task_ids(&task.dependencies),
        fmt_task_ids(&task.blocks),
    );
    if !task.description.is_empty() {
        out.push_str(&format!("\nDesc:         {}", task.description));
    }
    if let Some(ref af) = task.active_form {
        out.push_str(&format!("\nActiveForm:   {}", af));
    }
    out
}

/// Format a list of tasks as a compact table.
pub(crate) fn fmt_task_list(tasks: &[Task]) -> String {
    if tasks.is_empty() {
        return "No tasks found.".to_string();
    }
    let mut out = format!("{} task(s):\n", tasks.len());
    for task in tasks {
        out.push_str(&format!(
            "  {}  \"{}\"  [{:?}]  {}\n",
            task.id,
            task.subject,
            task.status,
            task.assignee.as_deref().unwrap_or("-"),
        ));
    }
    out.trim_end().to_string()
}

/// Format a create confirmation.
pub(crate) fn fmt_create_confirm(task: &Task) -> String {
    format!(
        "Task {} created: \"{}\" [{:?}] assignee: {}",
        task.id,
        task.subject,
        task.status,
        task.assignee.as_deref().unwrap_or("-"),
    )
}

/// Format a scheme (parameter list) for a specific subcommand.
pub(crate) fn fmt_scheme(subcommand: &str, params: &[(&str, bool, &str)]) -> String {
    let mut out = format!("{} parameters:\n", subcommand);
    for (name, required, desc) in params {
        let req = if *required {
            "(required)"
        } else {
            "(optional)"
        };
        out.push_str(&format!("  --{:<14} {:<10} {}\n", name, req, desc));
    }
    out.trim_end().to_string()
}

/// Serialize a value to JSON string, with error fallback.
pub(crate) fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
}

fn fmt_task_ids(ids: &[TaskId]) -> String {
    if ids.is_empty() {
        "-".to_string()
    } else {
        ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Task, TaskId, TaskKind, TaskStatus};
    use std::time::SystemTime;

    fn sample_task() -> Task {
        Task {
            id: TaskId(42),
            status: TaskStatus::Pending,
            kind: TaskKind::Agent,
            publisher: Some("test-agent".into()),
            assignee: Some("coding-agent".into()),
            subject: "fix login bug".into(),
            description: "handle OAuth callback".into(),
            active_form: Some("Fixing login bug".into()),
            dependencies: vec![TaskId(1), TaskId(2)],
            blocks: vec![TaskId(50)],
            result: None,
            summary: None,
            output_file: None,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn test_fmt_task_detail() {
        let output = fmt_task_detail(&sample_task());
        assert!(output.contains("Task t42"));
        assert!(output.contains("fix login bug"));
        assert!(output.contains("Pending"));
        assert!(output.contains("coding-agent"));
        assert!(output.contains("t1, t2"));
        assert!(output.contains("t50"));
        assert!(output.contains("handle OAuth callback"));
    }

    #[test]
    fn test_fmt_task_list_empty() {
        assert_eq!(fmt_task_list(&[]), "No tasks found.");
    }

    #[test]
    fn test_fmt_task_list_nonempty() {
        let tasks = vec![sample_task()];
        let output = fmt_task_list(&tasks);
        assert!(output.contains("1 task(s)"));
        assert!(output.contains("t42"));
        assert!(output.contains("fix login bug"));
    }

    #[test]
    fn test_fmt_create_confirm() {
        let output = fmt_create_confirm(&sample_task());
        assert!(output.contains("Task t42 created"));
        assert!(output.contains("fix login bug"));
    }

    #[test]
    fn test_fmt_scheme() {
        let params = vec![
            ("name", true, "Task subject"),
            ("desc", true, "Task description"),
            ("assignee", false, "Agent type"),
        ];
        let output = fmt_scheme("create", &params);
        assert!(output.contains("create parameters"));
        assert!(output.contains("--name"));
        assert!(output.contains("(required)"));
        assert!(output.contains("(optional)"));
    }

    #[test]
    fn test_to_json() {
        let json = to_json(&sample_task());
        assert!(json.contains("\"fix login bug\""));
        assert!(json.starts_with('{'));
    }
}
