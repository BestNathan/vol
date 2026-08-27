use crate::{
    InMemorySandboxStore, Sandbox, SandboxCapabilities, SandboxError, SandboxFilter, SandboxId,
    SandboxInfo, SandboxProvider, SandboxRecord, SandboxRef, SandboxResult, SandboxSpec,
    SandboxStatus, SandboxStore,
};
use chrono::Utc;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unified orchestration service for sandbox lifecycle management.
///
/// Replaces `SandboxRegistry`. Manages sandbox profiles (specs), routes to providers,
/// and tracks instances via a `SandboxStore`.
pub struct SandboxManager {
    providers: RwLock<HashMap<String, Arc<dyn SandboxProvider>>>,
    store: Arc<dyn SandboxStore>,
    specs: RwLock<HashMap<String, SandboxSpec>>,
    /// Cache of live sandbox handles keyed by backend_id.
    instances: RwLock<HashMap<String, Arc<dyn Sandbox>>>,
}

impl SandboxManager {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            store: Arc::new(InMemorySandboxStore::new()),
            specs: RwLock::new(HashMap::new()),
            instances: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_store(store: Arc<dyn SandboxStore>) -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            store,
            specs: RwLock::new(HashMap::new()),
            instances: RwLock::new(HashMap::new()),
        }
    }

    /// Register a provider for a sandbox kind.
    pub async fn register_provider(&self, provider: Arc<dyn SandboxProvider>) {
        let kind = provider.kind().to_string();
        self.providers.write().await.insert(kind, provider);
    }

    /// Load sandbox profiles from a directory.
    /// Reads *.toml files and parses them as SandboxSpec.
    pub async fn load_profiles(&self, sandboxes_dir: &Path) -> SandboxResult<()> {
        if !sandboxes_dir.exists() {
            return Ok(());
        }
        let entries = std::fs::read_dir(sandboxes_dir).map_err(SandboxError::Io)?;
        for entry in entries {
            let entry = entry.map_err(SandboxError::Io)?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let content = std::fs::read_to_string(&path).map_err(SandboxError::Io)?;
            match toml::from_str::<SandboxSpec>(&content) {
                Ok(spec) => {
                    self.specs.write().await.insert(spec.name.clone(), spec);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse sandbox config {}: {}", path.display(), e);
                }
            }
        }
        Ok(())
    }

    /// Register a profile spec programmatically.
    pub async fn register_profile(&self, spec: SandboxSpec) {
        self.specs.write().await.insert(spec.name.clone(), spec);
    }

    /// Create a new sandbox instance from a profile.
    pub async fn create(&self, profile: &str) -> SandboxResult<SandboxId> {
        let spec = {
            let specs = self.specs.read().await;
            specs
                .get(profile)
                .cloned()
                .ok_or_else(|| SandboxError::NotFound(format!("profile: {profile}")))?
        };

        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(spec.provider())
                .cloned()
                .ok_or_else(|| SandboxError::UnknownType(spec.provider().to_string()))?
        };

        let backend_ref = provider.create(&spec).await?;
        let id = SandboxId::new();

        let record = SandboxRecord {
            id: id.clone(),
            profile: spec.name.clone(),
            provider_kind: spec.provider().to_string(),
            backend_id: backend_ref.backend_id.clone(),
            status: SandboxStatus::Running,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: spec.metadata.clone(),
        };
        self.store.insert(record).await?;

        // Cache the sandbox handle
        self.instances
            .write()
            .await
            .insert(backend_ref.backend_id, backend_ref.sandbox);

        Ok(id)
    }

    /// Get a sandbox handle by ID.
    pub async fn get(&self, id: &SandboxId) -> SandboxResult<SandboxRef> {
        let record = self
            .store
            .get(id)
            .await?
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;

        // Check cache first
        {
            let instances = self.instances.read().await;
            if let Some(sandbox) = instances.get(&record.backend_id) {
                return Ok(sandbox.clone());
            }
        }

        // Fall back to provider
        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&record.provider_kind)
                .cloned()
                .ok_or_else(|| SandboxError::UnknownType(record.provider_kind.clone()))?
        };

        let sandbox = provider.get(&record.backend_id).await?;
        self.instances
            .write()
            .await
            .insert(record.backend_id, sandbox.clone());
        Ok(sandbox)
    }

    /// List all sandbox instances.
    pub async fn list(&self, filter: Option<SandboxFilter>) -> SandboxResult<Vec<SandboxInfo>> {
        let records = self.store.list(filter).await?;
        let mut infos = Vec::new();
        for record in records {
            let caps = {
                let providers = self.providers.read().await;
                providers
                    .get(&record.provider_kind)
                    .map(|p| p.capabilities())
                    .unwrap_or(SandboxCapabilities {
                        persistent: false,
                        pausable: false,
                        stoppable: false,
                        destroyable: false,
                    })
            };
            let root_path = {
                let instances = self.instances.read().await;
                instances
                    .get(&record.backend_id)
                    .and_then(|s| s.root_path().map(|p| p.to_string_lossy().to_string()))
            };
            infos.push(SandboxInfo {
                id: record.id.to_string(),
                profile: record.profile,
                kind: record.provider_kind,
                status: format!("{:?}", record.status).to_lowercase(),
                root_path,
                capabilities: caps,
            });
        }
        Ok(infos)
    }

    /// Start a sandbox instance.
    pub async fn start(&self, id: &SandboxId) -> SandboxResult<()> {
        let record = self
            .store
            .get(id)
            .await?
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;

        Self::validate_transition(record.status, SandboxStatus::Running)?;

        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&record.provider_kind)
                .cloned()
                .ok_or_else(|| SandboxError::UnknownType(record.provider_kind.clone()))?
        };

        provider.start(&record.backend_id).await?;
        self.store.update_status(id, SandboxStatus::Running).await?;
        Ok(())
    }

    /// Stop a sandbox instance.
    pub async fn stop(&self, id: &SandboxId) -> SandboxResult<()> {
        let record = self
            .store
            .get(id)
            .await?
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;

        Self::validate_transition(record.status, SandboxStatus::Stopped)?;

        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&record.provider_kind)
                .cloned()
                .ok_or_else(|| SandboxError::UnknownType(record.provider_kind.clone()))?
        };

        provider.stop(&record.backend_id).await?;
        self.store.update_status(id, SandboxStatus::Stopped).await?;
        Ok(())
    }

    /// Destroy a sandbox instance.
    pub async fn destroy(&self, id: &SandboxId) -> SandboxResult<()> {
        let record = self
            .store
            .get(id)
            .await?
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;

        let provider = {
            let providers = self.providers.read().await;
            providers
                .get(&record.provider_kind)
                .cloned()
                .ok_or_else(|| SandboxError::UnknownType(record.provider_kind.clone()))?
        };

        provider.destroy(&record.backend_id).await?;
        self.instances.write().await.remove(&record.backend_id);
        self.store.delete(id).await?;
        Ok(())
    }

    /// Get the default sandbox.
    /// If exactly one sandbox exists, return it.
    /// Otherwise create a fresh TmpSandbox.
    pub async fn default(&self) -> SandboxResult<SandboxRef> {
        let records = self.store.list(None).await?;
        if records.len() == 1 {
            let first = records
                .first()
                .ok_or_else(|| SandboxError::NotFound("no sandbox instances found".to_string()))?;
            return self.get(&first.id).await;
        }
        // Create a fresh TmpSandbox
        let spec = SandboxSpec {
            name: "default-tmp".to_string(),
            config: crate::SandboxProviderConfig::Tmp { sub_dir: None },
            metadata: HashMap::new(),
        };

        let provider = {
            let providers = self.providers.read().await;
            providers.get("tmp").cloned()
        };

        if let Some(provider) = provider {
            let backend_ref = provider.create(&spec).await?;
            let id = SandboxId::new();
            let record = SandboxRecord {
                id: id.clone(),
                profile: "default-tmp".to_string(),
                provider_kind: "tmp".to_string(),
                backend_id: backend_ref.backend_id.clone(),
                status: SandboxStatus::Running,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                metadata: HashMap::new(),
            };
            self.store.insert(record).await?;
            self.instances
                .write()
                .await
                .insert(backend_ref.backend_id.clone(), backend_ref.sandbox.clone());
            Ok(backend_ref.sandbox)
        } else {
            // Fallback: create a LocalSandbox directly
            Ok(Arc::new(crate::local::LocalSandbox::new(None)))
        }
    }

    /// Register a pre-existing sandbox instance (backward compat).
    pub async fn register_instance(
        &self,
        spec: SandboxSpec,
        sandbox: SandboxRef,
    ) -> SandboxResult<SandboxId> {
        let id = SandboxId::new();
        let backend_id = sandbox
            .root_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let record = SandboxRecord {
            id: id.clone(),
            profile: spec.name.clone(),
            provider_kind: spec.provider().to_string(),
            backend_id: backend_id.clone(),
            status: SandboxStatus::Running,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: spec.metadata.clone(),
        };
        self.store.insert(record).await?;
        self.instances.write().await.insert(backend_id, sandbox);
        Ok(id)
    }

    fn validate_transition(from: SandboxStatus, to: SandboxStatus) -> SandboxResult<()> {
        let valid = matches!(
            (from, to),
            (SandboxStatus::Created, SandboxStatus::Starting)
                | (SandboxStatus::Starting, SandboxStatus::Running)
                | (SandboxStatus::Running, SandboxStatus::Pausing)
                | (SandboxStatus::Pausing, SandboxStatus::Paused)
                | (SandboxStatus::Paused, SandboxStatus::Starting)
                | (SandboxStatus::Running, SandboxStatus::Stopping)
                | (SandboxStatus::Paused, SandboxStatus::Stopping)
                | (SandboxStatus::Stopping, SandboxStatus::Stopped)
                | (SandboxStatus::Stopped, SandboxStatus::Destroying)
                | (SandboxStatus::Destroying, SandboxStatus::Destroyed)
                | (_, SandboxStatus::Failed)
        );
        if valid {
            Ok(())
        } else {
            Err(SandboxError::InvalidTransition { from, to })
        }
    }
}

impl Default for SandboxManager {
    fn default() -> Self {
        Self::new()
    }
}
