//! SeaORM-backed database session store.

mod entity;
mod mapping;
mod migration;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::manager::{SessionInfo, SessionManager};
use crate::store::{Result, SessionEntryStore, StoreError};

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
            "unsupported session store database url scheme: <missing>".to_string(),
        )),
        other => Err(StoreError::Database(format!(
            "unsupported session store database url scheme: {other}"
        ))),
    }
}

fn normalize_sqlite_url(url: &str) -> Result<String> {
    if url == "sqlite::memory:" || url == "sqlite://:memory:" {
        return Ok(url.to_string());
    }

    if !url.starts_with("sqlite:") {
        return Err(StoreError::Database(
            "sqlite session store url must start with sqlite:".to_string(),
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

impl DatabaseBackend {
    fn label(self) -> &'static str {
        match self {
            DatabaseBackend::Sqlite => "sqlite",
            DatabaseBackend::Postgres => "postgres",
            DatabaseBackend::MySql => "mysql",
        }
    }
}

#[derive(Clone)]
pub struct DatabaseSessionManager {
    pub(crate) db: DatabaseConnection,
    backend: DatabaseBackend,
}

pub struct DatabaseSessionEntryStore {
    db: DatabaseConnection,
    agent_id: String,
}

impl DatabaseSessionManager {
    pub async fn connect(url: &str) -> Result<Self> {
        match infer_backend(url)? {
            DatabaseBackend::Sqlite => {
                Self::connect_backend(DatabaseBackend::Sqlite, normalize_sqlite_url(url)?).await
            }
            DatabaseBackend::Postgres => {
                Self::connect_backend(DatabaseBackend::Postgres, url.to_string()).await
            }
            DatabaseBackend::MySql => Err(StoreError::Database(
                "database session store backend is recognized but not enabled yet: mysql"
                    .to_string(),
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
                "failed to connect {} session store: {e}",
                backend.label()
            ))
        })?;

        migration::Migrator::up(&db, None).await.map_err(|e| {
            StoreError::Database(format!(
                "failed to migrate {} session store: {e}",
                backend.label()
            ))
        })?;

        Ok(Self { db, backend })
    }

    fn scoped_store(&self, agent_id: &str) -> DatabaseSessionEntryStore {
        DatabaseSessionEntryStore {
            db: self.db.clone(),
            agent_id: agent_id.to_string(),
        }
    }
}

impl DatabaseSessionEntryStore {
    async fn ensure_session_for_entry(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        entry: &crate::entry::SessionEntry,
    ) -> Result<()> {
        use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};

        let existing = entity::sessions::Entity::find_by_id(entry.session_id.clone())
            .one(txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to load session metadata: {e}")))?;

        match existing {
            Some(model) => {
                if model.agent_id != self.agent_id {
                    return Err(StoreError::SessionAgentScopeConflict {
                        session_id: entry.session_id.clone(),
                        expected: model.agent_id,
                        actual: self.agent_id.clone(),
                    });
                }
                Ok(())
            }
            None => {
                entity::sessions::ActiveModel {
                    id: ActiveValue::Set(entry.session_id.clone()),
                    agent_id: ActiveValue::Set(self.agent_id.clone()),
                    created_at: ActiveValue::Set(entry.created_at),
                    updated_at: ActiveValue::Set(entry.created_at),
                    entry_count: ActiveValue::Set(0),
                    metadata: ActiveValue::Set("{}".to_string()),
                }
                .insert(txn)
                .await
                .map_err(|e| StoreError::Database(format!("failed to create session metadata: {e}")))?;
                Ok(())
            }
        }
    }
}

#[async_trait]
impl SessionEntryStore for DatabaseSessionEntryStore {
    async fn save(&self, entry: crate::entry::SessionEntry) -> Result<()> {
        use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, TransactionTrait};

        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| StoreError::Database(format!("failed to begin session entry transaction: {e}")))?;

        self.ensure_session_for_entry(&txn, &entry).await?;

        mapping::entry_to_active_model(entry.clone())?
            .insert(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to insert session entry: {e}")))?;

        let session = entity::sessions::Entity::find_by_id(entry.session_id.clone())
            .one(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to reload session metadata: {e}")))?
            .ok_or_else(|| StoreError::NotFound(format!("session {}", entry.session_id)))?;

        let next_count = session.entry_count + 1;
        let mut active: entity::sessions::ActiveModel = session.into();
        active.updated_at = ActiveValue::Set(entry.created_at);
        active.entry_count = ActiveValue::Set(next_count);
        active
            .update(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to update session metadata: {e}")))?;

        txn.commit()
            .await
            .map_err(|e| StoreError::Database(format!("failed to commit session entry transaction: {e}")))?;
        Ok(())
    }

    async fn get_entries(&self, session_id: &str) -> Result<Vec<crate::entry::SessionEntry>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let models = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .order_by_asc(entity::session_entries::Column::CreatedAt)
            .order_by_asc(entity::session_entries::Column::Id)
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get session entries: {e}")))?;

        models.into_iter().map(mapping::model_to_entry).collect()
    }

    async fn get_after(&self, session_id: &str, after: i64) -> Result<Vec<crate::entry::SessionEntry>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let models = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .filter(entity::session_entries::Column::CreatedAt.gt(after))
            .order_by_asc(entity::session_entries::Column::CreatedAt)
            .order_by_asc(entity::session_entries::Column::Id)
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get session entries after {after}: {e}")))?;

        models.into_iter().map(mapping::model_to_entry).collect()
    }

    async fn find_latest_checkpoint(&self, session_id: &str) -> Result<Option<crate::entry::SessionEntry>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let model = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .filter(entity::session_entries::Column::EntryType.eq("checkpoint"))
            .order_by_desc(entity::session_entries::Column::CreatedAt)
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to find latest checkpoint: {e}")))?;

        model.map(mapping::model_to_entry).transpose()
    }

    async fn delete_session(&self, session_id: &str) -> Result<()> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};

        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| StoreError::Database(format!("failed to begin delete session transaction: {e}")))?;

        entity::session_entries::Entity::delete_many()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .exec(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to delete session entries: {e}")))?;

        entity::sessions::Entity::delete_by_id(session_id.to_string())
            .exec(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to delete session metadata: {e}")))?;

        txn.commit()
            .await
            .map_err(|e| StoreError::Database(format!("failed to commit delete session transaction: {e}")))?;
        Ok(())
    }

    async fn get_count(&self, session_id: &str) -> Result<usize> {
        use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

        let count = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .count(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to count session entries: {e}")))?;
        Ok(count as usize)
    }
}

#[async_trait]
impl SessionManager for DatabaseSessionManager {
    fn entry_store_for_agent(&self, agent_id: &str) -> Arc<dyn SessionEntryStore> {
        Arc::new(self.scoped_store(agent_id))
    }

    async fn list_sessions(&self, agent_id: Option<&str>) -> Result<Vec<SessionInfo>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let mut query = entity::sessions::Entity::find().order_by_desc(entity::sessions::Column::UpdatedAt);
        if let Some(agent_id) = agent_id {
            query = query.filter(entity::sessions::Column::AgentId.eq(agent_id.to_string()));
        }

        let models = query
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to list sessions: {e}")))?;
        Ok(models.into_iter().map(mapping::session_model_to_info).collect())
    }

    async fn session_exists(&self, agent_id: Option<&str>, session_id: &str) -> Result<bool> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let mut query = entity::sessions::Entity::find()
            .filter(entity::sessions::Column::Id.eq(session_id.to_string()));
        if let Some(agent_id) = agent_id {
            query = query.filter(entity::sessions::Column::AgentId.eq(agent_id.to_string()));
        }

        let model = query
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to check session existence: {e}")))?;
        Ok(model.is_some())
    }

    async fn resolve_session_agent(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> Result<String> {
        use sea_orm::EntityTrait;

        let model = entity::sessions::Entity::find_by_id(session_id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to resolve session agent: {e}")))?
            .ok_or_else(|| StoreError::NotFound(format!("session {session_id}")))?;

        if let Some(expected_agent_id) = agent_id {
            if model.agent_id != expected_agent_id {
                return Err(StoreError::SessionAgentScopeConflict {
                    session_id: session_id.to_string(),
                    expected: expected_agent_id.to_string(),
                    actual: model.agent_id,
                });
            }
        }

        Ok(model.agent_id)
    }

    async fn entry_store_for_session(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> Result<Arc<dyn SessionEntryStore>> {
        let resolved_agent_id = self.resolve_session_agent(agent_id, session_id).await?;
        Ok(Arc::new(self.scoped_store(&resolved_agent_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};

    fn test_entry(session_id: &str, id: &str, created_at: i64) -> SessionEntry {
        SessionEntry {
            id: id.to_string(),
            session_id: session_id.to_string(),
            created_at,
            parent_id: None,
            r#type: SessionEntryType::Summary,
            data: SessionEntryData::Summary { summary: format!("summary-{id}") },
        }
    }

    async fn sqlite_manager() -> (tempfile::TempDir, DatabaseSessionManager) {
        let temp = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", temp.path().join("sessions.db").display());
        let manager = DatabaseSessionManager::connect(&url).await.unwrap();
        (temp, manager)
    }

    #[tokio::test]
    async fn sqlite_save_get_and_count_round_trip() {
        let (_temp, manager) = sqlite_manager().await;
        let store = manager.entry_store_for_agent("alpha");
        store.save(test_entry("session-a", "entry-1", 10)).await.unwrap();
        store.save(test_entry("session-a", "entry-2", 20)).await.unwrap();

        let entries = store.get_entries("session-a").await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "entry-1");
        assert_eq!(entries[1].id, "entry-2");
        assert_eq!(store.get_count("session-a").await.unwrap(), 2);
    }

    #[tokio::test]
    async fn sqlite_list_sessions_reads_sessions_table() {
        let (_temp, manager) = sqlite_manager().await;
        let store = manager.entry_store_for_agent("alpha");
        store.save(test_entry("session-a", "entry-1", 10)).await.unwrap();
        store.save(test_entry("session-a", "entry-2", 20)).await.unwrap();

        let sessions = manager.list_sessions(Some("alpha")).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session-a");
        assert_eq!(sessions[0].agent_id, "alpha");
        assert_eq!(sessions[0].entry_count, 2);
        assert_eq!(sessions[0].created_at, 10);
        assert_eq!(sessions[0].updated_at, Some(20));
    }

    #[tokio::test]
    async fn sqlite_delete_session_removes_metadata_and_entries() {
        let (_temp, manager) = sqlite_manager().await;
        let store = manager.entry_store_for_agent("alpha");
        store.save(test_entry("session-a", "entry-1", 10)).await.unwrap();

        store.delete_session("session-a").await.unwrap();

        assert_eq!(store.get_entries("session-a").await.unwrap().len(), 0);
        assert!(!manager.session_exists(Some("alpha"), "session-a").await.unwrap());
    }

    #[tokio::test]
    async fn sqlite_rejects_conflicting_agent_scope() {
        let (_temp, manager) = sqlite_manager().await;
        manager
            .entry_store_for_agent("alpha")
            .save(test_entry("session-a", "entry-1", 10))
            .await
            .unwrap();

        let err = manager
            .entry_store_for_agent("beta")
            .save(test_entry("session-a", "entry-2", 20))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Session agent scope conflict"));
    }
}
