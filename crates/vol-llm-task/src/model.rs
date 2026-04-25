//! Task data models.

use std::path::PathBuf;
use std::time::SystemTime;

/// Unique task identifier (newtype over String for type safety).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TaskId(pub String);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Generate a new task ID with "t" prefix + UUID
fn generate_task_id() -> TaskId {
    let id = format!("t{}", uuid::Uuid::new_v4().simple()).chars().take(9).collect::<String>();
    TaskId(id)
}

/// Task lifecycle status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub enum TaskStatus {
    /// Created, waiting for dependencies
    Pending,
    /// Currently executing
    Running,
    /// Successfully completed
    Completed,
    /// Execution failed
    Failed,
    /// Explicitly terminated
    Killed,
}

/// Type of task
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TaskKind {
    /// Sub-agent task
    Agent,
    /// Manual marker (for todo splitting)
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
    pub description: String,
    pub dependencies: Vec<TaskId>,
    pub result: Option<TaskResult>,
    pub summary: Option<String>,
    pub output_file: Option<PathBuf>,
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
}

impl Task {
    /// Create a new pending task
    pub fn new(kind: TaskKind, description: String, dependencies: Vec<TaskId>) -> Self {
        Self {
            id: generate_task_id(),
            status: TaskStatus::Pending,
            kind,
            description,
            dependencies,
            result: None,
            summary: None,
            output_file: None,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
        }
    }
}
