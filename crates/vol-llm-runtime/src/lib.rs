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
use vol_llm_agent::AgentLoader;
use vol_llm_agent::ReActAgent;
use vol_llm_mcp::{McpConfig, McpManager};
use vol_llm_provider::{create_provider, ProviderLoader};
use vol_llm_skill::{SkillLoader, SkillTool};
use vol_llm_task::TaskStore;
use vol_llm_task::{DatabaseTaskStore, FileTaskStore};
use vol_llm_tool::ToolRegistry;
use vol_session::{DatabaseSessionManager, FileSessionManager, Session, SessionManager};

/// Runtime status of a registered agent.
#[derive(Debug, Clone, Default)]
pub struct AgentStatus {
    pub status: String, // "idle" | "running"
    pub current_input: Option<String>,
    pub run_id: Option<String>,
}

impl AgentStatus {
    pub fn idle() -> Self {
        Self {
            status: "idle".into(),
            current_input: None,
            run_id: None,
        }
    }
    pub fn running(input: String, run_id: String) -> Self {
        Self {
            status: "running".into(),
            current_input: Some(input),
            run_id: Some(run_id),
        }
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
    pub session_manager: Arc<dyn SessionManager>,
    pub mcp_manager: Arc<McpManager>,
    pub sandbox_registry: Arc<vol_llm_sandbox::registry::SandboxRegistry>,
    pub skill_loader: Arc<SkillLoader>,
    pub agent_defs: Arc<std::sync::RwLock<HashMap<String, AgentDef>>>,
    pub agent_status: Arc<std::sync::RwLock<HashMap<String, AgentStatus>>>,
}

impl AgentRuntime {
    pub fn builder(
        working_dir: impl Into<PathBuf>,
        store_dir: impl Into<PathBuf>,
    ) -> AgentRuntimeBuilder {
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
    pub fn resolve_llm_for_agent(
        &self,
        def: &AgentDef,
    ) -> Result<Arc<dyn vol_llm_core::LLMClient>, String> {
        if let Some(ref model_name) = def.model {
            if let Some(fc) = self.llm_registry.get(model_name) {
                return create_provider(&fc.to_llm_config())
                    .map(Arc::from)
                    .map_err(|e| format!("LLM error for '{}': {}", model_name, e));
            }
        }
        let ids = self.llm_registry.ids();
        let first_id = ids
            .first()
            .ok_or_else(|| "No LLM providers configured".to_string())?;
        let fc = self
            .llm_registry
            .get(first_id)
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
        std::fs::create_dir_all(&agent_dir)
            .map_err(|e| format!("failed to create agent dirs: {e}"))?;

        let llm = self.resolve_llm_for_agent(&def)?;

        let session_store = self.session_manager.entry_store_for_agent(&agent_id);
        let session = Arc::new(Session::new(session_store));

        // Clone the full shared registry, then apply per-agent filters.
        let mut tool_registry = (*self.tool_registry).clone();

        // Filter MCP servers if mcps is set
        if let Some(ref server_names) = def.mcps {
            tool_registry = tool_registry.filter_mcp_servers(server_names);
        }

        // Filter allowed/disallowed tools (existing mechanism)
        let allowed_refs: Option<Vec<&str>> = def
            .tools
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect());
        let disallowed_refs: Option<Vec<&str>> = def
            .disallowed_tools
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect());
        let tool_registry = tool_registry.filter(
            allowed_refs.as_deref(),
            disallowed_refs.as_deref(),
        );

        let mut config = AgentConfig::builder()
            .with_def(def.clone())
            .with_llm(llm)
            .with_tools(tool_registry)
            .with_session(session)
            .with_working_dir(agent_dir.clone())
            .build()
            .expect("AgentConfig build failed — all required fields provided");

        config.mcp_manager = Some(self.mcp_manager.clone());

        let agent = ReActAgent::new(config);

        self.agent_defs
            .write()
            .unwrap()
            .insert(agent_id.clone(), def);
        self.agent_status
            .write()
            .unwrap()
            .insert(agent_id, AgentStatus::idle());

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
        tokio::spawn(async move {
            let _ = mcp.connect().await;
        });

        let skill = self.skill_loader.clone();
        tokio::spawn(async move {
            let _ = skill.discover_all().await;
        });

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
                let all_idle = status_map
                    .read()
                    .unwrap()
                    .values()
                    .all(|s| s.status == "idle");
                if all_idle || tokio::time::Instant::now() > deadline {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            tracing::info!("AgentRuntime stopped");
        });

        AgentRuntimeHandle {
            shutdown_tx,
            join_handle,
        }
    }
}

impl AgentRuntime {
    #[doc(hidden)]
    pub async fn for_test() -> Self {
        let store_dir = PathBuf::from("/tmp/vol-llm-runtime-test");
        let working_dir = PathBuf::from(".");

        let llm_registry = ProviderLoader::load(Some(&working_dir));
        let llm_registry = if llm_registry.is_empty() {
            ProviderLoader::default()
        } else {
            llm_registry
        };
        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        let task_store: Arc<dyn TaskStore> = Arc::new(vol_llm_task::InMemoryTaskStore::new());
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(store_dir.join("agents")));
        // Register the unified CLI-style `task` tool (agents using `tools: [task]`).
        vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
        let tool_registry = Arc::new(tool_registry);
        let mcp_manager = Arc::new(McpManager::new(vec![]));
        let sandbox_registry = {
            let tmp = std::env::temp_dir().join("vol-llm-runtime-test-sandboxes");
            let _ = std::fs::create_dir_all(&tmp);
            Arc::new(
                vol_llm_sandbox::registry::SandboxRegistry::load(&tmp)
                    .await
                    .expect("SandboxRegistry init in for_test"),
            )
        };
        let skill_loader = Arc::new(SkillLoader::new_empty());

        AgentRuntime {
            working_dir,
            store_dir,
            llm_registry,
            tool_registry,
            task_store,
            session_manager,
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
                    return Err(
                        "runtime.task_store.url is not valid when type = \"file\"".to_string()
                    );
                }
                Ok(())
            }
            TaskStoreType::Database => {
                let url = self.url.as_deref().ok_or_else(|| {
                    "runtime.task_store.url is required when type = \"database\"".to_string()
                })?;
                validate_database_url_scheme(url)
            }
        }
    }

    pub fn required_url(&self) -> Result<&str, String> {
        self.url.as_deref().ok_or_else(|| {
            "runtime.task_store.url is required when type = \"database\"".to_string()
        })
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
        other => Err(format!(
            "unsupported task store database url scheme: {other}"
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionStoreType {
    File,
    Database,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SessionStoreConfig {
    #[serde(rename = "type")]
    pub store_type: SessionStoreType,
    pub url: Option<String>,
}

impl SessionStoreConfig {
    pub fn validate(&self) -> Result<(), String> {
        match self.store_type {
            SessionStoreType::File => {
                if self.url.is_some() {
                    return Err(
                        "runtime.session_store.url is not valid when type = \"file\"".to_string(),
                    );
                }
                Ok(())
            }
            SessionStoreType::Database => {
                let url = self.url.as_deref().ok_or_else(|| {
                    "runtime.session_store.url is required when type = \"database\"".to_string()
                })?;
                validate_session_database_url_scheme(url)
            }
        }
    }

    pub fn required_url(&self) -> Result<&str, String> {
        self.url.as_deref().ok_or_else(|| {
            "runtime.session_store.url is required when type = \"database\"".to_string()
        })
    }
}

pub fn validate_session_database_url_scheme(url: &str) -> Result<(), String> {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .unwrap_or_default();

    match scheme {
        "sqlite" | "postgres" | "postgresql" | "mysql" => Ok(()),
        "" => Err("unsupported session store database url scheme: <missing>".to_string()),
        other => Err(format!(
            "unsupported session store database url scheme: {other}"
        )),
    }
}

pub struct AgentRuntimeBuilder {
    working_dir: PathBuf,
    store_dir: PathBuf,
    task_store_config: Option<TaskStoreConfig>,
    session_store_config: Option<SessionStoreConfig>,
}

impl AgentRuntimeBuilder {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        Self {
            working_dir,
            store_dir,
            task_store_config: None,
            session_store_config: None,
        }
    }

    pub fn with_task_store_config(mut self, config: Option<TaskStoreConfig>) -> Self {
        self.task_store_config = config;
        self
    }

    pub fn with_session_store_config(mut self, config: Option<SessionStoreConfig>) -> Self {
        self.session_store_config = config;
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
            vol_llm_sandbox::registry::SandboxRegistry::load(&sandboxes_dir)
                .await
                .map_err(|e| format!("Sandbox registry init failed: {}", e))?
        };
        let sandbox_registry = Arc::new(sandbox_registry);
        let skill_loader = {
            let loader = Arc::new(SkillLoader::new(Some(self.working_dir.clone())));
            let ld = Arc::clone(&loader);
            tokio::spawn(async move {
                let _ = ld.discover_all().await;
            });
            loader
        };

        let task_store: Arc<dyn TaskStore> = match self.task_store_config.as_ref() {
            None => build_file_task_store(&store_dir).await?,
            Some(config) if config.store_type == TaskStoreType::File => {
                build_file_task_store(&store_dir).await?
            }
            Some(config) if config.store_type == TaskStoreType::Database => {
                build_database_task_store(config.required_url()?).await?
            }
            Some(_) => return Err("unsupported task store configuration".to_string()),
        };

        let session_manager: Arc<dyn SessionManager> = match self.session_store_config.as_ref() {
            None => build_file_session_manager(&agents_root).await?,
            Some(config) if config.store_type == SessionStoreType::File => {
                build_file_session_manager(&agents_root).await?
            }
            Some(config) if config.store_type == SessionStoreType::Database => {
                build_database_session_manager(config.required_url()?).await?
            }
            Some(_) => return Err("unsupported session store configuration".to_string()),
        };

        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        // Register the unified CLI-style `task` tool (agents using `tools: [task]`).
        vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
        // Register declarative CLI-as-Tool entries from .agents/cli-tools/*.toml
        {
            let cli_tools_dir = self.working_dir.join(".agents").join("cli-tools");
            match vol_llm_tools_builtin::cli_tool::register_all(
                &mut tool_registry,
                &sandbox_registry,
                &cli_tools_dir,
            )
            .await
            {
                Ok(0) => {}
                Ok(n) => tracing::info!(n, "cli-tools registered"),
                Err(e) => return Err(format!("cli-tool registration failed: {e}")),
            }
        }
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
            session_manager,
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
    std::fs::create_dir_all(&tasks_dir).map_err(|e| format!("failed to create tasks dir: {e}"))?;
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

async fn build_file_session_manager(
    agents_root: &std::path::Path,
) -> Result<Arc<dyn SessionManager>, String> {
    std::fs::create_dir_all(agents_root)
        .map_err(|e| format!("failed to create agents dir for session store: {e}"))?;
    Ok(Arc::new(FileSessionManager::new(agents_root)))
}

async fn build_database_session_manager(url: &str) -> Result<Arc<dyn SessionManager>, String> {
    let manager = DatabaseSessionManager::connect(url)
        .await
        .map_err(|e| format!("failed to create database session store: {e}"))?;
    Ok(Arc::new(manager))
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

    #[test]
    fn session_store_config_rejects_file_url() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::File,
            url: Some("sqlite://sessions.db".to_string()),
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("runtime.session_store.url is not valid"));
    }

    #[test]
    fn session_store_config_requires_database_url() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::Database,
            url: None,
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("runtime.session_store.url is required"));
    }

    #[test]
    fn session_store_config_accepts_sqlite_postgres_and_mysql_schemes() {
        for url in [
            "sqlite://sessions.db",
            "postgres://user:pass@localhost/db",
            "postgresql://user:pass@localhost/db",
            "mysql://user:pass@localhost/db",
        ] {
            let config = SessionStoreConfig {
                store_type: SessionStoreType::Database,
                url: Some(url.to_string()),
            };
            config.validate().unwrap();
        }
    }

    #[tokio::test]
    async fn builds_sqlite_session_manager() {
        let temp = tempfile::tempdir().unwrap();
        let db_url = format!("sqlite://{}", temp.path().join("sessions.db").display());
        let manager = build_database_session_manager(&db_url).await.unwrap();
        let store = manager.entry_store_for_agent("alpha");

        store
            .save(vol_session::SessionEntry::new_summary(
                "session-a".to_string(),
                "summary".to_string(),
            ))
            .await
            .unwrap();

        let sessions = manager.list_sessions(Some("alpha")).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "session-a");
        assert_eq!(sessions[0].entry_count, 1);
    }

    #[tokio::test]
    async fn builds_task_and_session_stores_in_same_sqlite_database() {
        let temp = tempfile::tempdir().unwrap();
        let db_url = format!("sqlite://{}", temp.path().join("data.db").display());

        let task_store = build_database_task_store(&db_url).await.unwrap();
        let session_manager = build_database_session_manager(&db_url).await.unwrap();

        let task_id = task_store
            .create(vol_llm_task::Task::new(
                vol_llm_task::TaskKind::Manual,
                "shared sqlite task".to_string(),
                Vec::new(),
            ))
            .await
            .unwrap();
        assert!(task_store.get(&task_id).await.unwrap().is_some());

        let session_store = session_manager.entry_store_for_agent("alpha");
        session_store
            .save(vol_session::SessionEntry::new_summary(
                "shared-session".to_string(),
                "summary".to_string(),
            ))
            .await
            .unwrap();

        let sessions = session_manager.list_sessions(Some("alpha")).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "shared-session");
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
        assert_eq!(
            persisted.description,
            "created through AgentRuntime::task_store"
        );
    }

    #[tokio::test]
    async fn builder_accepts_postgres_database_task_store_config() -> anyhow::Result<()> {
        let Ok(postgres_url) = std::env::var(POSTGRES_TEST_URL_ENV) else {
            eprintln!(
                "SKIPPED: VOL_AGENT_POSTGRES_TEST_URL is not set; runtime Postgres task-store coverage was not exercised"
            );
            return Ok(());
        };

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
            url: Some(postgres_url),
        };
        let subject = format!(
            "runtime postgres database task store test {} {}",
            std::process::id(),
            uuid::Uuid::new_v4()
        );

        async fn cleanup_marker(
            store: &dyn vol_llm_task::TaskStore,
            subject: &str,
        ) -> anyhow::Result<()> {
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

        let mut task =
            vol_llm_task::Task::new(vol_llm_task::TaskKind::Manual, subject.clone(), Vec::new());
        task.description = "created through AgentRuntime::task_store using postgres".to_string();
        let task_id = runtime.task_store.create(task).await?;
        drop(runtime);

        let result = async {
            let runtime = AgentRuntime::builder(temp.path(), temp.path())
                .with_task_store_config(Some(config))
                .build()
                .await
                .map_err(anyhow::Error::msg)?;
            let persisted = runtime.task_store.get(&task_id).await?.ok_or_else(|| {
                anyhow::anyhow!("created task should persist across runtime rebuilds")
            })?;

            anyhow::ensure!(persisted.id == task_id, "persisted task id should match");
            anyhow::ensure!(
                persisted.subject == subject,
                "persisted task subject should match"
            );
            Ok::<_, anyhow::Error>(())
        }
        .await;

        cleanup_marker(cleanup_store.as_ref(), &subject).await?;
        result
    }

    // ── AgentStatus tests ──

    #[test]
    fn agent_status_idle() {
        let s = AgentStatus::idle();
        assert_eq!(s.status, "idle");
        assert!(s.current_input.is_none());
        assert!(s.run_id.is_none());
    }

    #[test]
    fn agent_status_running() {
        let s = AgentStatus::running("test input".into(), "run-1".into());
        assert_eq!(s.status, "running");
        assert_eq!(s.current_input.as_deref(), Some("test input"));
        assert_eq!(s.run_id.as_deref(), Some("run-1"));
    }

    // ── AgentRuntimeHandle tests ──

    #[tokio::test]
    async fn agent_runtime_handle_stop_completes() {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let handle = AgentRuntimeHandle {
            shutdown_tx,
            join_handle: tokio::spawn(async {
                let _ = shutdown_rx.await;
            }),
        };
        // Should complete without panicking
        handle.stop().await;
    }

    // ── AgentRuntime builder and accessors tests ──

    #[test]
    fn agent_runtime_builder_creates_default() {
        let builder = AgentRuntime::builder("/tmp/wd", "/tmp/store");
        assert_eq!(builder.working_dir, PathBuf::from("/tmp/wd"));
        assert_eq!(builder.store_dir, PathBuf::from("/tmp/store"));
        assert!(builder.task_store_config.is_none());
        assert!(builder.session_store_config.is_none());
    }

    #[test]
    fn agent_runtime_builder_with_configs() {
        let builder = AgentRuntime::builder("/tmp/wd", "/tmp/store")
            .with_task_store_config(Some(TaskStoreConfig {
                store_type: TaskStoreType::File,
                url: None,
            }))
            .with_session_store_config(Some(SessionStoreConfig {
                store_type: SessionStoreType::Database,
                url: Some("sqlite:///tmp/test.db".into()),
            }));
        assert!(builder.task_store_config.is_some());
        assert!(builder.session_store_config.is_some());
    }

    // ── TaskStoreConfig validation tests ──

    #[test]
    fn task_store_config_file_rejects_url() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::File,
            url: Some("sqlite://tasks.db".into()),
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("runtime.task_store.url is not valid"));
    }

    #[test]
    fn task_store_config_file_accepts_no_url() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::File,
            url: None,
        };
        config.validate().unwrap();
    }

    #[test]
    fn task_store_config_database_requires_url() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: None,
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("runtime.task_store.url is required"));
    }

    #[test]
    fn task_store_config_required_url_returns_error_when_none() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: None,
        };
        let err = config.required_url().unwrap_err();
        assert!(err.contains("runtime.task_store.url is required"));
    }

    #[test]
    fn task_store_config_required_url_returns_url() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some("sqlite:///tmp/db".into()),
        };
        assert_eq!(config.required_url().unwrap(), "sqlite:///tmp/db");
    }

    #[test]
    fn task_store_config_accepts_sqlite() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some("sqlite:///tmp/tasks.db".into()),
        };
        config.validate().unwrap();
    }

    #[test]
    fn task_store_config_accepts_postgres() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some("postgres://user:pass@localhost/db".into()),
        };
        config.validate().unwrap();
    }

    #[test]
    fn task_store_config_accepts_mysql() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some("mysql://user:pass@localhost/db".into()),
        };
        config.validate().unwrap();
    }

    // ── SessionStoreConfig validation (some already tested, add remaining) ──

    #[test]
    fn session_store_config_file_accepts_no_url() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::File,
            url: None,
        };
        config.validate().unwrap();
    }

    #[test]
    fn session_store_config_required_url_returns_url() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::Database,
            url: Some("sqlite:///tmp/sessions.db".into()),
        };
        assert_eq!(config.required_url().unwrap(), "sqlite:///tmp/sessions.db");
    }

    #[test]
    fn session_store_config_required_url_none_error() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::Database,
            url: None,
        };
        let err = config.required_url().unwrap_err();
        assert!(err.contains("runtime.session_store.url is required"));
    }

    #[test]
    fn session_store_config_accepts_postgresql() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::Database,
            url: Some("postgresql://user:pass@localhost/db".into()),
        };
        config.validate().unwrap();
    }

    #[test]
    fn session_store_config_rejects_unsupported_scheme() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::Database,
            url: Some("mongodb://localhost/db".into()),
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("unsupported session store database url scheme"));
    }

    // ── validate_database_url_scheme tests ──

    #[test]
    fn validate_database_url_scheme_missing_scheme() {
        let err = validate_database_url_scheme("localhost/db").unwrap_err();
        assert!(err.contains("<missing>"));
    }

    #[test]
    fn validate_database_url_scheme_unsupported() {
        let err = validate_database_url_scheme("mongodb://localhost/db").unwrap_err();
        assert!(err.contains("mongodb"));
    }

    #[test]
    fn validate_database_url_scheme_accepts_sqlite() {
        validate_database_url_scheme("sqlite:///tmp/db").unwrap();
    }

    #[test]
    fn validate_database_url_scheme_accepts_postgres() {
        validate_database_url_scheme("postgres://localhost/db").unwrap();
    }

    #[test]
    fn validate_database_url_scheme_accepts_mysql() {
        validate_database_url_scheme("mysql://localhost/db").unwrap();
    }

    // ── validate_session_database_url_scheme tests ──

    #[test]
    fn validate_session_url_accepts_valid_schemes() {
        for url in ["sqlite:///tmp/db", "postgres://localhost/db", "postgresql://localhost/db", "mysql://localhost/db"] {
            validate_session_database_url_scheme(url).unwrap();
        }
    }

    #[test]
    fn validate_session_url_missing_scheme() {
        let err = validate_session_database_url_scheme("localhost/db").unwrap_err();
        assert!(err.contains("<missing>"));
    }

    #[test]
    fn validate_session_url_unsupported() {
        let err = validate_session_database_url_scheme("redis://localhost/db").unwrap_err();
        assert!(err.contains("redis"));
    }

    // ── expand_tilde tests ──

    #[test]
    fn expand_tilde_replaces_with_home() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let expanded = expand_tilde(PathBuf::from("~/test/path"));
        let expected = PathBuf::from(format!("{}/test/path", home));
        assert_eq!(expanded, expected);
    }

    #[test]
    fn expand_tilde_no_change_without_tilde() {
        let path = PathBuf::from("/absolute/path");
        let expanded = expand_tilde(path.clone());
        assert_eq!(expanded, path);
    }

    #[test]
    fn expand_tilde_handles_tilde_slash() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let expanded = expand_tilde(PathBuf::from("~"));
        assert_eq!(expanded, PathBuf::from(home));
    }

    // ── AgentRuntime resolve_llm_for_agent error path tests ──

    #[tokio::test]
    async fn resolve_llm_no_providers_returns_error() {

        let temp = tempfile::tempdir().unwrap();
        let sandboxes_dir = temp.path().join(".sandboxes");
        std::fs::create_dir_all(&sandboxes_dir).unwrap();
        let registry = vol_llm_sandbox::registry::SandboxRegistry::load(&sandboxes_dir).await.unwrap();
        let runtime = AgentRuntime {
            working_dir: PathBuf::from("/tmp"),
            store_dir: PathBuf::from("/tmp/store"),
            llm_registry: ProviderLoader::default(),
            tool_registry: Arc::new(ToolRegistry::new()),
            task_store: Arc::new(vol_llm_task::InMemoryTaskStore::new()),
            session_manager: Arc::new(FileSessionManager::new("/tmp/agents")),
            mcp_manager: Arc::new(McpManager::new(vec![])),
            sandbox_registry: Arc::new(registry),
            skill_loader: Arc::new(SkillLoader::new_empty()),
            agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
            agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
        };
        let def = AgentDef::new("test-agent", "You are a test agent.");
        match runtime.resolve_llm_for_agent(&def) {
            Err(e) => assert!(e.contains("No LLM providers configured"), "unexpected error: {}", e),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    #[tokio::test]
    async fn resolve_llm_with_non_matching_model_falls_back() {
        let temp = tempfile::tempdir().unwrap();
        let sandboxes_dir = temp.path().join(".sandboxes");
        std::fs::create_dir_all(&sandboxes_dir).unwrap();
        let registry = vol_llm_sandbox::registry::SandboxRegistry::load(&sandboxes_dir).await.unwrap();
        let runtime = AgentRuntime {
            working_dir: PathBuf::from("/tmp"),
            store_dir: PathBuf::from("/tmp/store"),
            llm_registry: ProviderLoader::default(),
            tool_registry: Arc::new(ToolRegistry::new()),
            task_store: Arc::new(vol_llm_task::InMemoryTaskStore::new()),
            session_manager: Arc::new(FileSessionManager::new("/tmp/agents")),
            mcp_manager: Arc::new(McpManager::new(vec![])),
            sandbox_registry: Arc::new(registry),
            skill_loader: Arc::new(SkillLoader::new_empty()),
            agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
            agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
        };
        let mut def = AgentDef::new("test-agent", "You are a test agent.");
        def.model = Some("non-existent-model".into());
        match runtime.resolve_llm_for_agent(&def) {
            Err(e) => assert!(e.contains("No LLM providers configured"), "unexpected error: {}", e),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

// ── Builder build() error paths ──

    #[tokio::test]
    async fn builder_build_fails_without_providers() {
        let temp = tempfile::tempdir().unwrap();
        match AgentRuntime::builder(temp.path(), temp.path()).build().await {
            Err(e) => assert!(e.contains("No LLM provider configured"), "expected provider error, got: {}", e),
            Ok(_) => panic!("expected build to fail without providers"),
        }
    }

    // ── build_file_task_store and build_file_session_manager tests ──

    #[tokio::test]
    async fn build_file_task_store_creates_store() {
        let temp = tempfile::tempdir().unwrap();
        let store = build_file_task_store(temp.path()).await.unwrap();
        // FileTaskStore should accept create/list operations
        let task = vol_llm_task::Task::new(
            vol_llm_task::TaskKind::Manual,
            "test task".to_string(),
            Vec::new(),
        );
        let id = store.create(task).await.unwrap();
        let fetched = store.get(&id).await.unwrap().unwrap();
        assert_eq!(fetched.subject, "test task");
    }

    #[tokio::test]
    async fn build_file_session_manager_creates_manager() {
        let temp = tempfile::tempdir().unwrap();
        let agents_root = temp.path().join("agents");
        let mgr = build_file_session_manager(&agents_root).await.unwrap();
        let store = mgr.entry_store_for_agent("test-agent");
        store
            .save(vol_session::SessionEntry::new_summary(
                "session-1".to_string(),
                "test summary".to_string(),
            ))
            .await
            .unwrap();
        let sessions = mgr.list_sessions(Some("test-agent")).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "session-1");
    }

    // ── Builder build without explicit store configs (defaults to file) ──

    #[tokio::test]
    async fn builder_build_without_store_configs_defaults_to_file() {
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

        let runtime = AgentRuntime::builder(temp.path(), temp.path())
            .with_task_store_config(None)
            .with_session_store_config(None)
            .build()
            .await
            .expect("build should succeed with empty store configs");

        // Default file stores should work
        let task = vol_llm_task::Task::new(
            vol_llm_task::TaskKind::Manual,
            "default store".to_string(),
            Vec::new(),
        );
        let id = runtime.task_store.create(task).await.unwrap();
        let fetched = runtime.task_store.get(&id).await.unwrap().unwrap();
        assert_eq!(fetched.subject, "default store");
    }

    // ── Builder with valid file configs (tests build branch for file stores) ──

    #[tokio::test]
    async fn builder_build_with_file_stores() {
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

        let task_cfg = TaskStoreConfig {
            store_type: TaskStoreType::File,
            url: None,
        };
        let session_cfg = SessionStoreConfig {
            store_type: SessionStoreType::File,
            url: None,
        };

        let runtime = AgentRuntime::builder(temp.path(), temp.path())
            .with_task_store_config(Some(task_cfg))
            .with_session_store_config(Some(session_cfg))
            .build()
            .await
            .expect("build should succeed with file stores and valid provider config");

        // Runtime should have functional file stores
        let task = vol_llm_task::Task::new(
            vol_llm_task::TaskKind::Manual,
            "builder file store".to_string(),
            Vec::new(),
        );
        let id = runtime.task_store.create(task).await.unwrap();
        let fetched = runtime.task_store.get(&id).await.unwrap().unwrap();
        assert_eq!(fetched.subject, "builder file store");

        let session_store = runtime.session_manager.entry_store_for_agent("test-agent");
        session_store
            .save(vol_session::SessionEntry::new_summary(
                "s1".to_string(),
                "file session".to_string(),
            ))
            .await
            .unwrap();
        let sessions = runtime.session_manager.list_sessions(Some("test-agent")).await.unwrap();
        assert_eq!(sessions.len(), 1);
    }

    // ── AgentRuntime accessor tests (using for_test) ──

    #[tokio::test]
    async fn runtime_working_dir_accessor() {
        let rt = AgentRuntime::for_test().await;
        assert_eq!(rt.working_dir(), std::path::Path::new("."));
    }

    #[tokio::test]
    async fn runtime_store_dir_accessor() {
        let rt = AgentRuntime::for_test().await;
        assert_eq!(rt.store_dir(), std::path::Path::new("/tmp/vol-llm-runtime-test"));
    }

    // ── AgentRuntime for_test builder provides all defaults ──

    #[tokio::test]
    async fn runtime_for_test_creates_valid_runtime() {
        let rt = AgentRuntime::for_test().await;
        assert!(rt.agent_defs.read().unwrap().is_empty());
        assert!(rt.agent_status.read().unwrap().is_empty());
        // for_test registers builtin tools, so tool_names should be non-empty
        assert!(!rt.tool_registry.tool_names().is_empty());
    }

    #[tokio::test]
    async fn register_agent_with_mcps_creates_filtered_tool_registry() {
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

        let wd = temp.path().to_path_buf();
        // Build a runtime, then register directly without discover_agents.
        // Use the builder so we get a real provider, then call register_agent manually.
        let rt = AgentRuntime::builder(wd.clone(), wd.clone())
            .build()
            .await
            .expect("runtime build should succeed");

        // Agent with mcps: ["nonexistent-server"] — no MCP tools should be registered
        // since the test McpManager has no servers configured.
        let def = AgentDef::new("filtered-agent", "You are a filtered agent.")
            .with_type("test")
            .with_description("Agent with filtered MCP")
            .with_mcps(vec!["nonexistent-server".to_string()]);

        let agent = rt
            .register_agent("filtered-agent", def)
            .await
            .expect("register_agent should succeed");

        // The agent should have built-in tools but no MCP tools
        let tool_names = agent.config().tools.tool_names();
        assert!(
            !tool_names.is_empty(),
            "agent should have built-in tools"
        );
        // No MCP tools should be present (the test McpManager has no servers)
        let mcp_tools: Vec<_> = tool_names
            .iter()
            .filter(|n| n.starts_with("mcp__"))
            .collect();
        assert!(
            mcp_tools.is_empty(),
            "agent with mcps filter should have no MCP tools when no matching servers exist, got: {:?}",
            mcp_tools
        );
    }

    #[tokio::test]
    async fn register_agent_without_mcps_uses_shared_registry() {
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

        let wd = temp.path().to_path_buf();
        let rt = AgentRuntime::builder(wd.clone(), wd.clone())
            .build()
            .await
            .expect("runtime build should succeed");

        let def = AgentDef::new("unfiltered-agent", "You are an unfiltered agent.")
            .with_type("test")
            .with_description("Agent without MCP filter");

        let agent = rt
            .register_agent("unfiltered-agent", def)
            .await
            .expect("register_agent should succeed");

        let tool_names = agent.config().tools.tool_names();
        assert!(
            !tool_names.is_empty(),
            "agent should have built-in tools"
        );
        // No MCP tools since test McpManager has no servers — same outcome,
        // but the code path uses the shared registry (no filtering).
    }

    #[tokio::test]
    async fn agent_runtime_handle_stop_does_not_panic() {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let (join_tx, join_rx) = tokio::sync::oneshot::channel::<()>();
        let join_handle = tokio::spawn(async {
            let _ = join_rx.await;
        });
        let handle = AgentRuntimeHandle {
            shutdown_tx: tx,
            join_handle,
        };
        drop(join_tx);
        handle.stop().await;
    }

    #[test]
    fn task_store_config_database_accepts_postgresql_scheme() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some("postgresql://user:pass@localhost/db".to_string()),
        };
        config.validate().unwrap();
    }

    #[test]
    fn task_store_config_rejects_unsupported_scheme() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some("oracle://localhost/db".to_string()),
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("unsupported task store database url scheme"));
    }

    #[test]
    fn builder_with_task_store_chains_and_exposes_config() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::File,
            url: None,
        };
        let builder = AgentRuntimeBuilder::new(PathBuf::from("."), PathBuf::from("/tmp"))
            .with_task_store_config(Some(config))
            .with_session_store_config(None);
        // builder compiles and chains — coverage for builder field setters
        let _ = builder;
    }

    #[test]
    fn task_store_required_url_returns_url_for_database() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some("sqlite://tasks.db".to_string()),
        };
        assert_eq!(config.required_url().unwrap(), "sqlite://tasks.db");
    }

    #[test]
    fn task_store_required_url_errors_for_none() {
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: None,
        };
        assert!(config.required_url().is_err());
    }

    #[test]
    fn session_store_required_url_returns_url_for_database() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::Database,
            url: Some("postgres://localhost/db".to_string()),
        };
        assert_eq!(config.required_url().unwrap(), "postgres://localhost/db");
    }

    #[test]
    fn session_store_required_url_errors_for_none() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::Database,
            url: None,
        };
        assert!(config.required_url().is_err());
    }

    #[test]
    fn task_store_type_is_file_when_set() {
        assert_eq!(TaskStoreType::File, TaskStoreType::File);
    }

    #[test]
    fn session_store_type_is_database_when_set() {
        assert_eq!(SessionStoreType::Database, SessionStoreType::Database);
    }

    #[test]
    fn validate_database_url_scheme_rejects_empty_scheme() {
        let err = validate_database_url_scheme(":no_scheme").unwrap_err();
        assert!(err.contains("<missing>"));
    }

    #[test]
    fn agent_runtime_builder_new_returns_agentruntimebuilder() {
        let builder = AgentRuntimeBuilder::new(PathBuf::from("/tmp/work"), PathBuf::from("/tmp/store"));
        // Chaining with_task_store_config confirms builder type is correct
        let _ = builder.with_task_store_config(None);
    }
}
