//! Unified core for the agent server.
//!
//! `AgentServerCore` is the single source of truth for shared resources:
//! - working directory and store directory
//! - MCP manager, skill loader, session store
//! - agent router, connection holders, LLM client
//! - domain handlers that hold the resources they need

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use vol_llm_core::LLMClient;
use vol_llm_mcp::McpManager;
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
pub struct AgentServerCore {
    // === Paths ===
    working_dir: PathBuf,
    store_dir: PathBuf,

    // === Registries ===
    mcp_manager: Option<Arc<McpManager>>,
    skill_loader: Option<Arc<SkillLoader>>,
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
    /// Create a new builder.
    pub fn builder() -> AgentServerCoreBuilder {
        AgentServerCoreBuilder::new()
    }

    // === Path accessors ===

    pub fn working_dir(&self) -> &std::path::Path {
        &self.working_dir
    }

    pub fn store_dir(&self) -> &std::path::Path {
        &self.store_dir
    }

    // === Registry accessors ===

    pub fn mcp_manager(&self) -> Option<&Arc<McpManager>> {
        self.mcp_manager.as_ref()
    }

    pub fn skill_loader(&self) -> Option<&Arc<SkillLoader>> {
        self.skill_loader.as_ref()
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

        let session = Arc::new(vol_session::Session::new(session_store));
        let mut config = vol_llm_agent::react::AgentConfig::new(llm, tools, session);
        config.def = Some(def);
        config.working_dir = self.working_dir.clone();
        if let Some(mcp) = &self.mcp_manager {
            config.mcp_manager = Some(mcp.clone());
        }

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
pub struct AgentServerCoreBuilder {
    working_dir: Option<PathBuf>,
    store_dir: Option<PathBuf>,
    llm: Option<Arc<dyn LLMClient>>,
    mcp_manager: Option<Arc<McpManager>>,
    skill_loader: Option<Arc<SkillLoader>>,
    session_store: Option<Arc<FileSessionEntryStore>>,
    tool_registry: Option<Arc<ToolRegistry>>,
}

impl AgentServerCoreBuilder {
    pub fn new() -> Self {
        Self {
            working_dir: None,
            store_dir: None,
            llm: None,
            mcp_manager: None,
            skill_loader: None,
            session_store: None,
            tool_registry: None,
        }
    }

    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn store_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.store_dir = Some(dir.into());
        self
    }

    pub fn llm(mut self, llm: Arc<dyn LLMClient>) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn mcp_manager(mut self, mcp: Arc<McpManager>) -> Self {
        self.mcp_manager = Some(mcp);
        self
    }

    pub fn skill_loader(mut self, loader: Arc<SkillLoader>) -> Self {
        self.skill_loader = Some(loader);
        self
    }

    pub fn session_store(mut self, store: Arc<FileSessionEntryStore>) -> Self {
        self.session_store = Some(store);
        self
    }

    pub fn tool_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    /// Build the core. Returns the core with all domain handlers initialized.
    pub async fn build(self) -> Result<AgentServerCore, String> {
        let working_dir = self.working_dir.unwrap_or_else(|| PathBuf::from("."));
        let store_dir = self.store_dir.ok_or("store_dir is required")?;
        let llm = self.llm.ok_or("llm is required")?;

        // Expand ~ in store_dir to home directory.
        let store_dir = expand_tilde(store_dir);

        // Create store directory if it doesn't exist.
        std::fs::create_dir_all(&store_dir).map_err(|e| format!("failed to create store_dir: {e}"))?;

        let session_store = self.session_store.unwrap_or_else(|| {
            Arc::new(FileSessionEntryStore::new(&store_dir))
        });
        let tool_registry = self.tool_registry.unwrap_or_else(|| {
            Arc::new(ToolRegistry::new())
        });

        let router = AgentRouter::new();
        let holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));

        let agent = AgentHandler::new(router.clone(), Arc::clone(&holders));
        let file = FileHandler::new(working_dir.clone());
        let session = SessionHandler::new(session_store.clone());
        let mcp = McpHandler::new(self.mcp_manager.clone());
        let skill = SkillHandler::new(self.skill_loader.clone());
        let log = LogHandler;
        let system = SystemHandler;

        Ok(AgentServerCore {
            working_dir,
            store_dir,
            mcp_manager: self.mcp_manager,
            skill_loader: self.skill_loader,
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

impl Default for AgentServerCoreBuilder {
    fn default() -> Self {
        Self::new()
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

        // Create a dummy LLM for tests (will panic if actually used — tests should mock it).
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
            mcp_manager: None,
            skill_loader: None,
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
