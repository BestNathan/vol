//! Unified core for the agent server.
//!
//! `AgentServerCore` is the single source of truth for shared resources.
//! Given only `working_dir` and `store_dir`, it derives all internal
//! registries (MCP, skills, agents, tools, sessions) automatically.

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
use crate::domain::{
    agent::AgentHandler, file::FileHandler, log::LogHandler, mcp::McpHandler,
    session::SessionHandler, skill::SkillHandler, system::SystemHandler,
};
use crate::router::AgentRouter;

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
    session_store: Arc<FileSessionEntryStore>,
    tool_registry: Arc<ToolRegistry>,

    // === Agent runtime ===
    llm: Arc<dyn LLMClient>,
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,

    // === Domain handlers ===
    pub agent: AgentHandler,
    pub file: FileHandler,
    pub session: SessionHandler,
    pub mcp: McpHandler,
    pub skill: SkillHandler,
    pub log: LogHandler,
    pub system: SystemHandler,
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

    pub fn session_store(&self) -> &Arc<FileSessionEntryStore> {
        &self.session_store
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

    /// Register a new agent with the given id and definition.
    ///
    /// Creates ReActAgent with ConnectionHolder as event bridge, Dispatcher with FIFO queue,
    /// and registers them with the router.
    pub async fn register_agent(
        &self,
        agent_id: impl Into<String>,
        def: vol_llm_agent::agent_def::AgentDef,
    ) -> Result<(), String> {
        let agent_id = agent_id.into();

        let llm = self.llm.clone();
        let tools = self.tool_registry.clone();
        let session_store = self.session_store.clone();
        let mcp = self.mcp_manager.clone();

        let session = Arc::new(vol_session::Session::new(session_store));
        let mut config = vol_llm_agent::react::AgentConfig::new(llm, tools, session);
        config.def = Some(def);
        config.working_dir = self.working_dir.clone();
        config.mcp_manager = Some(mcp);

        let holder = ConnectionHolder::new(agent_id.clone(), "client".to_string());
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
                let arc_def = Arc::try_unwrap(def).unwrap_or_else(|arc| (*arc).clone());
                self.register_agent(&meta.name, arc_def).await?;
            }
        }
        Ok(())
    }

    /// Handle an inbound `AgentServerMessage` by dispatching to the appropriate domain handler.
    pub async fn handle(
        &self,
        message: crate::agent_server_protocol::AgentServerMessage,
    ) -> Result<Vec<crate::agent_server_protocol::AgentServerMessage>, crate::agent_server_protocol::ProtocolError> {
        use crate::agent_server_protocol::Operation;
        match message.operation.clone() {
            Operation::Agent(op) => self.agent.handle(op, message).await,
            Operation::File(op) => self.file.handle(op, message).await,
            Operation::Session(op) => self.session.handle(op, message).await,
            Operation::Mcp(op) => self.mcp.handle(op, message).await,
            Operation::Skill(op) => self.skill.handle(op, message).await,
            Operation::Log(op) => self.log.handle(op, message).await,
            Operation::System(op) => self.system.handle(op, message).await,
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
}

impl AgentServerCoreBuilder {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        Self { working_dir, store_dir }
    }

    /// Build the core. All internal registries are derived from working_dir and store_dir.
    pub async fn build(self) -> Result<AgentServerCore, String> {
        // Expand ~ in store_dir to home directory.
        let store_dir = expand_tilde(self.store_dir);

        // Create store directory if it doesn't exist.
        std::fs::create_dir_all(&store_dir).map_err(|e| format!("failed to create store_dir: {e}"))?;

        // Derive LLM client from .agents/providers/*.toml (default: first provider found).
        let llm = derive_llm_client(&self.working_dir)?;

        // Derive MCP manager from .mcp.json in working_dir.
        let mcp_manager = derive_mcp_manager(&self.working_dir);

        // Derive skill loader from .agents/skills/ in working_dir.
        let skill_loader = derive_skill_loader(&self.working_dir);

        // Derive session store from store_dir.
        let session_store = Arc::new(FileSessionEntryStore::new(&store_dir));

        // Derive tool registry (empty by default, agents populate it).
        let tool_registry = Arc::new(ToolRegistry::new());

        let router = AgentRouter::new();
        let holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));

        let agent = AgentHandler::new(router.clone(), Arc::clone(&holders));
        let file = FileHandler::new(self.working_dir.clone());
        let session = SessionHandler::new(session_store.clone());
        let mcp = McpHandler::new(Some(mcp_manager.clone()));
        let skill = SkillHandler::new(Some(skill_loader.clone()));
        let log = LogHandler;
        let system = SystemHandler;

        Ok(AgentServerCore {
            working_dir: self.working_dir,
            store_dir,
            mcp_manager,
            skill_loader,
            session_store,
            tool_registry,
            llm,
            router,
            holders,
            agent,
            file,
            session,
            mcp,
            skill,
            log,
            system,
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
    pub fn for_test() -> Self {
        use std::sync::Arc;

        let store_dir = PathBuf::from("/tmp/vol-llm-agent-channel-test-sessions");
        std::fs::create_dir_all(&store_dir).ok();

        let session_store = Arc::new(FileSessionEntryStore::new(&store_dir));
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

        let agent = AgentHandler::new(router.clone(), Arc::clone(&holders));
        let file = FileHandler::new(PathBuf::from("."));
        let session = SessionHandler::new(session_store.clone());
        let mcp = McpHandler::new(None);
        let skill = SkillHandler::new(None);
        let log = LogHandler;
        let system = SystemHandler;

        AgentServerCore {
            working_dir: PathBuf::from("."),
            store_dir,
            mcp_manager: Arc::new(McpManager::new(vec![])),
            skill_loader: Arc::new(SkillLoader::new_empty()),
            session_store,
            tool_registry: Arc::new(ToolRegistry::new()),
            llm: Arc::new(TestLlm),
            router,
            holders,
            agent,
            file,
            session,
            mcp,
            skill,
            log,
            system,
        }
    }
}
