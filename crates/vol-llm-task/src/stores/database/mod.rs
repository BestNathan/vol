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

    if let Some((_, query)) = url.split_once('?') {
        if query
            .split('&')
            .filter_map(|param| param.split_once('=').map(|(key, _)| key))
            .any(|key| key == "mode")
        {
            Ok(url.to_string())
        } else {
            Ok(format!("{url}&mode=rwc"))
        }
    } else {
        Ok(format!("{url}?mode=rwc"))
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
    async fn create(&self, _task: crate::model::Task) -> Result<crate::model::TaskId> {
        Err(StoreError::Internal(
            "SeaORM database task create is not implemented".to_string(),
        ))
    }

    async fn get(&self, _task_id: &crate::model::TaskId) -> Result<Option<crate::model::Task>> {
        Err(StoreError::Internal(
            "SeaORM database task get is not implemented".to_string(),
        ))
    }

    async fn update(&self, _task: crate::model::Task) -> Result<()> {
        Err(StoreError::Internal(
            "SeaORM database task update is not implemented".to_string(),
        ))
    }

    async fn delete(&self, _task_id: &crate::model::TaskId) -> Result<()> {
        Err(StoreError::Internal(
            "SeaORM database task delete is not implemented".to_string(),
        ))
    }

    async fn list(
        &self,
        _status: Option<crate::model::TaskStatus>,
    ) -> Result<Vec<crate::model::Task>> {
        Err(StoreError::Internal(
            "SeaORM database task list is not implemented".to_string(),
        ))
    }

    async fn get_ready_tasks(&self) -> Result<Vec<crate::model::TaskId>> {
        Err(StoreError::Internal(
            "SeaORM database task ready query is not implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
