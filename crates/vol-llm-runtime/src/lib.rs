//! AgentRuntime — core runtime for the multi-agent system.
//!
//! Owns all runtime resources: LLM registry, tool registry, task store,
//! agent definitions, and agent status tracking.
//! Provides lifecycle methods (run/stop) and agent registration.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::AgentConfig;
use vol_llm_agent::ReActAgent;
use vol_llm_agent::AgentLoader;
use vol_llm_mcp::{McpConfig, McpManager};
use vol_llm_provider::{create_provider, ProviderLoader};
use vol_llm_skill::{SkillLoader, SkillTool};
use vol_llm_task::{DatabaseTaskStore, FileTaskStore};
use vol_llm_task::TaskStore;
use vol_llm_tool::ToolRegistry;
use vol_session::file_store::FileSessionEntryStore;
use vol_session::Session;

/// Runtime status of a registered agent.
#[derive(Debug, Clone, Default)]
pub struct AgentStatus {
    pub status: String, // "idle" | "running"
    pub current_input: Option<String>,
    pub run_id: Option<String>,
}

impl AgentStatus {
    pub fn idle() -> Self {
        Self { status: "idle".into(), current_input: None, run_id: None }
    }
    pub fn running(input: String, run_id: String) -> Self {
        Self { status: "running".into(), current_input: Some(input), run_id: Some(run_id) }
    }
}

/// Handle returned by AgentRuntime::run(), used to control runtime lifecycle.
pub struct AgentRuntimeHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    pub join_handle: tokio::task::JoinHandle<()>,
}

impl AgentRuntimeHandle {
    pub async fn stop(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.join_handle.await;
    }
}

/// Core agent runtime. Owns all shared resources and manages agent lifecycle.
pub struct AgentRuntime {
    working_dir: PathBuf,
    store_dir: PathBuf,
    pub llm_registry: ProviderLoader,
    pub tool_registry: Arc<ToolRegistry>,
    pub task_store: Arc<dyn TaskStore>,
    pub mcp_manager: Arc<McpManager>,
    pub sandbox_registry: Arc<vol_llm_sandbox::registry::SandboxRegistry>,
    pub skill_loader: Arc<SkillLoader>,
    pub agent_defs: Arc<std::sync::RwLock<HashMap<String, AgentDef>>>,
    pub agent_status: Arc<std::sync::RwLock<HashMap<String, AgentStatus>>>,
}

impl AgentRuntime {
    pub fn builder(working_dir: impl Into<PathBuf>, store_dir: impl Into<PathBuf>) -> AgentRuntimeBuilder {
        AgentRuntimeBuilder::new(working_dir.into(), store_dir.into())
    }

    pub fn working_dir(&self) -> &std::path::Path {
        &self.working_dir
    }

    pub fn store_dir(&self) -> &std::path::Path {
        &self.store_dir
    }

    /// Resolve the LLM client for an agent definition.
    /// Tries agent-specified model first, falls back to first available provider.
    pub fn resolve_llm_for_agent(&self, def: &AgentDef) -> Result<Arc<dyn vol_llm_core::LLMClient>, String> {
        if let Some(ref model_name) = def.model {
            if let Some(fc) = self.llm_registry.get(model_name) {
                return create_provider(&fc.to_llm_config())
                    .map(Arc::from)
                    .map_err(|e| format!("LLM error for '{}': {}", model_name, e));
            }
        }
        let ids = self.llm_registry.ids();
        let first_id = ids.first()
            .ok_or_else(|| "No LLM providers configured".to_string())?;
        let fc = self.llm_registry.get(first_id)
            .ok_or_else(|| "Provider not found".to_string())?;
        create_provider(&fc.to_llm_config())
            .map(Arc::from)
            .map_err(|e| format!("LLM error: {}", e))
    }

    /// Register an agent into the runtime. Returns the created ReActAgent.
    pub async fn register_agent(
        &self,
        agent_id: impl Into<String>,
        def: AgentDef,
    ) -> Result<ReActAgent, String> {
        let agent_id = agent_id.into();
        let agent_dir = self.store_dir.join("agents").join(&agent_id);
        let sessions_dir = agent_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir)
            .map_err(|e| format!("failed to create agent dirs: {e}"))?;

        let llm = self.resolve_llm_for_agent(&def)?;

        let session_store = Arc::new(FileSessionEntryStore::new(&sessions_dir));
        let session = Arc::new(Session::new(session_store));

        let mut config = AgentConfig::builder()
            .with_def(def.clone())
            .with_llm(llm)
            .with_tools(self.tool_registry.clone())
            .with_session(session)
            .with_working_dir(agent_dir.clone())
            .build()
            .expect("AgentConfig build failed — all required fields provided");

        config.mcp_manager = Some(self.mcp_manager.clone());

        let agent = ReActAgent::new(config);

        self.agent_defs.write().unwrap().insert(agent_id.clone(), def);
        self.agent_status.write().unwrap().insert(agent_id, AgentStatus::idle());

        Ok(agent)
    }

    /// Discover and register all agents from .agents/agents/ directories.
    pub async fn discover_agents(&self) -> Result<Vec<(String, ReActAgent)>, String> {
        let loader = AgentLoader::new(Some(self.working_dir.clone()));
        loader.discover_all().await.map_err(|e| e.to_string())?;

        let mut registered = Vec::new();
        let agents = loader.list_metadata().await;
        for meta in agents {
            if let Some(def) = loader.get(&meta.name).await {
                let arc_def = Arc::try_unwrap(def).unwrap_or_else(|arc| (*arc).clone());
                let agent = self.register_agent(&meta.name, arc_def).await?;
                registered.push((meta.name, agent));
            }
        }
        Ok(registered)
    }

    /// Start the runtime: connect MCP, discover skills/agents, return handle.
    pub async fn run(&self) -> AgentRuntimeHandle {
        let mcp = self.mcp_manager.clone();
        tokio::spawn(async move { let _ = mcp.connect().await; });

        let skill = self.skill_loader.clone();
        tokio::spawn(async move { let _ = skill.discover_all().await; });

        if let Err(e) = self.discover_agents().await {
            tracing::warn!(error = %e, "Failed to discover agents at runtime start");
        }

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let status_map = self.agent_status.clone();
        let join_handle = tokio::spawn(async move {
            let _ = shutdown_rx.try_recv();
            tracing::info!("AgentRuntime shutdown signal received");
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                let all_idle = status_map.read().unwrap().values().all(|s| s.status == "idle");
                if all_idle || tokio::time::Instant::now() > deadline {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            tracing::info!("AgentRuntime stopped");
        });

        AgentRuntimeHandle { shutdown_tx, join_handle }
    }
}

impl AgentRuntime {
    #[doc(hidden)]
    pub async fn for_test() -> Self {
        let store_dir = PathBuf::from("/tmp/vol-llm-runtime-test");
        let working_dir = PathBuf::from(".");

        let llm_registry = ProviderLoader::load(Some(&working_dir));
        let llm_registry = if llm_registry.is_empty() { ProviderLoader::default() } else { llm_registry };
        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        let task_store: Arc<dyn TaskStore> = Arc::new(vol_llm_task::InMemoryTaskStore::new());
        // Register the unified CLI-style `task` tool (agents using `tools: [task]`).
        vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
        let tool_registry = Arc::new(tool_registry);
        let mcp_manager = Arc::new(McpManager::new(vec![]));
        let sandbox_registry = {
            let tmp = std::env::temp_dir().join("vol-llm-runtime-test-sandboxes");
            let _ = std::fs::create_dir_all(&tmp);
            Arc::new(
                vol_llm_sandbox::registry::SandboxRegistry::load(&tmp).await
                    .expect("SandboxRegistry init in for_test")
            )
        };
        let skill_loader = Arc::new(SkillLoader::new_empty());

        AgentRuntime {
            working_dir,
            store_dir,
            llm_registry,
            tool_registry,
            task_store,
            mcp_manager,
            sandbox_registry,
            skill_loader,
            agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
            agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }
}

// === Builder ===

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStoreType {
    File,
    Database,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TaskStoreConfig {
    #[serde(rename = "type")]
    pub store_type: TaskStoreType,
    pub url: Option<String>,
}

impl TaskStoreConfig {
    pub fn validate(&self) -> Result<(), String> {
        match self.store_type {
            TaskStoreType::File => {
                if self.url.is_some() {
                    return Err("runtime.task_store.url is not valid when type = \"file\"".to_string());
                }
                Ok(())
            }
            TaskStoreType::Database => {
                let url = self
                    .url
                    .as_deref()
                    .ok_or_else(|| "runtime.task_store.url is required when type = \"database\"".to_string())?;
                validate_database_url_scheme(url)
            }
        }
    }

    pub fn required_url(&self) -> Result<&str, String> {
        self.url
            .as_deref()
            .ok_or_else(|| "runtime.task_store.url is required when type = \"database\"".to_string())
    }
}

pub fn validate_database_url_scheme(url: &str) -> Result<(), String> {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .unwrap_or_default();

    match scheme {
        "sqlite" | "postgres" | "postgresql" | "mysql" => Ok(()),
        "" => Err("unsupported task store database url scheme: <missing>".to_string()),
        other => Err(format!("unsupported task store database url scheme: {other}")),
    }
}

pub struct AgentRuntimeBuilder {
    working_dir: PathBuf,
    store_dir: PathBuf,
    task_store_config: Option<TaskStoreConfig>,
}

impl AgentRuntimeBuilder {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        Self { working_dir, store_dir, task_store_config: None }
    }

    pub fn with_task_store_config(mut self, config: Option<TaskStoreConfig>) -> Self {
        self.task_store_config = config;
        self
    }

    pub async fn build(self) -> Result<AgentRuntime, String> {
        let store_dir = expand_tilde(self.store_dir);
        let agents_root = store_dir.join("agents");
        std::fs::create_dir_all(&agents_root)
            .map_err(|e| format!("failed to create agents dir: {e}"))?;

        let llm_registry = ProviderLoader::load(Some(&self.working_dir));
        if llm_registry.is_empty() {
            return Err("No LLM provider configured in .agents/providers/*.toml".to_string());
        }

        let mcp_manager = {
            let configs = McpConfig::load(Some(&self.working_dir))
                .map(|c| c.servers().to_vec())
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to load MCP config: {}", e);
                    vec![]
                });
            let manager = McpManager::new(configs);
            // Connect synchronously so tools are cached before we register them.
            if let Err(e) = manager.connect().await {
                tracing::warn!("MCP connect error (tools may be unavailable): {}", e);
            }
            Arc::new(manager)
        };

        let sandbox_registry = {
            let sandboxes_dir = self.working_dir.join(".agents").join("sandboxes");
            vol_llm_sandbox::registry::SandboxRegistry::load(&sandboxes_dir).await
                .map_err(|e| format!("Sandbox registry init failed: {}", e))?
        };
        let sandbox_registry = Arc::new(sandbox_registry);
        let skill_loader = {
            let loader = Arc::new(SkillLoader::new(Some(self.working_dir.clone())));
            let ld = Arc::clone(&loader);
            tokio::spawn(async move { let _ = ld.discover_all().await; });
            loader
        };

        let task_store: Arc<dyn TaskStore> = match self.task_store_config.as_ref() {
            None => build_file_task_store(&store_dir).await?,
            Some(config) if config.store_type == TaskStoreType::File => build_file_task_store(&store_dir).await?,
            Some(config) if config.store_type == TaskStoreType::Database => {
                build_database_task_store(config.required_url()?).await?
            }
            Some(_) => return Err("unsupported task store configuration".to_string()),
        };

        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        // Register the unified CLI-style `task` tool (agents using `tools: [task]`).
        vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
        let tool_config = vol_llm_tool::ToolConfig::default();
        vol_llm_tools_builtin::register_web_all(&mut tool_registry, &tool_config);
        tool_registry.register(SkillTool::new(skill_loader.clone()));
        let mcp_count = tool_registry.register_from_mcp(mcp_manager.clone()).await;
        if mcp_count > 0 {
            tracing::info!(mcp_count, "MCP tools registered");
        }
        let tool_registry = Arc::new(tool_registry);

        Ok(AgentRuntime {
            working_dir: self.working_dir,
            store_dir,
            llm_registry,
            tool_registry,
            task_store,
            mcp_manager,
            sandbox_registry,
            skill_loader,
            agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
            agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
        })
    }
}

async fn build_file_task_store(store_dir: &std::path::Path) -> Result<Arc<dyn TaskStore>, String> {
    let tasks_dir = store_dir.join("tasks");
    std::fs::create_dir_all(&tasks_dir)
        .map_err(|e| format!("failed to create tasks dir: {e}"))?;
    let store = FileTaskStore::new(&tasks_dir)
        .await
        .map_err(|e| format!("failed to create file task store: {e}"))?;
    Ok(Arc::new(store))
}

async fn build_database_task_store(url: &str) -> Result<Arc<dyn TaskStore>, String> {
    let store = DatabaseTaskStore::connect(url)
        .await
        .map_err(|e| format!("failed to create database task store: {e}"))?;
    Ok(Arc::new(store))
}

fn expand_tilde(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy().to_string();
    if s.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let rest = s.trim_start_matches('~').trim_start_matches('/');
        PathBuf::from(format!("{}/{}", home, rest))
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSTGRES_TEST_URL_ENV: &str = "VOL_AGENT_POSTGRES_TEST_URL";
    const POSTGRES_TEST_URL_REQUIRED: &str =
        "VOL_AGENT_POSTGRES_TEST_URL must be set for mandatory Postgres task-store tests";

    fn postgres_test_url() -> String {
        std::env::var(POSTGRES_TEST_URL_ENV).expect(POSTGRES_TEST_URL_REQUIRED)
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

    #[tokio::test]
    async fn builder_accepts_database_task_store_config_until_provider_requirement() {
        let temp = tempfile::tempdir().unwrap();
        let providers_dir = temp.path().join(".agents/providers");
        std::fs::create_dir_all(&providers_dir).unwrap();
        std::fs::write(
            providers_dir.join("test.toml"),
            r#"
provider = "anthropic"
model = "claude-test"
api_key = "sk-test"
base_url = "https://api.test.com"
"#,
        )
        .unwrap();

        let db_url = format!("sqlite://{}", temp.path().join("tasks.db").display());
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some(db_url),
        };

        let runtime = AgentRuntime::builder(temp.path(), temp.path())
            .with_task_store_config(Some(config.clone()))
            .build()
            .await
            .expect("runtime should build with valid provider and database task store config");

        let mut task = vol_llm_task::Task::new(
            vol_llm_task::TaskKind::Manual,
            "runtime database task store test".to_string(),
            Vec::new(),
        );
        task.description = "created through AgentRuntime::task_store".to_string();
        let task_id = runtime
            .task_store
            .create(task)
            .await
            .expect("database task store should create tasks");
        drop(runtime);

        let runtime = AgentRuntime::builder(temp.path(), temp.path())
            .with_task_store_config(Some(config))
            .build()
            .await
            .expect("runtime should reconnect to configured database task store");
        let persisted = runtime
            .task_store
            .get(&task_id)
            .await
            .expect("database task store should get tasks")
            .expect("created task should persist across runtime rebuilds");

        assert_eq!(persisted.id, task_id);
        assert_eq!(persisted.subject, "runtime database task store test");
        assert_eq!(persisted.description, "created through AgentRuntime::task_store");
    }

    #[tokio::test]
    async fn builder_accepts_postgres_database_task_store_config() -> anyhow::Result<()> {
        let _guard = PostgresTestLock::acquire();
        let temp = tempfile::tempdir().unwrap();
        let providers_dir = temp.path().join(".agents/providers");
        std::fs::create_dir_all(&providers_dir).unwrap();
        std::fs::write(
            providers_dir.join("test.toml"),
            r#"
provider = "anthropic"
model = "claude-test"
api_key = "sk-test"
base_url = "https://api.test.com"
"#,
        )
        .unwrap();

        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some(postgres_test_url()),
        };
        let subject = format!(
            "runtime postgres database task store test {} {}",
            std::process::id(),
            uuid::Uuid::new_v4()
        );

        async fn cleanup_marker(store: &dyn vol_llm_task::TaskStore, subject: &str) -> anyhow::Result<()> {
            for task in store.list(None).await? {
                if task.subject == subject {
                    store.delete(&task.id).await?;
                }
            }
            Ok(())
        }

        let runtime = AgentRuntime::builder(temp.path(), temp.path())
            .with_task_store_config(Some(config.clone()))
            .build()
            .await
            .map_err(anyhow::Error::msg)?;
        let cleanup_store = runtime.task_store.clone();
        cleanup_marker(cleanup_store.as_ref(), &subject).await?;

        let mut task = vol_llm_task::Task::new(
            vol_llm_task::TaskKind::Manual,
            subject.clone(),
            Vec::new(),
        );
        task.description = "created through AgentRuntime::task_store using postgres".to_string();
        let task_id = runtime.task_store.create(task).await?;
        drop(runtime);

        let result = async {
            let runtime = AgentRuntime::builder(temp.path(), temp.path())
                .with_task_store_config(Some(config))
                .build()
                .await
                .map_err(anyhow::Error::msg)?;
            let persisted = runtime
                .task_store
                .get(&task_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("created task should persist across runtime rebuilds"))?;

            anyhow::ensure!(persisted.id == task_id, "persisted task id should match");
            anyhow::ensure!(persisted.subject == subject, "persisted task subject should match");
            Ok::<_, anyhow::Error>(())
        }
        .await;

        cleanup_marker(cleanup_store.as_ref(), &subject).await?;
        result
    }
}
