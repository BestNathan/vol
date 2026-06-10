//! Task data models.

use std::path::PathBuf;
use std::time::SystemTime;

/// Unique task identifier (newtype over u64, auto-increment).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct TaskId(pub u64);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "t{}", self.0)
    }
}

/// Task lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Killed,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Killed => write!(f, "killed"),
        }
    }
}

/// Type of task
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TaskKind {
    Agent,
    Manual,
}

/// Task execution result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskResult {
    pub success: bool,
    pub output_truncated: String,
    pub output_file: PathBuf,
}

/// A managed task
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub status: TaskStatus,
    pub kind: TaskKind,
    pub publisher: Option<String>,
    pub assignee: Option<String>,
    pub subject: String,
    pub description: String,
    pub active_form: Option<String>,
    pub dependencies: Vec<TaskId>,
    pub blocks: Vec<TaskId>,
    pub result: Option<TaskResult>,
    pub summary: Option<String>,
    pub output_file: Option<PathBuf>,
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
}

impl Task {
    /// Create a new pending task. Caller must set the id (store assigns it).
    pub fn new(kind: TaskKind, subject: String, dependencies: Vec<TaskId>) -> Self {
        Self {
            id: TaskId(0),
            status: TaskStatus::Pending,
            kind,
            publisher: None,
            assignee: None,
            subject,
            description: String::new(),
            active_form: None,
            dependencies,
            blocks: Vec::new(),
            result: None,
            summary: None,
            output_file: None,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_display() {
        let id = TaskId(42);
        assert_eq!(format!("{}", id), "t42");
    }

    #[test]
    fn test_next_task_id_empty() {
        let ids: Vec<u64> = vec![];
        let next = ids.iter().max().map_or(1, |m| m + 1);
        assert_eq!(next, 1);
    }

    #[test]
    fn test_next_task_id_with_existing() {
        let ids: Vec<u64> = vec![1, 3, 2];
        let next = ids.iter().max().map_or(1, |m| m + 1);
        assert_eq!(next, 4);
    }

    #[test]
    fn test_task_id_copy() {
        let a = TaskId(5);
        let b = a;
        assert_eq!(a.0, b.0);
    }
}
