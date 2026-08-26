use crate::{SandboxError, SandboxId, SandboxResult, SandboxStatus};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRecord {
    pub id: SandboxId,
    pub profile: String,
    pub provider_kind: String,
    pub backend_id: String,
    pub status: SandboxStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct SandboxFilter {
    pub profile: Option<String>,
    pub provider_kind: Option<String>,
    pub status: Option<SandboxStatus>,
}

#[async_trait]
pub trait SandboxStore: Send + Sync {
    async fn insert(&self, record: SandboxRecord) -> SandboxResult<()>;
    async fn get(&self, id: &SandboxId) -> SandboxResult<Option<SandboxRecord>>;
    async fn list(&self, filter: Option<SandboxFilter>) -> SandboxResult<Vec<SandboxRecord>>;
    async fn update_status(&self, id: &SandboxId, status: SandboxStatus) -> SandboxResult<()>;
    async fn delete(&self, id: &SandboxId) -> SandboxResult<()>;
}

pub struct InMemorySandboxStore {
    records: RwLock<HashMap<SandboxId, SandboxRecord>>,
}

impl InMemorySandboxStore {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemorySandboxStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SandboxStore for InMemorySandboxStore {
    async fn insert(&self, record: SandboxRecord) -> SandboxResult<()> {
        let mut records = self.records.write().await;
        records.insert(record.id.clone(), record);
        Ok(())
    }

    async fn get(&self, id: &SandboxId) -> SandboxResult<Option<SandboxRecord>> {
        let records = self.records.read().await;
        Ok(records.get(id).cloned())
    }

    async fn list(&self, filter: Option<SandboxFilter>) -> SandboxResult<Vec<SandboxRecord>> {
        let records = self.records.read().await;
        let results: Vec<SandboxRecord> = records
            .values()
            .filter(|r| {
                if let Some(ref f) = filter {
                    if let Some(ref profile) = f.profile {
                        if &r.profile != profile {
                            return false;
                        }
                    }
                    if let Some(ref provider_kind) = f.provider_kind {
                        if &r.provider_kind != provider_kind {
                            return false;
                        }
                    }
                    if let Some(status) = f.status {
                        if r.status != status {
                            return false;
                        }
                    }
                }
                true
            })
            .cloned()
            .collect();
        Ok(results)
    }

    async fn update_status(&self, id: &SandboxId, status: SandboxStatus) -> SandboxResult<()> {
        let mut records = self.records.write().await;
        if let Some(record) = records.get_mut(id) {
            record.status = status;
            record.updated_at = Utc::now();
            Ok(())
        } else {
            Err(SandboxError::NotFound(id.to_string()))
        }
    }

    async fn delete(&self, id: &SandboxId) -> SandboxResult<()> {
        let mut records = self.records.write().await;
        records.remove(id);
        Ok(())
    }
}
