//! SQLx-backed database task store.

use crate::store::{Result, StoreError, TaskStore};

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

pub struct DatabaseTaskStore;

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
}
