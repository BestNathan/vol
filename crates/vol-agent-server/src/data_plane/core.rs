//! Unified core for the agent server.
//!
//! `DataPlaneServerCore` is the single source of truth for shared resources.
//!
//! store_dir 内部按 agent 分层，所有资源归 agent 所有：
//! ```text
//! {store_dir}/
//!   agents/
//!     {agent_id}/
//!       sessions/      — 该 agent 的会话持久化
//!       ...            — 日志、缓存等
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent::react::AgentConfig;
use vol_llm_core::LLMClient;
use vol_llm_mcp::McpManager;
use vol_llm_provider::{create_provider, ProviderLoader};
use vol_llm_runtime::{AgentRuntime, SessionStoreConfig, TaskStoreConfig};
use vol_llm_skill::SkillLoader;
use vol_llm_tool::ToolRegistry;

use crate::data_plane::connection_holder::ConnectionHolder;
use crate::data_plane::dispatcher::AgentDispatcher;
use crate::data_plane::handlers::{
    agent::AgentHandler, control::DataPlaneControlHandler, file::FileHandler, log::LogHandler,
    mcp::McpHandler, sandbox::SandboxHandler, session::SessionHandler, skill::SkillHandler,
    system::SystemHandler, task::TaskHandler, tool::ToolHandler,
};
use crate::data_plane::router::AgentRouter;
use vol_llm_agent_protocol::Connection;
use vol_llm_agent_protocol::HandlerRegistry;

/// Derived sub-paths within store_dir.
///
/// ```text
/// {store_dir}/
///   sessions/          — 会话持久化文件
///   agents/
///     {agent_id}/      — agent 私有目录（日志、缓存等）
/// ```
pub struct StorePaths {
    pub root: PathBuf,
    pub sessions: PathBuf,
    pub agents_root: PathBuf,
}

impl StorePaths {
    /// Get the private directory for a specific agent.
    pub fn agent_dir(&self, agent_id: &str) -> PathBuf {
        self.agents_root.join(agent_id)
    }

    /// Ensure all directories exist.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.sessions)?;
        std::fs::create_dir_all(&self.agents_root)?;
        Ok(())
    }
}

pub use vol_llm_runtime::AgentStatus;

/// Shared core for the agent server.
///
/// The core owns all shared resources (paths, registries, agent runtime).
/// Domain handlers hold the specific resources they need (no self-reference).
///
/// Given only `working_dir` and `store_dir`, all internal registries are
/// derived automatically (LLM, MCP, skills, agents, tools, sessions).
pub struct DataPlaneServerCore {
    // === AgentRuntime ===
    pub runtime: AgentRuntime,

    // === Paths (derived from runtime) ===
    working_dir: PathBuf,
    store_dir: PathBuf,

    // === Registries (from runtime) ===
    mcp_manager: Arc<McpManager>,
    skill_loader: Arc<SkillLoader>,
    tool_registry: Arc<ToolRegistry>,

    // === Agent runtime ===
    llm: Arc<dyn LLMClient>,
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,

    // === Sandbox registry (from runtime) ===
    sandbox_registry: Arc<vol_llm_sandbox::registry::SandboxRegistry>,

    // === Agent definitions (from runtime) ===
    agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_core::AgentDef>>>,

    // === Agent status (from runtime) ===
    agent_status: Arc<std::sync::RwLock<HashMap<String, AgentStatus>>>,

    // === Domain handlers ===
    handler_registry: HandlerRegistry,
}

impl DataPlaneServerCore {
    /// Create a new core from paths only.
    ///
    /// All internal registries (LLM, MCP, skills, tools, sessions) are
    /// derived automatically from working_dir and store_dir.
    pub async fn new(
        working_dir: impl Into<PathBuf>,
        store_dir: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        Self::builder(working_dir, store_dir).build().await
    }

    /// Create a builder for optional override.
    pub fn builder(
        working_dir: impl Into<PathBuf>,
        store_dir: impl Into<PathBuf>,
    ) -> DataPlaneServerCoreBuilder {
        DataPlaneServerCoreBuilder::new(working_dir.into(), store_dir.into())
    }

    // === Path accessors ===

    pub fn working_dir(&self) -> &std::path::Path {
        &self.working_dir
    }

    pub fn store_dir(&self) -> &std::path::Path {
        &self.store_dir
    }

    // === Registry accessors ===

    pub fn mcp_manager(&self) -> &Arc<McpManager> {
        &self.mcp_manager
    }

    pub fn skill_loader(&self) -> &Arc<SkillLoader> {
        &self.skill_loader
    }

    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    // === Agent runtime accessors ===

    pub fn llm(&self) -> &Arc<dyn LLMClient> {
        &self.llm
    }

    pub fn router(&self) -> &AgentRouter {
        &self.router
    }

    pub fn holders(&self) -> &Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>> {
        &self.holders
    }

    pub fn agent_defs(&self) -> &Arc<std::sync::RwLock<HashMap<String, vol_llm_core::AgentDef>>> {
        &self.agent_defs
    }

    pub fn agent_status(&self) -> &Arc<std::sync::RwLock<HashMap<String, AgentStatus>>> {
        &self.agent_status
    }

    /// Register a new agent with the given id and definition.
    ///
    /// Agent 所有资源归 `{store_dir}/agents/{agent_id}/` 下，不污染用户工作区。
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    pub async fn register_agent(
        &self,
        agent_id: impl Into<String>,
        def: vol_llm_core::agent_def::AgentDef,
    ) -> Result<(), String> {
        let agent_id = agent_id.into();
        let agent_dir = self.store_dir.join("agents").join(&agent_id);
        std::fs::create_dir_all(&agent_dir)
            .map_err(|e| format!("failed to create agent dirs: {e}"))?;

        let llm = self.llm.clone();

        // Clone the full shared registry and apply per-agent filters (mcps, tools, disallowed_tools).
        let mut tool_registry = (*self.tool_registry).clone();
        if let Some(ref server_names) = def.mcps {
            tool_registry = tool_registry.filter_mcp_servers(server_names);
        }
        let allowed_refs: Option<Vec<&str>> = def
            .tools
            .as_ref()
            .map(|v| v.iter().map(std::string::String::as_str).collect());
        let disallowed_refs: Option<Vec<&str>> = def
            .disallowed_tools
            .as_ref()
            .map(|v| v.iter().map(std::string::String::as_str).collect());
        let tools = tool_registry.filter(allowed_refs.as_deref(), disallowed_refs.as_deref());

        let mcp = self.mcp_manager.clone();

        let session_store = self
            .runtime
            .session_manager
            .entry_store_for_agent(&agent_id);
        let session = Arc::new(vol_session::Session::new(session_store));

        let mut config = AgentConfig::builder()
            .with_def(def.clone())
            .with_llm(llm)
            .with_tools(tools)
            .with_session(session)
            .with_sandbox_registry(self.sandbox_registry.clone())
            .with_working_dir(agent_dir.clone())
            .build()
            .expect("AgentConfig build failed — LLM, tools, and session are all provided");

        config.mcp_manager = Some(mcp);

        let holder = ConnectionHolder::new(
            agent_id.clone(),
            "client".to_string(),
            Some(self.agent_status.clone()),
        );
        config.plugin_registry.register(holder.clone());

        // Register observability plugins
        config
            .plugin_registry
            .register(vol_llm_observability::MetricsPlugin::new());
        config
            .plugin_registry
            .register(vol_llm_observability::LokiPlugin::new());

        let agent = vol_llm_agent::ReActAgent::new(config);
        let dispatcher = Arc::new(AgentDispatcher::new(agent));

        self.router.register(agent_id.clone(), dispatcher).await;
        let count = {
            let mut holders = self.holders.lock().unwrap();
            holders.insert(agent_id.clone(), Arc::new(holder));
            holders.len()
        };

        tracing::info!(name = %agent_id, holders_count = count, "Agent registered");

        Ok(())
    }

    /// List all registered agent IDs.
    #[allow(clippy::unwrap_used)]
    pub async fn list_agent_ids(&self) -> Vec<String> {
        self.holders.lock().unwrap().keys().cloned().collect()
    }

    /// Discover and register all agents from .agents/agents/ directories.
    #[allow(clippy::unwrap_used)]
    pub async fn discover_agents(&self) -> Result<(), String> {
        let loader = vol_llm_agent::AgentLoader::new(Some(self.working_dir.clone()));
        loader.discover_all().await.map_err(|e| e.to_string())?;

        let agents = loader.list_metadata().await;
        tracing::info!(count = agents.len(), "Discovered agents from disk");
        for meta in agents {
            if let Some(def) = loader.get(&meta.name).await {
                tracing::info!(name = %meta.name, r#type = %meta.r#type, "Registering agent");
                // Store def for metadata queries
                self.agent_defs
                    .write()
                    .unwrap()
                    .insert(meta.name.clone(), (*def).clone());
                let arc_def = Arc::try_unwrap(def).unwrap_or_else(|arc| (*arc).clone());
                self.register_agent(&meta.name, arc_def).await?;
                self.agent_status
                    .write()
                    .unwrap()
                    .insert(meta.name.clone(), AgentStatus::idle());
            }
        }
        Ok(())
    }

    /// Handle an inbound `AgentServerMessage` by dispatching via the handler registry.
    pub async fn handle(
        &self,
        message: vol_llm_agent_protocol::agent_server_protocol::AgentServerMessage,
    ) -> Result<
        Vec<vol_llm_agent_protocol::agent_server_protocol::AgentServerMessage>,
        vol_llm_agent_protocol::agent_server_protocol::ProtocolError,
    > {
        self.handler_registry.dispatch(message).await
    }

    /// Serve incoming messages from a connection, dispatching each to the handler registry.
    ///
    /// Attaches the connection to all holders so agent events flow through.
    /// Loops `recv() → handle() → send()` until the connection closes or errors.
    pub async fn serve(&self, conn: impl Connection) {
        self.serve_dyn(Arc::new(conn)).await;
    }

    /// Serve incoming messages from a type-erased connection.
    #[allow(clippy::unwrap_used)]
    pub async fn serve_dyn(&self, conn: Arc<dyn Connection>) {
        tracing::info!(dir = "dp < client", "data-plane accepted client connection");

        // Attach to all holders so agent events are pushed to this connection.
        let holders: Vec<_> = { self.holders.lock().unwrap().values().cloned().collect() };
        for holder in &holders {
            holder.attach(conn.clone()).await;
        }

        while let Some(result) = conn.recv().await {
            let responses = match result {
                Ok(msg) => match self.handle(msg).await {
                    Ok(resp) => resp,
                    Err(e) => vec![vol_llm_agent_protocol::agent_server_protocol::AgentServerMessage::new_error(
                        uuid::Uuid::new_v4().to_string(),
                        vol_llm_agent_protocol::agent_server_protocol::Operation::System(
                            vol_llm_agent_protocol::agent_server_protocol::SystemOperation::Connected,
                        ),
                        vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                            code: "dispatch_error".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: false,
                        },
                    )],
                },
                Err(e) => {
                    tracing::debug!(%e, "connection receive ended");
                    break;
                }
            };
            for resp in responses {
                if let Err(e) = conn.send(resp).await {
                    tracing::debug!(%e, "connection send ended");
                    tracing::info!(
                        dir = "dp < client",
                        "data-plane client connection closed (send error)"
                    );
                    return;
                }
            }
        }
        tracing::info!(dir = "dp < client", "data-plane client connection closed");
    }
}

#[async_trait]
impl vol_llm_agent_protocol::JsonRpcMessageService for DataPlaneServerCore {
    async fn serve_connection(&self, conn: Arc<dyn Connection>) {
        self.serve_dyn(conn).await;
    }
}

/// Builder for [`DataPlaneServerCore`].
///
/// Only `working_dir` and `store_dir` are required.
/// All other resources (LLM, MCP, skills, tools, sessions) are derived automatically.
pub struct DataPlaneServerCoreBuilder {
    working_dir: PathBuf,
    store_dir: PathBuf,
    task_store_config: Option<TaskStoreConfig>,
    session_store_config: Option<SessionStoreConfig>,
    extra_handlers: Vec<Arc<dyn vol_llm_agent_protocol::DomainHandler>>,
}

impl Default for DataPlaneServerCoreBuilder {
    fn default() -> Self {
        Self {
            working_dir: PathBuf::new(),
            store_dir: PathBuf::new(),
            task_store_config: None,
            session_store_config: None,
            extra_handlers: Vec::new(),
        }
    }
}

impl DataPlaneServerCoreBuilder {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        Self {
            working_dir,
            store_dir,
            task_store_config: None,
            session_store_config: None,
            extra_handlers: Vec::new(),
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

    /// Register an external domain handler.
    pub fn register_handler(
        mut self,
        handler: Arc<dyn vol_llm_agent_protocol::DomainHandler>,
    ) -> Self {
        self.extra_handlers.push(handler);
        self
    }

    /// Build the core. Creates AgentRuntime internally, then wraps it with transport layer.
    pub async fn build(self) -> Result<DataPlaneServerCore, String> {
        // Build AgentRuntime (owns all shared resources)
        let runtime = AgentRuntime::builder(self.working_dir.clone(), self.store_dir.clone())
            .with_task_store_config(self.task_store_config.clone())
            .with_session_store_config(self.session_store_config.clone())
            .build()
            .await?;

        // Extract all resources from runtime first
        let store_dir = runtime.store_dir().to_path_buf();
        let mcp_manager = runtime.mcp_manager.clone();
        let skill_loader = runtime.skill_loader.clone();
        let agent_defs = runtime.agent_defs.clone();
        let agent_status = runtime.agent_status.clone();
        let session_manager = runtime.session_manager.clone();
        let sandbox_registry = runtime.sandbox_registry.clone();

        // Tool registry already includes SkillTool from AgentRuntime
        let tool_registry = runtime.tool_registry.clone();

        // Derive LLM — try each configured provider, skip ones that fail auth.
        // This avoids crashing when a provider's env var isn't set (e.g. OPENAI_API_KEY)
        // as long as at least one provider resolves successfully.
        let llm = derive_llm_client(&self.working_dir)?;

        let router = AgentRouter::new();
        let holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));

        let mut handler_registry = HandlerRegistry::new();
        handler_registry
            .register(Arc::new(AgentHandler::new(
                router.clone(),
                Arc::clone(&holders),
                agent_defs.clone(),
                agent_status.clone(),
            )))
            .map_err(|e| format!("failed to register AgentHandler: {e}"))?;
        handler_registry
            .register(Arc::new(FileHandler::new(self.working_dir.clone())))
            .map_err(|e| format!("failed to register FileHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SessionHandler::new(
                session_manager,
                router.clone(),
            )))
            .map_err(|e| format!("failed to register SessionHandler: {e}"))?;
        handler_registry
            .register(Arc::new(McpHandler::new(Some(mcp_manager.clone()))))
            .map_err(|e| format!("failed to register McpHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SkillHandler::new(Some(skill_loader.clone()))))
            .map_err(|e| format!("failed to register SkillHandler: {e}"))?;
        handler_registry
            .register(Arc::new(ToolHandler::new(tool_registry.clone())))
            .map_err(|e| format!("failed to register ToolHandler: {e}"))?;
        handler_registry
            .register(Arc::new(LogHandler))
            .map_err(|e| format!("failed to register LogHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SystemHandler))
            .map_err(|e| format!("failed to register SystemHandler: {e}"))?;

        handler_registry
            .register(Arc::new(TaskHandler::new(runtime.task_store.clone())))
            .map_err(|e| format!("failed to register TaskHandler: {e}"))?;
        handler_registry
            .register(Arc::new(DataPlaneControlHandler::new()))
            .map_err(|e| format!("failed to register DataPlaneControlHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SandboxHandler::new(sandbox_registry.default())))
            .map_err(|e| format!("failed to register SandboxHandler: {e}"))?;

        for extra in self.extra_handlers {
            handler_registry
                .register(extra)
                .map_err(|e| format!("failed to register external handler: {e}"))?;
        }

        Ok(DataPlaneServerCore {
            runtime,
            working_dir: self.working_dir,
            store_dir,
            mcp_manager,
            skill_loader,
            tool_registry,
            sandbox_registry,
            llm,
            router,
            holders,
            agent_defs,
            agent_status,
            handler_registry,
        })
    }
}

#[allow(clippy::unwrap_used)]
fn derive_llm_client(working_dir: &std::path::Path) -> Result<Arc<dyn LLMClient>, String> {
    let loader = ProviderLoader::load(Some(working_dir));
    if loader.is_empty() {
        return Err("No LLM provider configured in .agents/providers/*.toml".to_string());
    }
    // Try each provider, skip ones that fail (e.g., missing env var).
    let mut errors = Vec::new();
    for id in loader.ids() {
        let file_config = loader.get(id).unwrap();
        let llm_config = file_config.to_llm_config();
        match create_provider(&llm_config) {
            Ok(client) => return Ok(Arc::from(client)),
            Err(e) => errors.push(format!("{id}: {e}")),
        }
    }
    Err(format!(
        "No usable LLM provider found. Errors: {}",
        errors.join("; ")
    ))
}

/// Test constructor that provides minimal defaults for all optional fields.
impl DataPlaneServerCore {
    #[doc(hidden)]
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    pub async fn for_test() -> Self {
        use std::sync::Arc;

        let store_dir = PathBuf::from("/tmp/vol-llm-agent-channel-test-sessions");
        let agents_root = store_dir.join("agents");
        std::fs::create_dir_all(&agents_root).ok();

        let router = AgentRouter::new();
        let holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));

        struct TestLlm;
        #[async_trait::async_trait]
        impl LLMClient for TestLlm {
            fn provider(&self) -> vol_llm_core::LLMProvider {
                vol_llm_core::LLMProvider::Anthropic
            }
            fn model(&self) -> &str {
                "test"
            }
            fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
                &[]
            }
            async fn converse(
                &self,
                _request: vol_llm_core::ConversationRequest,
            ) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> {
                Err(vol_llm_core::LLMError::Parse(
                    "test LLM not implemented".into(),
                ))
            }
            async fn converse_stream(
                &self,
                _request: vol_llm_core::ConversationRequest,
            ) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> {
                let (_tx, rx) = tokio::sync::mpsc::channel(1);
                Ok(vol_llm_core::StreamReceiver::new(rx))
            }
        }

        // Register a test agent dispatcher so submit flow works.
        {
            use crate::data_plane::dispatcher::AgentDispatcher;
            use vol_llm_agent::react::AgentConfig;
            use vol_llm_agent::ReActAgent;
            use vol_session::InMemoryEntryStore;
            use vol_session::Session;

            let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
            let tools: Arc<vol_llm_tool::ToolRegistry> =
                Arc::new(vol_llm_tool::ToolRegistry::new());
            let config = AgentConfig::builder()
                .with_llm(Arc::new(TestLlm))
                .with_tools(tools)
                .with_session(session)
                .build()
                .expect("AgentConfig build failed for test");
            let agent = ReActAgent::new(config);
            let dispatcher = Arc::new(AgentDispatcher::new(agent));
            let holder = Arc::new(ConnectionHolder::new(
                "test_agent".to_string(),
                "client".to_string(),
                None,
            ));
            router.register("test_agent".to_string(), dispatcher).await;
            holders
                .lock()
                .unwrap()
                .insert("test_agent".to_string(), holder);
        }

        let agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_core::AgentDef>>> =
            Arc::new(std::sync::RwLock::new(HashMap::new()));
        let agent_status: Arc<std::sync::RwLock<HashMap<String, AgentStatus>>> =
            Arc::new(std::sync::RwLock::new(HashMap::new()));
        let mut handler_registry = HandlerRegistry::new();
        handler_registry
            .register(Arc::new(AgentHandler::new(
                router.clone(),
                Arc::clone(&holders),
                agent_defs.clone(),
                agent_status.clone(),
            )))
            .ok();
        handler_registry
            .register(Arc::new(FileHandler::new(PathBuf::from("."))))
            .ok();
        let runtime = AgentRuntime::for_test().await;
        handler_registry
            .register(Arc::new(SessionHandler::new(
                runtime.session_manager.clone(),
                router.clone(),
            )))
            .ok();
        handler_registry
            .register(Arc::new(McpHandler::new(None)))
            .ok();
        handler_registry
            .register(Arc::new(SkillHandler::new(None)))
            .ok();
        handler_registry
            .register(Arc::new(ToolHandler::new(Arc::new(ToolRegistry::new()))))
            .ok();
        handler_registry.register(Arc::new(LogHandler)).ok();
        handler_registry.register(Arc::new(SystemHandler)).ok();
        handler_registry
            .register(Arc::new(TaskHandler::new(Arc::new(
                vol_llm_task::InMemoryTaskStore::new(),
            ))))
            .ok();
        handler_registry
            .register(Arc::new(DataPlaneControlHandler::new()))
            .ok();

        let sandbox_registry = runtime.sandbox_registry.clone();

        DataPlaneServerCore {
            runtime,
            working_dir: PathBuf::from("."),
            store_dir,
            mcp_manager: Arc::new(McpManager::new(vec![])),
            skill_loader: Arc::new(SkillLoader::new_empty()),
            tool_registry: Arc::new(ToolRegistry::new()),
            sandbox_registry,
            llm: Arc::new(TestLlm),
            router,
            holders,
            agent_defs,
            agent_status,
            handler_registry,
        }
    }
}

#[cfg(test)]
mod store_paths_tests {
    use super::StorePaths;
    use std::path::PathBuf;

    #[test]
    fn store_paths_agent_dir_joins_agents_root() {
        let paths = StorePaths {
            root: PathBuf::from("/tmp/store"),
            sessions: PathBuf::from("/tmp/store/sessions"),
            agents_root: PathBuf::from("/tmp/store/agents"),
        };
        assert_eq!(
            paths.agent_dir("agent-a"),
            PathBuf::from("/tmp/store/agents/agent-a")
        );
    }

    #[test]
    fn store_paths_ensure_dirs_creates_directories() {
        let tmp = std::env::temp_dir().join("vol-agent-test-store-paths");
        let sessions = tmp.join("sessions");
        let agents_root = tmp.join("agents");
        let paths = StorePaths {
            root: tmp.clone(),
            sessions: sessions.clone(),
            agents_root: agents_root.clone(),
        };
        paths.ensure_dirs().unwrap();
        assert!(sessions.exists());
        assert!(agents_root.exists());
        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
