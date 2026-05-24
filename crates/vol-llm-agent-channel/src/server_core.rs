//! Unified core for the agent server.
//!
//! `AgentServerCore` is the single source of truth for shared resources.
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

use vol_llm_core::LLMClient;
use vol_llm_mcp::McpConfig;
use vol_llm_mcp::McpManager;
use vol_llm_provider::{create_provider, ProviderLoader};
use vol_llm_skill::SkillLoader;
use vol_llm_tool::ToolRegistry;
use vol_session::file_store::FileSessionEntryStore;

use crate::connection::ConnectionHolder;
use crate::dispatcher::AgentDispatcher;
use crate::domain::registry::HandlerRegistry;
use crate::domain::{
    agent::AgentHandler, file::FileHandler, log::LogHandler,
    mcp::McpHandler, session::SessionHandler, skill::SkillHandler, system::SystemHandler,
    tool::ToolHandler,
};
use crate::router::AgentRouter;

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

/// Runtime status of a registered agent.
#[derive(Debug, Clone, Default)]
pub struct AgentStatus {
    pub status: String,  // "idle" | "running"
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

/// Shared core for the agent server.
///
/// The core owns all shared resources (paths, registries, agent runtime).
/// Domain handlers hold the specific resources they need (no self-reference).
///
/// Given only `working_dir` and `store_dir`, all internal registries are
/// derived automatically (LLM, MCP, skills, agents, tools, sessions).
pub struct AgentServerCore {
    // === Paths ===
    working_dir: PathBuf,
    store_dir: PathBuf,

    // === Registries ===
    mcp_manager: Arc<McpManager>,
    skill_loader: Arc<SkillLoader>,
    tool_registry: Arc<ToolRegistry>,

    // === Agent runtime ===
    llm: Arc<dyn LLMClient>,
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,

    // === Agent definitions ===
    agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_core::AgentDef>>>,

    // === Agent status ===
    agent_status: Arc<std::sync::RwLock<HashMap<String, AgentStatus>>>,

    // === Domain handlers ===
    handler_registry: HandlerRegistry,
}

impl AgentServerCore {
    /// Create a new core from paths only.
    ///
    /// All internal registries (LLM, MCP, skills, tools, sessions) are
    /// derived automatically from working_dir and store_dir.
    pub async fn new(working_dir: impl Into<PathBuf>, store_dir: impl Into<PathBuf>) -> Result<Self, String> {
        Self::builder(working_dir, store_dir).build().await
    }

    /// Create a builder for optional override.
    pub fn builder(working_dir: impl Into<PathBuf>, store_dir: impl Into<PathBuf>) -> AgentServerCoreBuilder {
        AgentServerCoreBuilder::new(working_dir.into(), store_dir.into())
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
    pub async fn register_agent(
        &self,
        agent_id: impl Into<String>,
        def: vol_llm_core::agent_def::AgentDef,
    ) -> Result<(), String> {
        let agent_id = agent_id.into();
        let agent_dir = self.store_dir.join("agents").join(&agent_id);
        let sessions_dir = agent_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir).map_err(|e| format!("failed to create agent dirs: {e}"))?;

        let llm = self.llm.clone();
        let tools = self.tool_registry.clone();
        let mcp = self.mcp_manager.clone();

        let session_store = Arc::new(FileSessionEntryStore::new(&sessions_dir));
        let session = Arc::new(vol_session::Session::new(session_store));

        let mut config = vol_llm_agent::react::AgentConfig::new(llm, tools, session);
        config.def = Some(def);
        // AgentConfig.working_dir 控制日志等内部路径 → agent 私有目录
        config.working_dir = agent_dir.clone();
        config.mcp_manager = Some(mcp);

        let holder = ConnectionHolder::new(
            agent_id.clone(),
            "client".to_string(),
            Some(self.agent_status.clone()),
        );
        config.plugin_registry.register(holder.clone());

        let agent = vol_llm_agent::ReActAgent::new(config);
        let dispatcher = Arc::new(AgentDispatcher::new(agent));

        self.router.register(agent_id.clone(), dispatcher).await;
        self.holders.lock().unwrap().insert(agent_id, Arc::new(holder));

        Ok(())
    }

    /// List all registered agent IDs.
    pub async fn list_agent_ids(&self) -> Vec<String> {
        self.holders.lock().unwrap().keys().cloned().collect()
    }

    /// Discover and register all agents from .agents/agents/ directories.
    pub async fn discover_agents(&self) -> Result<(), String> {
        let loader = vol_llm_agent::AgentLoader::new(Some(self.working_dir.clone()));
        loader.discover_all().await.map_err(|e| e.to_string())?;

        let agents = loader.list_metadata().await;
        for meta in agents {
            if let Some(def) = loader.get(&meta.name).await {
                // Store def for metadata queries
                self.agent_defs.write().unwrap().insert(meta.name.clone(), (*def).clone());
                let arc_def = Arc::try_unwrap(def).unwrap_or_else(|arc| (*arc).clone());
                self.register_agent(&meta.name, arc_def).await?;
                self.agent_status.write().unwrap().insert(meta.name.clone(), AgentStatus::idle());
            }
        }
        Ok(())
    }

    /// Handle an inbound `AgentServerMessage` by dispatching via the handler registry.
    pub async fn handle(
        &self,
        message: crate::agent_server_protocol::AgentServerMessage,
    ) -> Result<Vec<crate::agent_server_protocol::AgentServerMessage>, crate::agent_server_protocol::ProtocolError> {
        self.handler_registry.dispatch(message).await
    }

    /// Serve incoming messages from a connection, dispatching each to the handler registry.
    ///
    /// Attaches the connection to all holders so agent events flow through.
    /// Loops `recv() → handle() → send()` until the connection closes or errors.
    pub async fn serve(&self, conn: impl crate::connection::Connection) {
        // Attach to all holders so agent events are pushed to this connection.
        let conn: Arc<dyn crate::connection::Connection> = Arc::new(conn);
        let holders: Vec<_> = {
            self.holders.lock().unwrap().values().cloned().collect()
        };
        for holder in &holders {
            holder.attach(conn.clone()).await;
        }

        while let Some(result) = conn.recv().await {
            let responses = match result {
                Ok(msg) => match self.handle(msg).await {
                    Ok(resp) => resp,
                    Err(e) => vec![crate::agent_server_protocol::AgentServerMessage::new_error(
                        uuid::Uuid::new_v4().to_string(),
                        crate::agent_server_protocol::Operation::System(
                            crate::agent_server_protocol::SystemOperation::Connected,
                        ),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "dispatch_error".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: false,
                        },
                    )],
                },
                Err(e) => {
                    tracing::warn!(%e, "connection receive error");
                    break;
                }
            };
            for resp in responses {
                if let Err(e) = conn.send(resp).await {
                    tracing::warn!(%e, "connection send error");
                    return;
                }
            }
        }
    }
}

/// Builder for [`AgentServerCore`].
///
/// Only `working_dir` and `store_dir` are required.
/// All other resources (LLM, MCP, skills, tools, sessions) are derived automatically.
pub struct AgentServerCoreBuilder {
    working_dir: PathBuf,
    store_dir: PathBuf,
    extra_handlers: Vec<Arc<dyn crate::domain::handler::DomainHandler>>,
}

impl Default for AgentServerCoreBuilder {
    fn default() -> Self {
        Self {
            working_dir: PathBuf::new(),
            store_dir: PathBuf::new(),
            extra_handlers: Vec::new(),
        }
    }
}

impl AgentServerCoreBuilder {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        Self { working_dir, store_dir, extra_handlers: Vec::new() }
    }

    /// Register an external domain handler.
    pub fn register_handler(mut self, handler: Arc<dyn crate::domain::handler::DomainHandler>) -> Self {
        self.extra_handlers.push(handler);
        self
    }

    /// Build the core. All internal registries are derived from working_dir and store_dir.
    pub async fn build(self) -> Result<AgentServerCore, String> {
        // Expand ~ in store_dir to home directory.
        let store_dir = expand_tilde(self.store_dir);

        // Create store subdirectories: sessions/ agents/
        let agents_root = store_dir.join("agents");
        std::fs::create_dir_all(&agents_root).map_err(|e| format!("failed to create agents dir: {e}"))?;

        // Derive LLM client from .agents/providers/*.toml (default: first provider found).
        let llm = derive_llm_client(&self.working_dir)?;

        // Derive MCP manager from .mcp.json in working_dir.
        let mcp_manager = derive_mcp_manager(&self.working_dir);

        // Derive skill loader from .agents/skills/ in working_dir.
        let skill_loader = derive_skill_loader(&self.working_dir);

        // Derive tool registry with built-in tools.
        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        let tool_registry = Arc::new(tool_registry);

        let router = AgentRouter::new();
        let holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));
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
            .map_err(|e| format!("failed to register AgentHandler: {e}"))?;
        handler_registry
            .register(Arc::new(FileHandler::new(self.working_dir.clone())))
            .map_err(|e| format!("failed to register FileHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SessionHandler::new(agents_root)))
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

        // Register extra handlers from builder (external registration).
        for extra in self.extra_handlers {
            handler_registry
                .register(extra)
                .map_err(|e| format!("failed to register external handler: {e}"))?;
        }

        Ok(AgentServerCore {
            working_dir: self.working_dir,
            store_dir,
            mcp_manager,
            skill_loader,
            tool_registry,
            llm,
            router,
            holders,
            agent_defs,
            agent_status,
            handler_registry,
        })
    }
}

fn derive_mcp_manager(working_dir: &std::path::Path) -> Arc<McpManager> {
    let configs = McpConfig::load(Some(working_dir))
        .map(|c| c.servers().to_vec())
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to load MCP config, using empty config: {}", e);
            vec![]
        });
    let manager = McpManager::new(configs);
    let mgr = manager.clone();
    tokio::spawn(async move {
        let _ = mgr.connect().await;
    });
    Arc::new(manager)
}

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
            Err(e) => errors.push(format!("{}: {}", id, e)),
        }
    }
    Err(format!("No usable LLM provider found. Errors: {}", errors.join("; ")))
}

fn derive_skill_loader(working_dir: &std::path::Path) -> Arc<SkillLoader> {
    let loader = Arc::new(SkillLoader::new(Some(working_dir.to_path_buf())));
    // Fire-and-forget discover in background.
    let ld = Arc::clone(&loader);
    tokio::spawn(async move {
        let _ = ld.discover_all().await;
    });
    loader
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

/// Test constructor that provides minimal defaults for all optional fields.
impl AgentServerCore {
    #[doc(hidden)]
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
            fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
            fn model(&self) -> &str { "test" }
            fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
            async fn converse(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> {
                Err(vol_llm_core::LLMError::Parse("test LLM not implemented".into()))
            }
            async fn converse_stream(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> {
                let (_tx, rx) = tokio::sync::mpsc::channel(1);
                Ok(vol_llm_core::StreamReceiver::new(rx))
            }
        }

        // Register a test agent dispatcher so submit flow works.
        {
            use crate::dispatcher::AgentDispatcher;
            use vol_llm_agent::ReActAgent;
            use vol_llm_agent::react::AgentConfig;
            use vol_session::Session;
            use vol_session::InMemoryEntryStore;

            let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
            let tools: Arc<vol_llm_tool::ToolRegistry> = Arc::new(vol_llm_tool::ToolRegistry::new());
            let config = AgentConfig::new(Arc::new(TestLlm), tools, session);
            let agent = ReActAgent::new(config);
            let dispatcher = Arc::new(AgentDispatcher::new(agent));
            let holder = Arc::new(ConnectionHolder::new("test_agent".to_string(), "client".to_string(), None));
            router.register("test_agent".to_string(), dispatcher).await;
            holders.lock().unwrap().insert("test_agent".to_string(), holder);
        }

        let agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_core::AgentDef>>> =
            Arc::new(std::sync::RwLock::new(HashMap::new()));
        let agent_status: Arc<std::sync::RwLock<HashMap<String, AgentStatus>>> =
            Arc::new(std::sync::RwLock::new(HashMap::new()));
        let mut handler_registry = HandlerRegistry::new();
        handler_registry.register(Arc::new(AgentHandler::new(
            router.clone(),
            Arc::clone(&holders),
            agent_defs.clone(),
            agent_status.clone(),
        ))).ok();
        handler_registry.register(Arc::new(FileHandler::new(PathBuf::from(".")))).ok();
        handler_registry.register(Arc::new(SessionHandler::new(agents_root))).ok();
        handler_registry.register(Arc::new(McpHandler::new(None))).ok();
        handler_registry.register(Arc::new(SkillHandler::new(None))).ok();
        handler_registry.register(Arc::new(ToolHandler::new(Arc::new(ToolRegistry::new())))).ok();
        handler_registry.register(Arc::new(LogHandler)).ok();
        handler_registry.register(Arc::new(SystemHandler)).ok();

        AgentServerCore {
            working_dir: PathBuf::from("."),
            store_dir,
            mcp_manager: Arc::new(McpManager::new(vec![])),
            skill_loader: Arc::new(SkillLoader::new_empty()),
            tool_registry: Arc::new(ToolRegistry::new()),
            llm: Arc::new(TestLlm),
            router,
            holders,
            agent_defs,
            agent_status,
            handler_registry,
        }
    }
}
