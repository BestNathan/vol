//! SQLx-backed database task store.

use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::model::{Task, TaskId, TaskKind, TaskResult, TaskStatus};
use crate::store::{Result, StoreError, TaskStore};

static SQLITE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/sqlite");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatabaseBackend {
    Sqlite,
    Postgres,
    MySql,
}

fn infer_backend(url: &str) -> Result<DatabaseBackend> {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .unwrap_or_default();

    match scheme {
        "sqlite" => Ok(DatabaseBackend::Sqlite),
        "postgres" | "postgresql" => Ok(DatabaseBackend::Postgres),
        "mysql" => Ok(DatabaseBackend::MySql),
        "" => Err(StoreError::Database(
            "unsupported task store database url scheme: <missing>".to_string(),
        )),
        other => Err(StoreError::Database(format!(
            "unsupported task store database url scheme: {other}"
        ))),
    }
}

pub struct DatabaseTaskStore {
    pool: SqlitePool,
}

impl DatabaseTaskStore {
    pub async fn connect(url: &str) -> Result<Self> {
        match infer_backend(url)? {
            DatabaseBackend::Sqlite => Self::connect_sqlite(url).await,
            DatabaseBackend::Postgres => Err(StoreError::Database(
                "database task store backend is recognized but not enabled yet: postgres"
                    .to_string(),
            )),
            DatabaseBackend::MySql => Err(StoreError::Database(
                "database task store backend is recognized but not enabled yet: mysql".to_string(),
            )),
        }
    }

    async fn connect_sqlite(url: &str) -> Result<Self> {
        create_sqlite_parent_dir(url)?;
        let options = SqliteConnectOptions::from_str(url)
            .map_err(|e| StoreError::Database(format!("invalid sqlite task store url: {e}")))?
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| {
                StoreError::Database(format!("failed to connect sqlite task store: {e}"))
            })?;
        SQLITE_MIGRATOR.run(&pool).await.map_err(|e| {
            StoreError::Database(format!("failed to migrate sqlite task store: {e}"))
        })?;
        Ok(Self { pool })
    }
}

fn create_sqlite_parent_dir(url: &str) -> Result<()> {
    let path = sqlite_file_path(url);
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(StoreError::Io)?;
            }
        }
    }
    Ok(())
}

fn sqlite_file_path(url: &str) -> Option<PathBuf> {
    if url == "sqlite::memory:" || url == "sqlite://:memory:" {
        return None;
    }
    let raw = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))?;
    if raw.is_empty() || raw == ":memory:" {
        return None;
    }
    Some(PathBuf::from(raw))
}

fn status_to_db(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Killed => "killed",
    }
}

fn status_from_db(value: &str) -> Result<TaskStatus> {
    match value {
        "pending" => Ok(TaskStatus::Pending),
        "running" => Ok(TaskStatus::Running),
        "completed" => Ok(TaskStatus::Completed),
        "failed" => Ok(TaskStatus::Failed),
        "killed" => Ok(TaskStatus::Killed),
        other => Err(StoreError::Serialization(format!(
            "unknown task status: {other}"
        ))),
    }
}

fn kind_to_db(kind: &TaskKind) -> &'static str {
    match kind {
        TaskKind::Agent => "agent",
        TaskKind::Manual => "manual",
    }
}

fn kind_from_db(value: &str) -> Result<TaskKind> {
    match value {
        "agent" => Ok(TaskKind::Agent),
        "manual" => Ok(TaskKind::Manual),
        other => Err(StoreError::Serialization(format!(
            "unknown task kind: {other}"
        ))),
    }
}

fn system_time_to_secs(value: SystemTime) -> Result<i64> {
    let duration = value
        .duration_since(UNIX_EPOCH)
        .map_err(|e| StoreError::Serialization(format!("task time is before unix epoch: {e}")))?;
    Ok(duration.as_secs() as i64)
}

fn secs_to_system_time(value: i64) -> Result<SystemTime> {
    let secs = u64::try_from(value)
        .map_err(|_| StoreError::Serialization(format!("negative task timestamp: {value}")))?;
    Ok(UNIX_EPOCH + Duration::from_secs(secs))
}

fn option_time_to_secs(value: Option<SystemTime>) -> Result<Option<i64>> {
    value.map(system_time_to_secs).transpose()
}

fn option_secs_to_time(value: Option<i64>) -> Result<Option<SystemTime>> {
    value.map(secs_to_system_time).transpose()
}

fn task_ids_to_json(ids: &[TaskId]) -> Result<String> {
    serde_json::to_string(ids).map_err(|e| StoreError::Serialization(e.to_string()))
}

fn task_ids_from_json(value: &str) -> Result<Vec<TaskId>> {
    serde_json::from_str(value).map_err(|e| StoreError::Serialization(e.to_string()))
}

fn result_to_json(result: &Option<TaskResult>) -> Result<Option<String>> {
    result
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| StoreError::Serialization(e.to_string()))
}

fn result_from_json(value: Option<String>) -> Result<Option<TaskResult>> {
    value
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|e| StoreError::Serialization(e.to_string()))
}

fn path_to_db(path: &Option<PathBuf>) -> Option<String> {
    path.as_ref().map(|p| p.to_string_lossy().to_string())
}

fn path_from_db(value: Option<String>) -> Option<PathBuf> {
    value.map(PathBuf::from)
}

fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> Result<Task> {
    let id: i64 = row.get("id");
    let status: String = row.get("status");
    let kind: String = row.get("kind");
    let dependencies_json: String = row.get("dependencies_json");
    let blocks_json: String = row.get("blocks_json");
    let result_json: Option<String> = row.get("result_json");
    let created_at_secs: i64 = row.get("created_at_secs");
    let started_at_secs: Option<i64> = row.get("started_at_secs");
    let completed_at_secs: Option<i64> = row.get("completed_at_secs");
    let output_file: Option<String> = row.get("output_file");

    Ok(Task {
        id: TaskId(id as u64),
        status: status_from_db(&status)?,
        kind: kind_from_db(&kind)?,
        publisher: row.get("publisher"),
        assignee: row.get("assignee"),
        subject: row.get("subject"),
        description: row.get("description"),
        active_form: row.get("active_form"),
        dependencies: task_ids_from_json(&dependencies_json)?,
        blocks: task_ids_from_json(&blocks_json)?,
        result: result_from_json(result_json)?,
        summary: row.get("summary"),
        output_file: path_from_db(output_file),
        created_at: secs_to_system_time(created_at_secs)?,
        started_at: option_secs_to_time(started_at_secs)?,
        completed_at: option_secs_to_time(completed_at_secs)?,
    })
}

#[async_trait::async_trait]
impl TaskStore for DatabaseTaskStore {
    async fn create(&self, task: Task) -> Result<TaskId> {
        let dependencies_json = task_ids_to_json(&task.dependencies)?;
        let blocks_json = task_ids_to_json(&task.blocks)?;
        let result_json = result_to_json(&task.result)?;
        let created_at_secs = system_time_to_secs(task.created_at)?;
        let started_at_secs = option_time_to_secs(task.started_at)?;
        let completed_at_secs = option_time_to_secs(task.completed_at)?;
        let output_file = path_to_db(&task.output_file);

        let result = sqlx::query(
            r#"
            INSERT INTO tasks (
                status, kind, publisher, assignee, subject, description, active_form,
                dependencies_json, blocks_json, result_json, summary, output_file,
                created_at_secs, started_at_secs, completed_at_secs
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
        )
        .bind(status_to_db(task.status))
        .bind(kind_to_db(&task.kind))
        .bind(task.publisher)
        .bind(task.assignee)
        .bind(task.subject)
        .bind(task.description)
        .bind(task.active_form)
        .bind(dependencies_json)
        .bind(blocks_json)
        .bind(result_json)
        .bind(task.summary)
        .bind(output_file)
        .bind(created_at_secs)
        .bind(started_at_secs)
        .bind(completed_at_secs)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Database(format!("failed to create task: {e}")))?;

        Ok(TaskId(result.last_insert_rowid() as u64))
    }

    async fn get(&self, task_id: &TaskId) -> Result<Option<Task>> {
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?1")
            .bind(task_id.0 as i64)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get task {}: {e}", task_id)))?;

        row.as_ref().map(row_to_task).transpose()
    }

    async fn update(&self, task: Task) -> Result<()> {
        let dependencies_json = task_ids_to_json(&task.dependencies)?;
        let blocks_json = task_ids_to_json(&task.blocks)?;
        let result_json = result_to_json(&task.result)?;
        let created_at_secs = system_time_to_secs(task.created_at)?;
        let started_at_secs = option_time_to_secs(task.started_at)?;
        let completed_at_secs = option_time_to_secs(task.completed_at)?;
        let output_file = path_to_db(&task.output_file);

        let result = sqlx::query(
            r#"
        UPDATE tasks SET
            status = ?1,
            kind = ?2,
            publisher = ?3,
            assignee = ?4,
            subject = ?5,
            description = ?6,
            active_form = ?7,
            dependencies_json = ?8,
            blocks_json = ?9,
            result_json = ?10,
            summary = ?11,
            output_file = ?12,
            created_at_secs = ?13,
            started_at_secs = ?14,
            completed_at_secs = ?15
        WHERE id = ?16
        "#,
        )
        .bind(status_to_db(task.status))
        .bind(kind_to_db(&task.kind))
        .bind(task.publisher)
        .bind(task.assignee)
        .bind(task.subject)
        .bind(task.description)
        .bind(task.active_form)
        .bind(dependencies_json)
        .bind(blocks_json)
        .bind(result_json)
        .bind(task.summary)
        .bind(output_file)
        .bind(created_at_secs)
        .bind(started_at_secs)
        .bind(completed_at_secs)
        .bind(task.id.0 as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Database(format!("failed to update task {}: {e}", task.id)))?;

        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(format!("Task {}", task.id)));
        }
        Ok(())
    }

    async fn delete(&self, task_id: &TaskId) -> Result<()> {
        sqlx::query("DELETE FROM tasks WHERE id = ?1")
            .bind(task_id.0 as i64)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Database(format!("failed to delete task {}: {e}", task_id)))?;
        Ok(())
    }

    async fn list(&self, status: Option<TaskStatus>) -> Result<Vec<Task>> {
        let rows = if let Some(status) = status {
            sqlx::query("SELECT * FROM tasks WHERE status = ?1 ORDER BY id ASC")
                .bind(status_to_db(status))
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query("SELECT * FROM tasks ORDER BY id ASC")
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|e| StoreError::Database(format!("failed to list tasks: {e}")))?;

        rows.iter().map(row_to_task).collect()
    }

    async fn get_ready_tasks(&self) -> Result<Vec<TaskId>> {
        Err(StoreError::Internal(
            "database task ready query is not implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::SystemTime;

    use super::*;
    use crate::model::{Task, TaskKind, TaskStatus};

    async fn temp_store() -> (DatabaseTaskStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tasks.db");
        let url = format!("sqlite://{}", db_path.display());
        let store = DatabaseTaskStore::connect(&url).await.unwrap();
        (store, dir)
    }

    #[test]
    fn infer_backend_from_sqlite_url() {
        assert_eq!(
            infer_backend("sqlite:///tmp/tasks.db").unwrap(),
            DatabaseBackend::Sqlite
        );
    }

    #[test]
    fn infer_backend_from_postgres_url() {
        assert_eq!(
            infer_backend("postgres://localhost/tasks").unwrap(),
            DatabaseBackend::Postgres
        );
        assert_eq!(
            infer_backend("postgresql://localhost/tasks").unwrap(),
            DatabaseBackend::Postgres
        );
    }

    #[test]
    fn infer_backend_from_mysql_url() {
        assert_eq!(
            infer_backend("mysql://localhost/tasks").unwrap(),
            DatabaseBackend::MySql
        );
    }

    #[test]
    fn infer_backend_rejects_unknown_url() {
        let err = infer_backend("oracle://localhost/tasks").unwrap_err();
        assert!(err
            .to_string()
            .contains("unsupported task store database url scheme: oracle"));
    }

    #[tokio::test]
    async fn create_assigns_id_and_get_retrieves_task() {
        let (store, _dir) = temp_store().await;
        let mut task = Task::new(TaskKind::Agent, "database task".to_string(), vec![]);
        task.description = "stored in sqlite".to_string();
        task.publisher = Some("planner".to_string());
        task.assignee = Some("worker".to_string());
        task.active_form = Some("Working".to_string());

        let id = store.create(task).await.unwrap();
        assert_eq!(id.0, 1);

        let got = store.get(&id).await.unwrap().unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.subject, "database task");
        assert_eq!(got.description, "stored in sqlite");
        assert_eq!(got.publisher.as_deref(), Some("planner"));
        assert_eq!(got.assignee.as_deref(), Some("worker"));
        assert_eq!(got.active_form.as_deref(), Some("Working"));
        assert_eq!(got.status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn update_persists_all_mutable_fields() {
        let (store, _dir) = temp_store().await;
        let id = store
            .create(Task::new(TaskKind::Agent, "original".to_string(), vec![]))
            .await
            .unwrap();

        let mut task = store.get(&id).await.unwrap().unwrap();
        task.status = TaskStatus::Completed;
        task.subject = "updated".to_string();
        task.description = "done".to_string();
        task.summary = Some("summary".to_string());
        task.output_file = Some(PathBuf::from("/tmp/output.txt"));
        task.result = Some(crate::model::TaskResult {
            success: true,
            output_truncated: "ok".to_string(),
            output_file: PathBuf::from("/tmp/result.txt"),
        });
        task.completed_at = Some(SystemTime::now());

        store.update(task).await.unwrap();

        let got = store.get(&id).await.unwrap().unwrap();
        assert_eq!(got.status, TaskStatus::Completed);
        assert_eq!(got.subject, "updated");
        assert_eq!(got.description, "done");
        assert_eq!(got.summary.as_deref(), Some("summary"));
        assert_eq!(
            got.output_file.as_deref(),
            Some(std::path::Path::new("/tmp/output.txt"))
        );
        assert_eq!(got.result.as_ref().unwrap().success, true);
        assert!(got.completed_at.is_some());
    }

    #[tokio::test]
    async fn delete_removes_task_and_missing_delete_succeeds() {
        let (store, _dir) = temp_store().await;
        let id = store
            .create(Task::new(TaskKind::Agent, "delete".to_string(), vec![]))
            .await
            .unwrap();

        store.delete(&id).await.unwrap();
        assert!(store.get(&id).await.unwrap().is_none());
        store.delete(&id).await.unwrap();
    }

    #[tokio::test]
    async fn list_returns_tasks_in_id_order_and_filters_status() {
        let (store, _dir) = temp_store().await;
        let id1 = store
            .create(Task::new(TaskKind::Agent, "first".to_string(), vec![]))
            .await
            .unwrap();
        let id2 = store
            .create(Task::new(TaskKind::Manual, "second".to_string(), vec![]))
            .await
            .unwrap();

        let mut second = store.get(&id2).await.unwrap().unwrap();
        second.status = TaskStatus::Completed;
        store.update(second).await.unwrap();

        let all = store.list(None).await.unwrap();
        assert_eq!(
            all.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![id1, id2]
        );

        let completed = store.list(Some(TaskStatus::Completed)).await.unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, id2);
    }
}
