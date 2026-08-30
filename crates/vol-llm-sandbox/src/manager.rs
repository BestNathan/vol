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

/// Reserved profile name for the manager's implicit scratch sandbox, created
/// on demand by [`SandboxManager::default`]. Reserved because the manager
/// treats a store record with this profile as *the* default instance.
pub const DEFAULT_TMP_PROFILE: &str = "default-tmp";

/// Unified orchestration service for sandbox lifecycle management.
///
/// The sole sandbox resolution path: manages sandbox profiles (specs), routes
/// to providers, and tracks instances via a `SandboxStore`.
pub struct SandboxManager {
    providers: RwLock<HashMap<String, Arc<dyn SandboxProvider>>>,
    store: Arc<dyn SandboxStore>,
    specs: RwLock<HashMap<String, SandboxSpec>>,
    /// Cache of live sandbox handles keyed by backend_id.
    instances: RwLock<HashMap<String, Arc<dyn Sandbox>>>,
    /// Maps profile name → backend_id for fast `acquire_by_name` lookup.
    name_to_backend: RwLock<HashMap<String, String>>,
    /// Serializes `default()`'s check-then-create.
    default_lock: tokio::sync::Mutex<()>,
}

impl SandboxManager {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            store: Arc::new(InMemorySandboxStore::new()),
            specs: RwLock::new(HashMap::new()),
            instances: RwLock::new(HashMap::new()),
            name_to_backend: RwLock::new(HashMap::new()),
            default_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn with_store(store: Arc<dyn SandboxStore>) -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            store,
            specs: RwLock::new(HashMap::new()),
            instances: RwLock::new(HashMap::new()),
            name_to_backend: RwLock::new(HashMap::new()),
            default_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Register a provider for a sandbox kind.
    pub async fn register_provider(&self, provider: Arc<dyn SandboxProvider>) {
        let kind = provider.kind().to_string();
        self.providers.write().await.insert(kind, provider);
    }

    /// Load sandbox profiles from a directory, reading `*.toml` files and
    /// parsing each as a [`SandboxSpec`].
    ///
    /// Registers specs only — **no sandbox is instantiated here**. Instances
    /// are created on demand by [`Self::acquire_by_name`], so a profile nobody
    /// references costs nothing beyond the parsed spec. A missing directory is
    /// not an error.
    ///
    /// Per-file parse failures are logged at `warn` and skipped, so one broken
    /// profile does not prevent startup. Callers that need to know whether
    /// every configured profile actually loaded must compare the file count
    /// against [`Self::list_specs`] themselves — otherwise a total parse
    /// failure looks identical to a healthy start with no profiles on disk.
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

    /// Look up a sandbox by profile name, creating it on first access.
    ///
    /// The primary entry point for profile-name resolution: returns a cached
    /// instance if one exists for the profile, otherwise creates one via the
    /// registered provider and caches it.
    pub async fn acquire_by_name(&self, name: &str) -> Option<SandboxRef> {
        // Fast path: already cached.
        {
            let name_map = self.name_to_backend.read().await;
            if let Some(backend_id) = name_map.get(name) {
                let instances = self.instances.read().await;
                if let Some(sandbox) = instances.get(backend_id) {
                    return Some(sandbox.clone());
                }
            }
        }

        // Slow path: create via provider.
        let spec = {
            let specs = self.specs.read().await;
            specs.get(name)?.clone()
        };
        let provider = {
            let providers = self.providers.read().await;
            providers.get(spec.provider()).cloned()?
        };

        match provider.create(&spec).await {
            Ok(backend_ref) => {
                let sandbox = backend_ref.sandbox.clone();
                self.instances
                    .write()
                    .await
                    .insert(backend_ref.backend_id.clone(), backend_ref.sandbox);
                self.name_to_backend
                    .write()
                    .await
                    .insert(name.to_string(), backend_ref.backend_id);
                Some(sandbox)
            }
            Err(e) => {
                tracing::warn!(
                    profile = name,
                    error = %e,
                    "failed to create sandbox for profile"
                );
                None
            }
        }
    }

    /// Build a one-off sandbox from a spec without caching.
    ///
    /// Used for inline `[sandbox]` blocks in cli-tool configs — the
    /// resulting sandbox is owned by the cli-tool, not tracked by the
    /// manager.
    pub async fn build_inline(&self, spec: &SandboxSpec) -> SandboxResult<SandboxRef> {
        let provider = {
            let providers = self.providers.read().await;
            providers.get(spec.provider()).cloned().ok_or_else(|| {
                SandboxError::UnknownType(format!(
                    "no provider registered for kind '{}'",
                    spec.provider()
                ))
            })?
        };
        let backend_ref = provider.create(spec).await?;
        Ok(backend_ref.sandbox)
    }

    /// Fall back to a fresh TmpSandbox when no named sandbox is available.
    ///
    /// Falls back further to a `LocalSandbox` if the tmp sandbox cannot be built.
    pub async fn default_tmp(&self) -> SandboxRef {
        match self
            .build_inline(&SandboxSpec {
                name: "default-tmp".to_string(),
                config: crate::SandboxProviderConfig::Tmp {
                    work_dir: None,
                    sub_dir: None,
                },
                metadata: HashMap::new(),
            })
            .await
        {
            Ok(sb) => sb,
            Err(e) => {
                tracing::warn!(error = %e, "default_tmp: TmpSandbox build failed, using LocalSandbox");
                Arc::new(crate::local::LocalSandbox::new(None))
            }
        }
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
            .insert(backend_ref.backend_id.clone(), backend_ref.sandbox);
        self.name_to_backend
            .write()
            .await
            .insert(spec.name.clone(), backend_ref.backend_id);

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

    /// List all sandbox spec profiles (templates).
    pub async fn list_specs(&self) -> Vec<crate::SandboxSpec> {
        self.specs.read().await.values().cloned().collect()
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
        // Also remove any profile→backend mapping for this record.
        {
            let mut name_map = self.name_to_backend.write().await;
            name_map.retain(|_, backend_id| backend_id != &record.backend_id);
        }
        self.store.delete(id).await?;
        Ok(())
    }

    /// Get the default sandbox — an implicit, lazily-created scratch sandbox.
    ///
    /// Idempotent: the instance is keyed on the reserved
    /// [`DEFAULT_TMP_PROFILE`] profile, so repeated calls return the same
    /// sandbox and the store does not grow.
    ///
    /// An unrelated instance is never returned. Callers that need a specific
    /// sandbox must resolve it by name via [`Self::acquire_by_name`] — this
    /// method only ever yields the scratch sandbox.
    pub async fn default(&self) -> SandboxResult<SandboxRef> {
        // Serialize the check-then-create so concurrent callers cannot each
        // create their own "default" instance.
        let _guard = self.default_lock.lock().await;

        let existing = self
            .store
            .list(Some(SandboxFilter {
                profile: Some(DEFAULT_TMP_PROFILE.to_string()),
                provider_kind: None,
                status: None,
            }))
            .await?;
        if let Some(record) = existing.first() {
            return self.get(&record.id).await;
        }

        let spec = SandboxSpec {
            name: DEFAULT_TMP_PROFILE.to_string(),
            config: crate::SandboxProviderConfig::Tmp {
                work_dir: None,
                sub_dir: None,
            },
            metadata: HashMap::new(),
        };

        let provider = {
            let providers = self.providers.read().await;
            providers.get("tmp").cloned()
        };

        let Some(provider) = provider else {
            // No tmp provider registered. Fall back to a bare LocalSandbox,
            // deliberately not recorded in the store: it is reconstructible
            // from config alone and so has no instance identity to track.
            return Ok(Arc::new(crate::local::LocalSandbox::new(None)));
        };

        let backend_ref = provider.create(&spec).await?;
        let record = SandboxRecord {
            id: SandboxId::new(),
            profile: DEFAULT_TMP_PROFILE.to_string(),
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
            .insert(backend_ref.backend_id, backend_ref.sandbox.clone());
        Ok(backend_ref.sandbox)
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
        self.instances
            .write()
            .await
            .insert(backend_id.clone(), sandbox);
        self.name_to_backend
            .write()
            .await
            .insert(spec.name.clone(), backend_id);
        Ok(id)
    }

    fn validate_transition(from: SandboxStatus, to: SandboxStatus) -> SandboxResult<()> {
        let valid = matches!(
            (from, to),
            (SandboxStatus::Created, SandboxStatus::Starting)
                | (SandboxStatus::Created, SandboxStatus::Running)
                | (SandboxStatus::Starting, SandboxStatus::Running)
                | (SandboxStatus::Running, SandboxStatus::Pausing)
                | (SandboxStatus::Running, SandboxStatus::Stopping)
                | (SandboxStatus::Running, SandboxStatus::Stopped)
                | (SandboxStatus::Pausing, SandboxStatus::Paused)
                | (SandboxStatus::Paused, SandboxStatus::Starting)
                | (SandboxStatus::Paused, SandboxStatus::Running)
                | (SandboxStatus::Paused, SandboxStatus::Stopping)
                | (SandboxStatus::Paused, SandboxStatus::Stopped)
                | (SandboxStatus::Stopping, SandboxStatus::Stopped)
                | (SandboxStatus::Stopped, SandboxStatus::Starting)
                | (SandboxStatus::Stopped, SandboxStatus::Running)
                | (SandboxStatus::Stopped, SandboxStatus::Destroying)
                | (SandboxStatus::Stopped, SandboxStatus::Destroyed)
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
