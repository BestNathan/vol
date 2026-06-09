//! Session manager abstractions for listing and resolving session stores.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use crate::file_store::FileSessionEntryStore;
use crate::store::{Result as StoreResult, SessionEntryStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    pub agent_id: String,
    pub session_id: String,
    pub entry_count: usize,
    pub created_at: i64,
    pub updated_at: Option<i64>,
}

#[async_trait]
pub trait SessionManager: Send + Sync {
    fn entry_store_for_agent(&self, agent_id: &str) -> Arc<dyn SessionEntryStore>;

    async fn list_sessions(&self, agent_id: Option<&str>) -> StoreResult<Vec<SessionInfo>>;

    async fn session_exists(&self, agent_id: Option<&str>, session_id: &str) -> StoreResult<bool>;

    async fn resolve_session_agent(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<String>;

    async fn entry_store_for_session(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<Arc<dyn SessionEntryStore>>;
}

#[derive(Debug, Clone)]
pub struct FileSessionManager {
    agents_root: PathBuf,
}

impl FileSessionManager {
    pub fn new<P: AsRef<Path>>(agents_root: P) -> Self {
        Self { agents_root: agents_root.as_ref().to_path_buf() }
    }

    fn agent_sessions_dir(&self, agent_id: &str) -> PathBuf {
        self.agents_root.join(agent_id).join("sessions")
    }

    fn file_store(&self, agent_id: &str) -> FileSessionEntryStore {
        FileSessionEntryStore::new(self.agent_sessions_dir(agent_id))
    }

    fn agent_ids(&self) -> StoreResult<Vec<String>> {
        let dir = match std::fs::read_dir(&self.agents_root) {
            Ok(dir) => dir,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::Io(e)),
        };

        let mut ids = Vec::new();
        for entry in dir {
            let entry = entry.map_err(StoreError::Io)?;
            if entry.path().is_dir() {
                if let Some(id) = entry.file_name().to_str() {
                    ids.push(id.to_string());
                }
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn session_matches(&self, agent_id: &str, session_id: &str) -> StoreResult<bool> {
        let store = self.file_store(agent_id);
        let summaries = store.list_sessions().map_err(StoreError::Io)?;
        Ok(summaries.iter().any(|summary| summary.session_id == session_id))
    }

    fn resolve_agent_for_session(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<String> {
        if let Some(agent_id) = agent_id {
            if self.session_matches(agent_id, session_id)? {
                return Ok(agent_id.to_string());
            }
            return Err(StoreError::NotFound(format!(
                "session {session_id} for agent {agent_id}"
            )));
        }

        let mut matches = Vec::new();
        for id in self.agent_ids()? {
            if self.session_matches(&id, session_id)? {
                matches.push(id);
            }
        }

        match matches.len() {
            0 => Err(StoreError::NotFound(format!("session {session_id}"))),
            1 => Ok(matches.remove(0)),
            _ => Err(StoreError::Internal(format!(
                "ambiguous session {session_id}: found under agents {}",
                matches.join(", ")
            ))),
        }
    }
}

#[async_trait]
impl SessionManager for FileSessionManager {
    fn entry_store_for_agent(&self, agent_id: &str) -> Arc<dyn SessionEntryStore> {
        Arc::new(self.file_store(agent_id))
    }

    async fn list_sessions(&self, agent_id: Option<&str>) -> StoreResult<Vec<SessionInfo>> {
        let agent_ids = match agent_id {
            Some(agent_id) => vec![agent_id.to_string()],
            None => self.agent_ids()?,
        };

        let mut sessions = Vec::new();
        for agent_id in agent_ids {
            let store = self.file_store(&agent_id);
            for summary in store.list_sessions().map_err(StoreError::Io)? {
                sessions.push(SessionInfo {
                    id: summary.session_id.clone(),
                    agent_id: agent_id.clone(),
                    session_id: summary.session_id,
                    entry_count: summary.entry_count,
                    created_at: summary.created_at,
                    updated_at: None,
                });
            }
        }
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }

    async fn session_exists(&self, agent_id: Option<&str>, session_id: &str) -> StoreResult<bool> {
        match self.resolve_agent_for_session(agent_id, session_id) {
            Ok(_) => Ok(true),
            Err(StoreError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn resolve_session_agent(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<String> {
        self.resolve_agent_for_session(agent_id, session_id)
    }

    async fn entry_store_for_session(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<Arc<dyn SessionEntryStore>> {
        let resolved_agent_id = self.resolve_agent_for_session(agent_id, session_id)?;
        Ok(Arc::new(self.file_store(&resolved_agent_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};

    fn entry(session_id: &str, id: &str, created_at: i64) -> SessionEntry {
        SessionEntry {
            id: id.to_string(),
            session_id: session_id.to_string(),
            created_at,
            parent_id: None,
            r#type: SessionEntryType::Summary,
            data: SessionEntryData::Summary { summary: format!("summary-{id}") },
        }
    }

    #[tokio::test]
    async fn file_manager_lists_sessions_with_agent_id() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FileSessionManager::new(temp.path().join("agents"));
        let alpha = manager.entry_store_for_agent("alpha");
        alpha.save(entry("session-a", "entry-a", 10)).await.unwrap();

        let sessions = manager.list_sessions(None).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session-a");
        assert_eq!(sessions[0].agent_id, "alpha");
        assert_eq!(sessions[0].session_id, "session-a");
        assert_eq!(sessions[0].entry_count, 1);
        assert_eq!(sessions[0].created_at, 10);
    }

    #[tokio::test]
    async fn file_manager_resolves_store_by_agent_and_session() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FileSessionManager::new(temp.path().join("agents"));
        let alpha = manager.entry_store_for_agent("alpha");
        alpha.save(entry("session-a", "entry-a", 10)).await.unwrap();

        let store = manager
            .entry_store_for_session(Some("alpha"), "session-a")
            .await
            .unwrap();
        let entries = store.get_entries("session-a").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "entry-a");
    }

    #[tokio::test]
    async fn file_manager_reports_ambiguous_unscoped_session() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FileSessionManager::new(temp.path().join("agents"));
        manager
            .entry_store_for_agent("alpha")
            .save(entry("same-session", "entry-a", 10))
            .await
            .unwrap();
        manager
            .entry_store_for_agent("beta")
            .save(entry("same-session", "entry-b", 20))
            .await
            .unwrap();

        let err = match manager
            .entry_store_for_session(None, "same-session")
            .await
        {
            Ok(_) => panic!("expected ambiguous session error"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("ambiguous session same-session"));
    }
}
