//! SeaORM-backed database task store.

mod entity;
mod mapping;
mod migration;

use std::path::PathBuf;

use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::store::{Result, StoreError};

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
    db: DatabaseConnection,
    #[allow(dead_code)]
    backend: DatabaseBackend,
}

impl DatabaseTaskStore {
    pub async fn connect(url: &str) -> Result<Self> {
        match infer_backend(url)? {
            DatabaseBackend::Sqlite => {
                Self::connect_backend(DatabaseBackend::Sqlite, normalize_sqlite_url(url)?).await
            }
            DatabaseBackend::Postgres => {
                Self::connect_backend(DatabaseBackend::Postgres, url.to_string()).await
            }
            DatabaseBackend::MySql => Err(StoreError::Database(
                "database task store backend is recognized but not enabled yet: mysql".to_string(),
            )),
        }
    }

    async fn connect_backend(backend: DatabaseBackend, url: String) -> Result<Self> {
        if backend == DatabaseBackend::Sqlite {
            create_sqlite_parent_dir(&url)?;
        }

        let mut options = ConnectOptions::new(url);
        options.max_connections(5);
        let db = Database::connect(options).await.map_err(|e| {
            StoreError::Database(format!(
                "failed to connect {} task store: {e}",
                backend.label()
            ))
        })?;

        migration::Migrator::up(&db, None).await.map_err(|e| {
            StoreError::Database(format!(
                "failed to migrate {} task store: {e}",
                backend.label()
            ))
        })?;

        Ok(Self { db, backend })
    }
}

impl DatabaseBackend {
    fn label(self) -> &'static str {
        match self {
            DatabaseBackend::Sqlite => "sqlite",
            DatabaseBackend::Postgres => "postgres",
            DatabaseBackend::MySql => "mysql",
        }
    }
}

fn normalize_sqlite_url(url: &str) -> Result<String> {
    if url == "sqlite::memory:" || url == "sqlite://:memory:" {
        return Ok(url.to_string());
    }

    if !url.starts_with("sqlite:") {
        return Err(StoreError::Database(
            "sqlite task store url must start with sqlite:".to_string(),
        ));
    }

    let url = expand_sqlite_tilde_url(url);

    if let Some((_, query)) = url.split_once('?') {
        if query
            .split('&')
            .filter_map(|param| param.split_once('=').map(|(key, _)| key))
            .any(|key| key == "mode")
        {
            Ok(url)
        } else {
            Ok(format!("{url}&mode=rwc"))
        }
    } else {
        Ok(format!("{url}?mode=rwc"))
    }
}

fn expand_sqlite_tilde_url(url: &str) -> String {
    let (without_query, query) = url
        .split_once('?')
        .map_or((url, None), |(path, query)| (path, Some(query)));
    let Some(raw) = without_query
        .strip_prefix("sqlite://")
        .or_else(|| without_query.strip_prefix("sqlite:"))
    else {
        return url.to_string();
    };

    let expanded = if raw == "~" {
        std::env::var("HOME").unwrap_or_else(|_| raw.to_string())
    } else if let Some(rest) = raw.strip_prefix("~/") {
        std::env::var("HOME")
            .map(|home| format!("{home}/{rest}"))
            .unwrap_or_else(|_| raw.to_string())
    } else {
        raw.to_string()
    };

    let rebuilt = format!("sqlite://{expanded}");
    match query {
        Some(query) => format!("{rebuilt}?{query}"),
        None => rebuilt,
    }
}

fn create_sqlite_parent_dir(url: &str) -> Result<()> {
    if let Some(path) = sqlite_file_path(url) {
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
    let without_query = url.split_once('?').map(|(path, _)| path).unwrap_or(url);
    let raw = without_query
        .strip_prefix("sqlite://")
        .or_else(|| without_query.strip_prefix("sqlite:"))?;
    if raw.is_empty() || raw == ":memory:" {
        return None;
    }
    Some(PathBuf::from(raw))
}

#[async_trait::async_trait]
impl crate::store::TaskStore for DatabaseTaskStore {
    async fn create(&self, task: crate::model::Task) -> Result<crate::model::TaskId> {
        use sea_orm::ActiveModelTrait;

        let mut active = mapping::task_to_active_model(task)?;
        active.id = sea_orm::ActiveValue::NotSet;
        let inserted = active
            .insert(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to create task: {e}")))?;
        mapping::task_id_from_db(inserted.id)
    }

    async fn get(&self, task_id: &crate::model::TaskId) -> Result<Option<crate::model::Task>> {
        use sea_orm::EntityTrait;

        let id = mapping::task_id_to_db(*task_id)?;
        let model = entity::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get task {}: {e}", task_id)))?;
        model.map(mapping::model_to_task).transpose()
    }

    async fn update(&self, task: crate::model::Task) -> Result<()> {
        use sea_orm::{ActiveModelTrait, EntityTrait};

        let id = mapping::task_id_to_db(task.id)?;
        let exists = entity::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| {
                StoreError::Database(format!("failed to get task {} for update: {e}", task.id))
            })?;
        if exists.is_none() {
            return Err(StoreError::NotFound(format!("Task {}", task.id)));
        }

        mapping::task_to_active_model(task.clone())?
            .update(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to update task {}: {e}", task.id)))?;
        Ok(())
    }

    async fn delete(&self, task_id: &crate::model::TaskId) -> Result<()> {
        use sea_orm::EntityTrait;

        let id = mapping::task_id_to_db(*task_id)?;
        entity::Entity::delete_by_id(id)
            .exec(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to delete task {}: {e}", task_id)))?;
        Ok(())
    }

    async fn list(
        &self,
        status: Option<crate::model::TaskStatus>,
    ) -> Result<Vec<crate::model::Task>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let mut query = entity::Entity::find().order_by_asc(entity::Column::Id);
        if let Some(status) = status {
            query = query.filter(entity::Column::Status.eq(mapping::status_to_db(status)));
        }
        let models = query
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to list tasks: {e}")))?;
        models.into_iter().map(mapping::model_to_task).collect()
    }

    async fn get_ready_tasks(&self) -> Result<Vec<crate::model::TaskId>> {
        let tasks = self.list(None).await?;
        let completed_ids: std::collections::HashSet<crate::model::TaskId> = tasks
            .iter()
            .filter(|task| task.status == crate::model::TaskStatus::Completed)
            .map(|task| task.id)
            .collect();

        let ready = tasks
            .iter()
            .filter(|task| {
                task.status == crate::model::TaskStatus::Pending
                    && task
                        .dependencies
                        .iter()
                        .all(|id| completed_ids.contains(id))
            })
            .map(|task| task.id)
            .collect();

        Ok(ready)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSTGRES_TEST_URL_ENV: &str = "VOL_AGENT_POSTGRES_TEST_URL";

    fn postgres_test_url() -> Option<String> {
        match std::env::var(POSTGRES_TEST_URL_ENV) {
            Ok(url) => Some(url),
            Err(_) => {
                eprintln!(
                    "SKIPPED: VOL_AGENT_POSTGRES_TEST_URL is not set; Postgres task-store coverage was not exercised"
                );
                None
            }
        }
    }

    struct PostgresTestLock(std::fs::File);

    impl PostgresTestLock {
        fn acquire() -> Self {
            let path = std::env::temp_dir().join("vol-agent-postgres-task-store-test.lock");
            let file = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(path)
                .expect("postgres test lock file should open");
            file.lock().expect("postgres test lock should be acquired");
            Self(file)
        }
    }

    impl Drop for PostgresTestLock {
        fn drop(&mut self) {
            self.0.unlock().expect("postgres test lock should release");
        }
    }

    async fn clear_store(store: &DatabaseTaskStore) {
        use sea_orm::{ConnectionTrait, Statement};

        let backend = match store.backend {
            DatabaseBackend::Sqlite => sea_orm::DatabaseBackend::Sqlite,
            DatabaseBackend::Postgres => sea_orm::DatabaseBackend::Postgres,
            DatabaseBackend::MySql => unreachable!("mysql is not enabled"),
        };

        store
            .db
            .execute(Statement::from_string(
                backend,
                "DELETE FROM tasks".to_string(),
            ))
            .await
            .unwrap();
    }

    async fn sqlite_store() -> (DatabaseTaskStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tasks.db");
        let url = format!("sqlite://{}", db_path.display());
        let store = DatabaseTaskStore::connect(&url).await.unwrap();
        clear_store(&store).await;
        (store, dir)
    }

    async fn postgres_store() -> Option<DatabaseTaskStore> {
        let url = postgres_test_url()?;
        let store = DatabaseTaskStore::connect(&url).await.unwrap();
        clear_store(&store).await;
        Some(store)
    }

    async fn assert_create_get(store: &DatabaseTaskStore) {
        use crate::model::{Task, TaskKind, TaskStatus};
        use crate::store::TaskStore;

        let mut task = Task::new(TaskKind::Agent, "database task".to_string(), vec![]);
        task.description = "stored with seaorm".to_string();
        task.publisher = Some("planner".to_string());
        task.assignee = Some("worker".to_string());
        task.active_form = Some("Working".to_string());

        let id = store.create(task).await.unwrap();
        assert!(id.0 > 0);

        let got = store.get(&id).await.unwrap().unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.subject, "database task");
        assert_eq!(got.description, "stored with seaorm");
        assert_eq!(got.publisher.as_deref(), Some("planner"));
        assert_eq!(got.assignee.as_deref(), Some("worker"));
        assert_eq!(got.active_form.as_deref(), Some("Working"));
        assert_eq!(got.status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn sqlite_create_assigns_id_and_get_retrieves_task() {
        let (store, _dir) = sqlite_store().await;
        assert_create_get(&store).await;
    }

    #[tokio::test]
    async fn postgres_create_assigns_id_and_get_retrieves_task() {
        let _guard = PostgresTestLock::acquire();
        let Some(store) = postgres_store().await else {
            return;
        };
        assert_create_get(&store).await;
    }

    async fn assert_update_delete_list(store: &DatabaseTaskStore) {
        use crate::model::{Task, TaskKind, TaskResult, TaskStatus};
        use crate::store::TaskStore;
        use std::path::PathBuf;
        use std::time::SystemTime;

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
        second.subject = "updated".to_string();
        second.description = "done".to_string();
        second.summary = Some("summary".to_string());
        second.output_file = Some(PathBuf::from("/tmp/output.txt"));
        second.result = Some(TaskResult {
            success: true,
            output_truncated: "ok".to_string(),
            output_file: PathBuf::from("/tmp/result.txt"),
        });
        second.completed_at = Some(SystemTime::now());
        store.update(second).await.unwrap();

        let updated = store.get(&id2).await.unwrap().unwrap();
        assert_eq!(updated.status, TaskStatus::Completed);
        assert_eq!(updated.subject, "updated");
        assert_eq!(updated.summary.as_deref(), Some("summary"));
        assert!(updated.result.as_ref().unwrap().success);

        let all = store.list(None).await.unwrap();
        assert_eq!(
            all.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![id1, id2]
        );
        let completed = store.list(Some(TaskStatus::Completed)).await.unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, id2);

        store.delete(&id1).await.unwrap();
        assert!(store.get(&id1).await.unwrap().is_none());
        store.delete(&id1).await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_update_delete_list() {
        let (store, _dir) = sqlite_store().await;
        assert_update_delete_list(&store).await;
    }

    #[tokio::test]
    async fn postgres_update_delete_list() {
        let _guard = PostgresTestLock::acquire();
        let Some(store) = postgres_store().await else {
            return;
        };
        assert_update_delete_list(&store).await;
    }

    async fn assert_ready_tasks(store: &DatabaseTaskStore) {
        use crate::model::{Task, TaskKind, TaskStatus};
        use crate::store::TaskStore;

        let dependency_id = store
            .create(Task::new(TaskKind::Agent, "dependency".to_string(), vec![]))
            .await
            .unwrap();
        let blocked_id = store
            .create(Task::new(
                TaskKind::Agent,
                "blocked".to_string(),
                vec![dependency_id],
            ))
            .await
            .unwrap();

        let ready = store.get_ready_tasks().await.unwrap();
        assert_eq!(ready, vec![dependency_id]);
        assert!(!ready.contains(&blocked_id));

        let mut dependency = store.get(&dependency_id).await.unwrap().unwrap();
        dependency.status = TaskStatus::Completed;
        store.update(dependency).await.unwrap();

        let ready = store.get_ready_tasks().await.unwrap();
        assert_eq!(ready, vec![blocked_id]);
    }

    #[tokio::test]
    async fn sqlite_ready_tasks() {
        let (store, _dir) = sqlite_store().await;
        assert_ready_tasks(&store).await;
    }

    #[tokio::test]
    async fn postgres_ready_tasks() {
        let _guard = PostgresTestLock::acquire();
        let Some(store) = postgres_store().await else {
            return;
        };
        assert_ready_tasks(&store).await;
    }

    #[tokio::test]
    async fn sqlite_tasks_persist_across_reconnect() {
        use crate::model::{Task, TaskKind};
        use crate::store::TaskStore;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tasks.db");
        let url = format!("sqlite://{}", db_path.display());
        let store = DatabaseTaskStore::connect(&url).await.unwrap();
        clear_store(&store).await;
        let id = store
            .create(Task::new(TaskKind::Agent, "persisted".to_string(), vec![]))
            .await
            .unwrap();
        drop(store);

        let reopened = DatabaseTaskStore::connect(&url).await.unwrap();
        let got = reopened.get(&id).await.unwrap().unwrap();
        assert_eq!(got.subject, "persisted");
    }

    #[tokio::test]
    async fn postgres_tasks_persist_across_reconnect() {
        let _guard = PostgresTestLock::acquire();
        use crate::model::{Task, TaskKind};
        use crate::store::TaskStore;

        let Some(url) = postgres_test_url() else {
            return;
        };
        let store = DatabaseTaskStore::connect(&url).await.unwrap();
        clear_store(&store).await;
        let id = store
            .create(Task::new(
                TaskKind::Agent,
                "persisted pg".to_string(),
                vec![],
            ))
            .await
            .unwrap();
        drop(store);

        let reopened = DatabaseTaskStore::connect(&url).await.unwrap();
        let got = reopened.get(&id).await.unwrap().unwrap();
        assert_eq!(got.subject, "persisted pg");
        clear_store(&reopened).await;
    }

    #[tokio::test]
    async fn sqlite_connect_runs_migration() {
        use sea_orm::{ConnectionTrait, Statement};

        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("tasks.db").display());
        let store = DatabaseTaskStore::connect(&url).await.unwrap();
        let rows = store
            .db
            .query_all(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'tasks'"
                    .to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn postgres_connect_runs_migration() {
        let _guard = PostgresTestLock::acquire();
        use sea_orm::{ConnectionTrait, Statement};

        let Some(url) = postgres_test_url() else {
            return;
        };
        let store = DatabaseTaskStore::connect(&url).await.unwrap();
        clear_store(&store).await;
        let rows = store
            .db
            .query_all(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'tasks'"
                    .to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn mapping_rejects_task_id_larger_than_i64() {
        let err = mapping::task_id_to_db(crate::model::TaskId(i64::MAX as u64 + 1)).unwrap_err();
        assert!(err
            .to_string()
            .contains("task id exceeds database i64 range"));
    }

    #[test]
    fn mapping_rejects_negative_database_id() {
        let err = mapping::task_id_from_db(-1).unwrap_err();
        assert!(err.to_string().contains("negative task id"));
    }

    #[test]
    fn mapping_roundtrips_task_model_fields() {
        use crate::model::{Task, TaskKind, TaskResult, TaskStatus};
        use std::path::PathBuf;
        use std::time::{Duration, UNIX_EPOCH};

        let mut task = Task::new(
            TaskKind::Agent,
            "mapped".to_string(),
            vec![crate::model::TaskId(7)],
        );
        task.id = crate::model::TaskId(42);
        task.status = TaskStatus::Running;
        task.publisher = Some("publisher".to_string());
        task.assignee = Some("assignee".to_string());
        task.description = "description".to_string();
        task.active_form = Some("Mapping".to_string());
        task.blocks = vec![crate::model::TaskId(9)];
        task.result = Some(TaskResult {
            success: true,
            output_truncated: "ok".to_string(),
            output_file: PathBuf::from("/tmp/result.txt"),
        });
        task.summary = Some("summary".to_string());
        task.output_file = Some(PathBuf::from("/tmp/output.txt"));
        task.created_at = UNIX_EPOCH + Duration::from_secs(11);
        task.started_at = Some(UNIX_EPOCH + Duration::from_secs(12));
        task.completed_at = Some(UNIX_EPOCH + Duration::from_secs(13));

        let active = mapping::task_to_active_model(task.clone()).unwrap();
        let model = entity::Model {
            id: 42,
            status: active.status.unwrap(),
            kind: active.kind.unwrap(),
            publisher: active.publisher.unwrap(),
            assignee: active.assignee.unwrap(),
            subject: active.subject.unwrap(),
            description: active.description.unwrap(),
            active_form: active.active_form.unwrap(),
            dependencies_json: active.dependencies_json.unwrap(),
            blocks_json: active.blocks_json.unwrap(),
            result_json: active.result_json.unwrap(),
            summary: active.summary.unwrap(),
            output_file: active.output_file.unwrap(),
            created_at_secs: active.created_at_secs.unwrap(),
            started_at_secs: active.started_at_secs.unwrap(),
            completed_at_secs: active.completed_at_secs.unwrap(),
        };

        let roundtripped = mapping::model_to_task(model).unwrap();
        assert_eq!(roundtripped.id, task.id);
        assert_eq!(roundtripped.status, task.status);
        assert_eq!(roundtripped.subject, task.subject);
        assert_eq!(roundtripped.description, task.description);
        assert_eq!(roundtripped.dependencies, task.dependencies);
        assert_eq!(roundtripped.blocks, task.blocks);
        assert!(roundtripped.result.unwrap().success);
        assert_eq!(roundtripped.summary, task.summary);
        assert_eq!(roundtripped.output_file, task.output_file);
        assert_eq!(roundtripped.created_at, task.created_at);
        assert_eq!(roundtripped.started_at, task.started_at);
        assert_eq!(roundtripped.completed_at, task.completed_at);
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

    #[test]
    fn normalize_sqlite_url_adds_create_mode() {
        assert_eq!(
            normalize_sqlite_url("sqlite:///tmp/tasks.db").unwrap(),
            "sqlite:///tmp/tasks.db?mode=rwc"
        );
        assert_eq!(
            normalize_sqlite_url("sqlite:///tmp/tasks.db?cache=shared").unwrap(),
            "sqlite:///tmp/tasks.db?cache=shared&mode=rwc"
        );
        assert_eq!(
            normalize_sqlite_url("sqlite:///tmp/tasks.db?journal_mode=wal").unwrap(),
            "sqlite:///tmp/tasks.db?journal_mode=wal&mode=rwc"
        );
        assert_eq!(
            normalize_sqlite_url("sqlite:///tmp/tasks.db?mode=rwc").unwrap(),
            "sqlite:///tmp/tasks.db?mode=rwc"
        );
    }

    #[test]
    fn normalize_sqlite_url_expands_home_dir() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(
            normalize_sqlite_url("sqlite://~/.vol/data.db").unwrap(),
            format!("sqlite://{home}/.vol/data.db?mode=rwc")
        );
        assert_eq!(
            normalize_sqlite_url("sqlite://~/.vol/data.db?cache=shared").unwrap(),
            format!("sqlite://{home}/.vol/data.db?cache=shared&mode=rwc")
        );
    }
}
