//! Task data models.

use std::path::PathBuf;
use std::time::SystemTime;

/// Unique task identifier (newtype over u64, auto-increment).
///
/// Canonical serialized form is a decimal string: `"1"`. Deserialization also
/// accepts a bare integer (data written before the representation was
/// unified) and a single `t` prefix (what models were shown historically).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u64);

/// Error returned when a string cannot be parsed as a [`TaskId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTaskIdError(String);

impl std::fmt::Display for ParseTaskIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid task id: {:?}", self.0)
    }
}

impl std::error::Error for ParseTaskIdError {}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TaskId {
    type Err = ParseTaskIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Strip at most one leading 't'. The previous implementation used
        // trim_start_matches('t'), which accepted "ttt1".
        let digits = s.strip_prefix('t').unwrap_or(s);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ParseTaskIdError(s.to_string()));
        }
        digits
            .parse::<u64>()
            .map(TaskId)
            .map_err(|_| ParseTaskIdError(s.to_string()))
    }
}

impl serde::Serialize for TaskId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for TaskId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TaskIdVisitor;

        impl serde::de::Visitor<'_> for TaskIdVisitor {
            type Value = TaskId;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a task id as a decimal string or an unsigned integer")
            }

            fn visit_u64<E>(self, v: u64) -> Result<TaskId, E>
            where
                E: serde::de::Error,
            {
                Ok(TaskId(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<TaskId, E>
            where
                E: serde::de::Error,
            {
                u64::try_from(v)
                    .map(TaskId)
                    .map_err(|_| E::custom(format!("negative task id: {v}")))
            }

            fn visit_str<E>(self, v: &str) -> Result<TaskId, E>
            where
                E: serde::de::Error,
            {
                use std::str::FromStr;
                TaskId::from_str(v).map_err(E::custom)
            }
        }

        // deserialize_any is required to accept both the number and string
        // forms. This works for JSON; TaskId is never sent through a
        // non-self-describing format in this workspace.
        deserializer.deserialize_any(TaskIdVisitor)
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
        assert_eq!(format!("{}", id), "42");
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

    #[test]
    fn test_task_id_serializes_as_string() {
        assert_eq!(serde_json::to_string(&TaskId(1)).unwrap(), "\"1\"");
        assert_eq!(serde_json::to_string(&TaskId(0)).unwrap(), "\"0\"");
        assert_eq!(
            serde_json::to_string(&TaskId(u64::MAX)).unwrap(),
            format!("\"{}\"", u64::MAX)
        );
    }

    #[test]
    fn test_task_id_deserializes_from_legacy_number() {
        // Rows written before this change hold bare integers.
        assert_eq!(serde_json::from_str::<TaskId>("1").unwrap(), TaskId(1));
        assert_eq!(
            serde_json::from_str::<Vec<TaskId>>("[1,2]").unwrap(),
            vec![TaskId(1), TaskId(2)]
        );
    }

    #[test]
    fn test_task_id_deserializes_from_canonical_string() {
        assert_eq!(serde_json::from_str::<TaskId>("\"1\"").unwrap(), TaskId(1));
        assert_eq!(
            serde_json::from_str::<Vec<TaskId>>("[\"1\",\"2\"]").unwrap(),
            vec![TaskId(1), TaskId(2)]
        );
    }

    #[test]
    fn test_task_id_deserializes_from_prefixed_string() {
        // Historical: models were shown "t1" for a long time.
        assert_eq!(serde_json::from_str::<TaskId>("\"t1\"").unwrap(), TaskId(1));
    }

    #[test]
    fn test_task_id_rejects_malformed() {
        assert!(serde_json::from_str::<TaskId>("\"ttt1\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\"\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\"t\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\"abc\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\"-1\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\" 1\"").is_err());
        assert!(serde_json::from_str::<TaskId>("-1").is_err());
    }

    #[test]
    fn test_task_id_round_trip() {
        for raw in [0u64, 1, 42, u64::MAX] {
            let json = serde_json::to_string(&TaskId(raw)).unwrap();
            assert_eq!(serde_json::from_str::<TaskId>(&json).unwrap(), TaskId(raw));
        }
    }

    #[test]
    fn test_task_id_from_str() {
        use std::str::FromStr;
        assert_eq!(TaskId::from_str("1").unwrap(), TaskId(1));
        assert_eq!(TaskId::from_str("t1").unwrap(), TaskId(1));
        assert_eq!(TaskId::from_str("t42").unwrap(), TaskId(42));
        assert!(TaskId::from_str("ttt1").is_err());
        assert!(TaskId::from_str("").is_err());
        assert!(TaskId::from_str("t").is_err());
    }

    #[test]
    fn test_task_id_rejects_wrong_json_type() {
        // Exercises the visitor's `expecting` message on non-string,
        // non-integer input.
        let err = serde_json::from_str::<TaskId>("true")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("a task id as a decimal string or an unsigned integer"),
            "unexpected message: {err}"
        );
        assert!(serde_json::from_str::<TaskId>("null").is_err());
        assert!(serde_json::from_str::<TaskId>("{}").is_err());
    }

    #[test]
    fn test_parse_task_id_error_is_a_std_error_with_the_offending_input() {
        use std::str::FromStr;
        let err = TaskId::from_str("ttt1").unwrap_err();
        assert_eq!(err.to_string(), "invalid task id: \"ttt1\"");
        let as_std_error: &dyn std::error::Error = &err;
        assert_eq!(as_std_error.to_string(), "invalid task id: \"ttt1\"");
    }
}
