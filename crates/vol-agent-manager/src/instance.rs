use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use vol_session::Session;

/// Status of a running agent instance.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum InstanceStatus {
    Running,
    Completed,
    Failed,
}

/// Summary of a running instance for API responses.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentInstanceSummary {
    pub agent_type: String,
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub status: InstanceStatus,
    pub connection_count: usize,
    pub created_at: i64,
}

/// A running agent instance.
pub struct AgentInstance {
    pub agent_type: String,
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub session: Arc<Session>,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    pub ws_connections: std::sync::Mutex<HashSet<String>>,
    pub broadcast_tx: broadcast::Sender<serde_json::Value>,
}

impl AgentInstance {
    fn new(
        agent_type: String,
        session_id: String,
        parent_session_id: Option<String>,
        session: Arc<Session>,
    ) -> Self {
        let (tx, _) = broadcast::channel(64);
        Self {
            agent_type,
            session_id,
            parent_session_id,
            session,
            status: InstanceStatus::Running,
            created_at: Utc::now(),
            ws_connections: std::sync::Mutex::new(HashSet::new()),
            broadcast_tx: tx,
        }
    }
}

/// Thread-safe registry of running agent instances.
pub struct AgentInstanceRegistry {
    instances: Arc<tokio::sync::RwLock<Vec<Arc<AgentInstance>>>>,
}

impl AgentInstanceRegistry {
    pub fn new() -> Self {
        Self {
            instances: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Get existing instance or create new one.
    pub async fn get_or_create(
        &self,
        agent_type: &str,
        session_id: &str,
        parent_session_id: Option<String>,
        session: Arc<Session>,
    ) -> Arc<AgentInstance> {
        {
            let guard = self.instances.read().await;
            if let Some(instance) =
                guard.iter().find(|i| i.agent_type == agent_type && i.session_id == session_id)
            {
                return instance.clone();
            }
        }

        let instance = Arc::new(AgentInstance::new(
            agent_type.to_string(),
            session_id.to_string(),
            parent_session_id,
            session,
        ));

        let mut guard = self.instances.write().await;
        guard.push(instance.clone());
        instance
    }

    /// Add a WS connection to an instance.
    pub async fn add_connection(
        &self,
        agent_type: &str,
        session_id: &str,
        conn_id: String,
    ) {
        let guard = self.instances.read().await;
        if let Some(instance) =
            guard.iter().find(|i| i.agent_type == agent_type && i.session_id == session_id)
        {
            let mut conns = instance.ws_connections.lock().unwrap();
            conns.insert(conn_id);
        }
    }

    /// Remove a WS connection from an instance.
    pub async fn remove_connection(
        &self,
        agent_type: &str,
        session_id: &str,
        conn_id: &str,
    ) {
        let guard = self.instances.read().await;
        if let Some(instance) =
            guard.iter().find(|i| i.agent_type == agent_type && i.session_id == session_id)
        {
            instance.ws_connections.lock().unwrap().remove(conn_id);
        }
    }

    /// Get broadcast sender for an instance.
    pub async fn get_broadcast(
        &self,
        agent_type: &str,
        session_id: &str,
    ) -> Option<broadcast::Sender<serde_json::Value>> {
        let guard = self.instances.read().await;
        let instance = guard
            .iter()
            .find(|i| i.agent_type == agent_type && i.session_id == session_id)?;
        Some(instance.broadcast_tx.clone())
    }

    /// List all running instances.
    pub async fn list_instances(&self) -> Vec<AgentInstanceSummary> {
        let guard = self.instances.read().await;
        guard
            .iter()
            .map(|i| AgentInstanceSummary {
                agent_type: i.agent_type.clone(),
                session_id: i.session_id.clone(),
                parent_session_id: i.parent_session_id.clone(),
                status: i.status,
                connection_count: i.ws_connections.lock().unwrap().len(),
                created_at: i.session.created_at,
            })
            .collect()
    }

    /// Destroy an instance.
    pub async fn destroy(&self, agent_type: &str, session_id: &str) {
        let mut guard = self.instances.write().await;
        guard.retain(|i| !(i.agent_type == agent_type && i.session_id == session_id));
    }
}

impl Default for AgentInstanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_session::InMemoryEntryStore;

    #[tokio::test]
    async fn test_registry_get_or_create_new() {
        let registry = AgentInstanceRegistry::new();
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Arc::new(Session::new(entry_store));
        let instance = registry.get_or_create("qa", "sess-1", None, session).await;
        assert_eq!(instance.agent_type, "qa");
        assert_eq!(instance.session_id, "sess-1");
    }

    #[tokio::test]
    async fn test_registry_get_or_create_returns_existing() {
        let registry = AgentInstanceRegistry::new();
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session1 = Arc::new(Session::new(entry_store.clone()));
        let first = registry.get_or_create("qa", "sess-1", None, session1).await;
        let entry_store2 = Arc::new(InMemoryEntryStore::new());
        let session2 = Arc::new(Session::new(entry_store2));
        let second = registry.get_or_create("qa", "sess-1", None, session2).await;
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn test_registry_list_instances() {
        let registry = AgentInstanceRegistry::new();
        let make_session = || {
            let entry_store = Arc::new(InMemoryEntryStore::new());
            Arc::new(Session::new(entry_store))
        };
        registry.get_or_create("qa", "s1", None, make_session()).await;
        registry.get_or_create("code", "s2", None, make_session()).await;
        let list = registry.list_instances().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_registry_destroy() {
        let registry = AgentInstanceRegistry::new();
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Arc::new(Session::new(entry_store));
        registry.get_or_create("qa", "sess-1", None, session).await;
        registry.destroy("qa", "sess-1").await;
        assert!(registry.list_instances().await.is_empty());
    }
}
