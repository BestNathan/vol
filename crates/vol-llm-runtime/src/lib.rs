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
use vol_llm_skill::SkillLoader;
use vol_llm_task::FileTaskStore;
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

        let mut config = AgentConfig::new(llm, self.tool_registry.clone(), session);
        config.def = Some(def.clone());
        config.working_dir = agent_dir;
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
    pub fn for_test() -> Self {
        let store_dir = PathBuf::from("/tmp/vol-llm-runtime-test");
        let working_dir = PathBuf::from(".");

        let llm_registry = ProviderLoader::load(Some(&working_dir));
        let llm_registry = if llm_registry.is_empty() { ProviderLoader::default() } else { llm_registry };
        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        let task_store: Arc<dyn TaskStore> = Arc::new(vol_llm_task::InMemoryTaskStore::new());
        vol_llm_task::tools::register_all(&mut tool_registry, task_store.clone());
        let tool_registry = Arc::new(tool_registry);
        let mcp_manager = Arc::new(McpManager::new(vec![]));
        let skill_loader = Arc::new(SkillLoader::new_empty());

        AgentRuntime {
            working_dir,
            store_dir,
            llm_registry,
            tool_registry,
            task_store,
            mcp_manager,
            skill_loader,
            agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
            agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }
}

// === Builder ===

pub struct AgentRuntimeBuilder {
    working_dir: PathBuf,
    store_dir: PathBuf,
}

impl AgentRuntimeBuilder {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        Self { working_dir, store_dir }
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
            let mgr = manager.clone();
            tokio::spawn(async move { let _ = mgr.connect().await; });
            Arc::new(manager)
        };

        let skill_loader = {
            let loader = Arc::new(SkillLoader::new(Some(self.working_dir.clone())));
            let ld = Arc::clone(&loader);
            tokio::spawn(async move { let _ = ld.discover_all().await; });
            loader
        };

        let tasks_dir = store_dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir)
            .map_err(|e| format!("failed to create tasks dir: {e}"))?;
        let task_store: Arc<dyn TaskStore> = Arc::new(
            FileTaskStore::new(&tasks_dir)
                .await
                .map_err(|e| format!("failed to create task store: {e}"))?
        );

        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        vol_llm_task::tools::register_all(&mut tool_registry, task_store.clone());
        let tool_config = vol_llm_tool::ToolConfig::default();
        vol_llm_tools_builtin::register_web_all(&mut tool_registry, &tool_config);
        let tool_registry = Arc::new(tool_registry);

        Ok(AgentRuntime {
            working_dir: self.working_dir,
            store_dir,
            llm_registry,
            tool_registry,
            task_store,
            mcp_manager,
            skill_loader,
            agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
            agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
        })
    }
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
