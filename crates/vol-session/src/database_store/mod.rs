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

        Ok(Self { db })
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
        use sea_orm::{sea_query::OnConflict, ActiveValue, EntityTrait};

        entity::sessions::Entity::insert(entity::sessions::ActiveModel {
            id: ActiveValue::Set(entry.session_id.clone()),
            agent_id: ActiveValue::Set(self.agent_id.clone()),
            created_at: ActiveValue::Set(entry.created_at),
            updated_at: ActiveValue::Set(entry.created_at),
            entry_count: ActiveValue::Set(0),
            metadata: ActiveValue::Set("{}".to_string()),
        })
        .on_conflict(
            OnConflict::column(entity::sessions::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(txn)
        .await
        .map_err(|e| StoreError::Database(format!("failed to ensure session metadata: {e}")))?;

        self.load_owned_session(txn, &entry.session_id)
            .await
            .map(|_| ())
    }

    async fn load_owned_session<C>(
        &self,
        db: &C,
        session_id: &str,
    ) -> Result<entity::sessions::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let session = entity::sessions::Entity::find()
            .filter(entity::sessions::Column::Id.eq(session_id.to_string()))
            .one(db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to load session metadata: {e}")))?
            .ok_or_else(|| StoreError::NotFound(format!("session {session_id}")))?;

        if session.agent_id != self.agent_id {
            return Err(StoreError::SessionAgentScopeConflict {
                session_id: session_id.to_string(),
                expected: session.agent_id,
                actual: self.agent_id.clone(),
            });
        }

        Ok(session)
    }
}

#[async_trait]
impl SessionEntryStore for DatabaseSessionEntryStore {
    async fn save(&self, entry: crate::entry::SessionEntry) -> Result<()> {
        use sea_orm::{
            sea_query::Expr, ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter,
            TransactionTrait,
        };

        let txn = self.db.begin().await.map_err(|e| {
            StoreError::Database(format!("failed to begin session entry transaction: {e}"))
        })?;

        self.ensure_session_for_entry(&txn, &entry).await?;

        mapping::entry_to_active_model(entry.clone())?
            .insert(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to insert session entry: {e}")))?;

        entity::sessions::Entity::update_many()
            .col_expr(
                entity::sessions::Column::EntryCount,
                Expr::col(entity::sessions::Column::EntryCount).add(1),
            )
            .col_expr(
                entity::sessions::Column::UpdatedAt,
                Expr::value(entry.created_at),
            )
            .filter(entity::sessions::Column::Id.eq(entry.session_id.clone()))
            .filter(entity::sessions::Column::AgentId.eq(self.agent_id.clone()))
            .exec(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to update session metadata: {e}")))?;

        txn.commit().await.map_err(|e| {
            StoreError::Database(format!("failed to commit session entry transaction: {e}"))
        })?;
        Ok(())
    }

    async fn get_entries(&self, session_id: &str) -> Result<Vec<crate::entry::SessionEntry>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        self.load_owned_session(&self.db, session_id).await?;

        let models = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .order_by_asc(entity::session_entries::Column::CreatedAt)
            .order_by_asc(entity::session_entries::Column::Id)
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get session entries: {e}")))?;

        models.into_iter().map(mapping::model_to_entry).collect()
    }

    async fn get_after(
        &self,
        session_id: &str,
        after: i64,
    ) -> Result<Vec<crate::entry::SessionEntry>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        self.load_owned_session(&self.db, session_id).await?;

        let models = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .filter(entity::session_entries::Column::CreatedAt.gte(after))
            .order_by_asc(entity::session_entries::Column::CreatedAt)
            .order_by_asc(entity::session_entries::Column::Id)
            .all(&self.db)
            .await
            .map_err(|e| {
                StoreError::Database(format!("failed to get session entries after {after}: {e}"))
            })?;

        models.into_iter().map(mapping::model_to_entry).collect()
    }

    async fn find_latest_checkpoint(
        &self,
        session_id: &str,
    ) -> Result<Option<crate::entry::SessionEntry>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        self.load_owned_session(&self.db, session_id).await?;

        let model = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .filter(entity::session_entries::Column::EntryType.eq("checkpoint"))
            .order_by_desc(entity::session_entries::Column::CreatedAt)
            .order_by_desc(entity::session_entries::Column::Id)
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to find latest checkpoint: {e}")))?;

        model.map(mapping::model_to_entry).transpose()
    }

    async fn delete_session(&self, session_id: &str) -> Result<()> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};

        let txn = self.db.begin().await.map_err(|e| {
            StoreError::Database(format!("failed to begin delete session transaction: {e}"))
        })?;

        self.load_owned_session(&txn, session_id).await?;

        entity::session_entries::Entity::delete_many()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .exec(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to delete session entries: {e}")))?;

        entity::sessions::Entity::delete_by_id(session_id.to_string())
            .exec(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to delete session metadata: {e}")))?;

        txn.commit().await.map_err(|e| {
            StoreError::Database(format!("failed to commit delete session transaction: {e}"))
        })?;
        Ok(())
    }

    async fn get_count(&self, session_id: &str) -> Result<usize> {
        use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

        self.load_owned_session(&self.db, session_id).await?;

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

        let mut query =
            entity::sessions::Entity::find().order_by_desc(entity::sessions::Column::UpdatedAt);
        if let Some(agent_id) = agent_id {
            query = query.filter(entity::sessions::Column::AgentId.eq(agent_id.to_string()));
        }

        let models = query
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to list sessions: {e}")))?;
        Ok(models
            .into_iter()
            .map(mapping::session_model_to_info)
            .collect())
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
    use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};

    fn test_entry(session_id: &str, id: &str, created_at: i64) -> SessionEntry {
        SessionEntry {
            id: id.to_string(),
            session_id: session_id.to_string(),
            created_at,
            parent_id: None,
            r#type: SessionEntryType::Summary,
            data: SessionEntryData::Summary {
                summary: format!("summary-{id}"),
            },
        }
    }

    fn checkpoint_entry(session_id: &str, id: &str, created_at: i64) -> SessionEntry {
        SessionEntry {
            id: id.to_string(),
            session_id: session_id.to_string(),
            created_at,
            parent_id: None,
            r#type: SessionEntryType::Checkpoint,
            data: SessionEntryData::Checkpoint {
                reason: CheckpointReason::Manual,
                note: Some(format!("checkpoint-{id}")),
            },
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
        store
            .save(test_entry("session-a", "entry-1", 10))
            .await
            .unwrap();
        store
            .save(test_entry("session-a", "entry-2", 20))
            .await
            .unwrap();

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
        store
            .save(test_entry("session-a", "entry-1", 10))
            .await
            .unwrap();
        store
            .save(test_entry("session-a", "entry-2", 20))
            .await
            .unwrap();

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
        store
            .save(test_entry("session-a", "entry-1", 10))
            .await
            .unwrap();

        store.delete_session("session-a").await.unwrap();

        let err = store.get_entries("session-a").await.unwrap_err();
        assert!(err.to_string().contains("Not found: session session-a"));
        assert!(!manager
            .session_exists(Some("alpha"), "session-a")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn sqlite_scoped_store_denies_cross_agent_reads_and_delete() {
        let (_temp, manager) = sqlite_manager().await;
        let alpha_store = manager.entry_store_for_agent("alpha");
        let beta_store = manager.entry_store_for_agent("beta");
        alpha_store
            .save(test_entry("session-a", "entry-1", 10))
            .await
            .unwrap();

        let read_err = beta_store.get_entries("session-a").await.unwrap_err();
        assert!(read_err
            .to_string()
            .contains("Session agent scope conflict"));
        let after_err = beta_store.get_after("session-a", 10).await.unwrap_err();
        assert!(after_err
            .to_string()
            .contains("Session agent scope conflict"));
        let checkpoint_err = beta_store
            .find_latest_checkpoint("session-a")
            .await
            .unwrap_err();
        assert!(checkpoint_err
            .to_string()
            .contains("Session agent scope conflict"));
        let count_err = beta_store.get_count("session-a").await.unwrap_err();
        assert!(count_err
            .to_string()
            .contains("Session agent scope conflict"));
        let delete_err = beta_store.delete_session("session-a").await.unwrap_err();
        assert!(delete_err
            .to_string()
            .contains("Session agent scope conflict"));

        assert!(manager
            .session_exists(Some("alpha"), "session-a")
            .await
            .unwrap());
        assert_eq!(alpha_store.get_entries("session-a").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn sqlite_concurrent_saves_preserve_entry_count() {
        let (_temp, manager) = sqlite_manager().await;
        let store = manager.entry_store_for_agent("alpha");
        let saves = (0..20).map(|idx| {
            let store = Arc::clone(&store);
            tokio::spawn(async move {
                store
                    .save(test_entry("session-a", &format!("entry-{idx:02}"), idx))
                    .await
                    .unwrap();
            })
        });

        for save in saves {
            save.await.unwrap();
        }

        assert_eq!(store.get_count("session-a").await.unwrap(), 20);
        let sessions = manager.list_sessions(Some("alpha")).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].entry_count, 20);
    }

    #[tokio::test]
    async fn sqlite_get_after_includes_equal_timestamp() {
        let (_temp, manager) = sqlite_manager().await;
        let store = manager.entry_store_for_agent("alpha");
        store
            .save(test_entry("session-a", "entry-1", 10))
            .await
            .unwrap();
        store
            .save(test_entry("session-a", "entry-2", 20))
            .await
            .unwrap();

        let entries = store.get_after("session-a", 10).await.unwrap();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["entry-1", "entry-2"]
        );
    }

    #[tokio::test]
    async fn sqlite_latest_checkpoint_tie_breaks_by_id_desc() {
        let (_temp, manager) = sqlite_manager().await;
        let store = manager.entry_store_for_agent("alpha");
        store
            .save(checkpoint_entry("session-a", "checkpoint-a", 10))
            .await
            .unwrap();
        store
            .save(checkpoint_entry("session-a", "checkpoint-z", 10))
            .await
            .unwrap();

        let checkpoint = store
            .find_latest_checkpoint("session-a")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.id, "checkpoint-z");
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

    const POSTGRES_TEST_URL_ENV: &str = "VOL_AGENT_POSTGRES_TEST_URL";
    const POSTGRES_TEST_AGENT_PREFIX: &str = "vol-agent-postgres-session-test-agent-";
    const POSTGRES_TEST_SESSION_PREFIX: &str = "vol-agent-postgres-session-test-session-";

    struct PostgresTestLock(std::fs::File);

    impl PostgresTestLock {
        fn acquire() -> Self {
            use fd_lock::RwLock;
            let path = std::env::temp_dir().join("vol-agent-postgres-session-store-test.lock");
            let file = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(path)
                .expect("postgres session test lock file should open");
            file.lock()
                .expect("postgres session test lock should be acquired");
            Self(file)
        }
    }

    impl Drop for PostgresTestLock {
        fn drop(&mut self) {
            use fd_lock::RwLock;
            self.0
                .unlock()
                .expect("postgres session test lock should release");
        }
    }

    async fn clean_postgres(manager: &DatabaseSessionManager) {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        entity::session_entries::Entity::delete_many()
            .filter(
                entity::session_entries::Column::SessionId
                    .starts_with(POSTGRES_TEST_SESSION_PREFIX),
            )
            .exec(&manager.db)
            .await
            .unwrap();
        entity::sessions::Entity::delete_many()
            .filter(entity::sessions::Column::Id.starts_with(POSTGRES_TEST_SESSION_PREFIX))
            .exec(&manager.db)
            .await
            .unwrap();
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

    #[tokio::test]
    async fn postgres_save_list_and_reconnect_when_configured() {
        // CI should set VOL_AGENT_POSTGRES_TEST_URL so Postgres session-store coverage is exercised.
        let Ok(url) = std::env::var(POSTGRES_TEST_URL_ENV) else {
            eprintln!(
                "SKIPPED: VOL_AGENT_POSTGRES_TEST_URL is not set; Postgres session-store coverage was not exercised"
            );
            return;
        };

        let _lock = PostgresTestLock::acquire();
        let manager = DatabaseSessionManager::connect(&url).await.unwrap();
        clean_postgres(&manager).await;

        let test_agent_id = format!("{POSTGRES_TEST_AGENT_PREFIX}{}", uuid::Uuid::new_v4());
        let test_session_id = format!("{POSTGRES_TEST_SESSION_PREFIX}{}", uuid::Uuid::new_v4());

        manager
            .entry_store_for_agent(&test_agent_id)
            .save(test_entry(&test_session_id, "pg-entry-1", 100))
            .await
            .unwrap();

        let reconnected = DatabaseSessionManager::connect(&url).await.unwrap();
        let sessions = reconnected
            .list_sessions(Some(&test_agent_id))
            .await
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, test_session_id);
        assert_eq!(sessions[0].entry_count, 1);

        clean_postgres(&reconnected).await;
    }

    #[tokio::test]
    async fn postgres_concurrent_first_saves_when_configured() {
        // CI should set VOL_AGENT_POSTGRES_TEST_URL so Postgres session-store coverage is exercised.
        let Ok(url) = std::env::var(POSTGRES_TEST_URL_ENV) else {
            eprintln!(
                "SKIPPED: VOL_AGENT_POSTGRES_TEST_URL is not set; Postgres session-store coverage was not exercised"
            );
            return;
        };

        let _lock = PostgresTestLock::acquire();
        let manager = DatabaseSessionManager::connect(&url).await.unwrap();
        clean_postgres(&manager).await;

        let test_agent_id = format!("{POSTGRES_TEST_AGENT_PREFIX}{}", uuid::Uuid::new_v4());
        let test_session_id = format!("{POSTGRES_TEST_SESSION_PREFIX}{}", uuid::Uuid::new_v4());
        let save_count = 8;

        let saves = (0..save_count).map(|idx| {
            let store = manager.entry_store_for_agent(&test_agent_id);
            let session_id = test_session_id.clone();
            tokio::spawn(async move {
                store
                    .save(test_entry(
                        &session_id,
                        &format!("pg-concurrent-entry-{idx:02}"),
                        idx,
                    ))
                    .await
            })
        });

        for save in saves {
            save.await.unwrap().unwrap();
        }

        let store = manager.entry_store_for_agent(&test_agent_id);
        let entries = store.get_entries(&test_session_id).await.unwrap();
        assert_eq!(entries.len(), save_count as usize);
        assert_eq!(
            store.get_count(&test_session_id).await.unwrap(),
            save_count as usize
        );

        let sessions = manager.list_sessions(Some(&test_agent_id)).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, test_session_id);
        assert_eq!(sessions[0].entry_count, save_count as usize);

        clean_postgres(&manager).await;
    }
}
