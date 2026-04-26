//! CodingAgent - AI-powered code assistant.

use std::sync::Arc;
use std::path::PathBuf;
use vol_llm_agent::react::SkillsConfig;
use vol_llm_tool::{ToolRegistry, ToolConfig};
use vol_llm_agent::{ReActAgent, AgentConfig};
use vol_llm_context::ContextBuilder;
use vol_session::Session;
use vol_llm_provider::{LLMProviderConfig, LLMProviderRegistry};
use crate::coding::config::CodingAgentConfig;
use crate::coding::error::CodingAgentError;
use crate::coding::observer::EventObserver;
use crate::coding::observer_plugin::ObserverPlugin;
use vol_llm_core::{Sandbox, LLMProvider};
use crate::coding::sandbox::LocalSandbox;

/// Coding Agent response
#[derive(Debug, Clone)]
pub struct CodingAgentResponse {
    pub success: bool,
    pub summary: String,
    pub iterations: u32,
    pub tool_calls: u32,
}

/// Coding Agent
pub struct CodingAgent {
    config: CodingAgentConfig,
    llm: Arc<dyn vol_llm_core::LLMClient>,
    tool_registry: Arc<ToolRegistry>,
    context_builder: ContextBuilder,
    observer: Option<Arc<dyn EventObserver>>,
    sandbox: Option<vol_llm_core::SandboxRef>,
}

impl CodingAgent {
    /// Create a new CodingAgent from config.
    ///
    /// If `config.llm` is None, an LLM is created from `ANTHROPIC_AUTH_TOKEN`.
    /// If `config.working_dir` is not ".", a LocalSandbox is automatically created.
    pub fn new(config: CodingAgentConfig) -> Result<Self, CodingAgentError> {
        let llm = Self::resolve_llm(&config)?;
        let (tool_registry, context_builder) = Self::build_tools_and_context(&config)?;
        let sandbox = Self::init_sandbox(&config.working_dir)?;

        Ok(Self {
            config,
            llm,
            tool_registry,
            context_builder,
            observer: None,
            sandbox,
        })
    }

    /// Resolve LLM from config or create from env.
    fn resolve_llm(config: &CodingAgentConfig) -> Result<Arc<dyn vol_llm_core::LLMClient>, CodingAgentError> {
        if let Some(llm) = &config.llm {
            return Ok(llm.clone());
        }

        let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
            .map_err(|_| CodingAgentError::Config(
                "ANTHROPIC_AUTH_TOKEN not set and no LLM client provided".to_string()
            ))?;
        let llm_config = LLMProviderConfig {
            id: config.llm_provider_id.clone(),
            config: vol_llm_provider::LLMConfig {
                provider: LLMProvider::Anthropic,
                model: "qwen3.5-plus".to_string(),
                api_key: vol_llm_provider::Secret::literal(api_key),
                base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
            },
        };
        let registry = LLMProviderRegistry::from_configs(&[llm_config])
            .map_err(|e| CodingAgentError::Config(format!("LLM provider error: {}", e)))?;
        registry.get(&config.llm_provider_id)
            .ok_or_else(|| CodingAgentError::Config(
                format!("LLM provider '{}' not found", config.llm_provider_id)
            ))
            .map(|llm| llm.clone())
    }

    /// Build tool registry and context builder together.
    fn build_tools_and_context(config: &CodingAgentConfig) -> Result<(Arc<ToolRegistry>, ContextBuilder), CodingAgentError> {
        let mut tool_registry = ToolRegistry::new();
        Self::register_coding_tools(&mut tool_registry, &config.tool_config);

        let skills = SkillsConfig::from_workdir(&config.working_dir);
        skills.register_tool(&mut tool_registry);

        let base_context = vol_llm_context::ContextBuilderBuilder::new(128_000)
            .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
                "You are an expert coding assistant. Help users understand, modify, and improve their codebase.".to_string(),
            )))
            .build();
        let context_builder = skills.enhance_context_builder(&base_context);

        Ok((Arc::new(tool_registry), context_builder))
    }

    /// Initialize sandbox if working_dir is not ".".
    fn init_sandbox(working_dir: &PathBuf) -> Result<Option<vol_llm_core::SandboxRef>, CodingAgentError> {
        if working_dir == &PathBuf::from(".") {
            return Ok(None);
        }
        let sandbox = LocalSandbox::new(Some(working_dir.clone()));
        sandbox.start().map_err(|e| CodingAgentError::Config(
            format!("Failed to start sandbox at {:?}: {}", working_dir, e)
        ))?;
        Ok(Some(Arc::new(sandbox)))
    }


    /// Register coding tools and web tools to the tool registry
    fn register_coding_tools(registry: &mut ToolRegistry, tool_config: &ToolConfig) {
        use vol_llm_tools_builtin::read_tool::ReadTool;
        use vol_llm_tools_builtin::write_tool::WriteTool;
        use vol_llm_tools_builtin::edit_tool::EditTool;
        use vol_llm_tools_builtin::glob_tool::GlobTool;
        use vol_llm_tools_builtin::grep_tool::GrepTool;
        use vol_llm_tools_builtin::bash_tool::BashTool;

        registry.register(ReadTool::new());
        registry.register(WriteTool::new());
        registry.register(EditTool::new());
        registry.register(GlobTool::new());
        registry.register(GrepTool::new());
        registry.register(BashTool::new());

        // Register web tools if configured
        vol_llm_tools_builtin::register_web_all(registry, tool_config);
    }

    /// Set the event observer and register plugin
    pub fn with_observer(mut self, observer: Arc<dyn EventObserver>) -> Self {
        // Register plugin with config's plugin_registry
        let mut new_config = self.config.clone();
        new_config.plugin_registry.register(ObserverPlugin::new(observer.clone()));
        self.config = new_config;

        self.observer = Some(observer);
        self
    }

    /// Get the agent's configuration
    pub fn config(&self) -> &CodingAgentConfig {
        &self.config
    }

    /// Get the event observer
    pub fn observer(&self) -> Option<&Arc<dyn EventObserver>> {
        self.observer.as_ref()
    }

    /// Set the sandbox for tool execution (overrides auto-init from working_dir)
    pub fn with_sandbox(mut self, sandbox: vol_llm_core::SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// Set the agent identifier (for log paths, etc.)
    pub fn with_agent_id(mut self, agent_id: String) -> Self {
        self.config.agent_id = agent_id;
        self
    }

    /// Generate missing context files from built-in templates.
    /// Files that already exist are not overwritten.
    pub fn init_context_files(&self) {
        const AGENT_MD: &str = "# Agent\n\nDefine your role and behavior here.\n";
        const INSTRUCTION_MD: &str = "# Instructions\n\nAdd project-specific instructions here.\n";
        const CLI_MD: &str = "# CLI Reference\n\nDocument available CLI tools and commands here.\n";

        let dir = &self.config.working_dir;

        for (filename, content) in &[
            ("AGENT.md", AGENT_MD),
            ("INSTRUCTION.md", INSTRUCTION_MD),
            ("CLI.md", CLI_MD),
        ] {
            let path = dir.join(filename);
            if !path.exists() {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to create context file");
                }
            }
        }
    }

    /// Build an AgentConfig for a single ReActAgent run.
    fn build_agent_config(&self) -> AgentConfig {
        AgentConfig {
            max_iterations: self.config.max_iterations,
            max_history_messages: 20,
            context_builder: self.context_builder.clone(),
            plugin_registry: self.config.plugin_registry.clone(),
            agent_id: self.config.agent_id.clone(),
            working_dir: self.config.working_dir.clone(),
            unsafe_mode: self.config.unsafe_mode,
            approval_handler: self.config.approval_handler.clone(),
        }
    }

    /// Run a coding task
    pub async fn run(&self, task: &str) -> Result<CodingAgentResponse, CodingAgentError> {
        // Create session for this run — use shared session from config if available
        let session = match &self.config.session {
            Some(s) => s.clone(),
            None => {
                use vol_session::InMemoryEntryStore;
                Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())))
            }
        };

        // Build AgentConfig on-demand
        let agent_config = self.build_agent_config();

        let mut react_agent = ReActAgent::new(
            self.llm.clone(),
            self.tool_registry.clone(),
            agent_config,
            session,
        );

        if let Some(ref sandbox) = self.sandbox {
            react_agent = react_agent.with_sandbox(sandbox.clone());
        }

        // Run the ReActAgent
        // Note: ObserverPlugin receives all events via PluginRegistry,
        // including AgentStart and AgentComplete emitted by ReActAgent itself.
        let response = react_agent.run(task).await
            .map_err(|e| CodingAgentError::Agent(e))?;

        // Signal completion to observer (for report generation)
        if let Some(ref observer) = self.observer {
            observer.on_complete().await
                .map_err(|e| CodingAgentError::Observer(e))?;
        }

        // Extract summary from response
        let summary = response.content.clone();
        let iterations = response.iterations;
        let tool_calls = response.tool_calls.len() as u32;

        Ok(CodingAgentResponse {
            success: true,
            summary,
            iterations,
            tool_calls,
        })
    }
    /// Resume a session from disk by its session ID.
    ///
    /// Loads the session from `{store_dir}/sessions` (or creates a new
    /// in-memory session if the ID is not found) and stores it on the agent.
    /// Subsequent `run()` calls use this session.
    ///
    /// Consumes `self` so it can be chained with `.run()`.
    pub async fn resume(mut self, session_id: &str) -> Result<Self, CodingAgentError> {
        use vol_session::{FileSessionEntryStore, InMemoryEntryStore};

        let session_dir = self.config.store_dir.join("sessions");
        let entry_store: Arc<dyn vol_session::SessionEntryStore> =
            if session_dir.exists() {
                Arc::new(FileSessionEntryStore::new(&session_dir))
            } else {
                Arc::new(InMemoryEntryStore::new())
            };
        let session = match Session::resume(session_id.to_string(), entry_store).await {
            Ok(s) => Arc::new(s),
            Err(_) => {
                Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())))
            }
        };

        self.config.session = Some(session);
        Ok(self)
    }
}

/// Builder pattern for CodingAgent
pub struct CodingAgentBuilder {
    config: CodingAgentConfig,
    sandbox: Option<vol_llm_core::SandboxRef>,
    store_dir_set: bool,
}

impl CodingAgentBuilder {
    pub fn new() -> Self {
        Self {
            config: CodingAgentConfig::default(),
            sandbox: None,
            store_dir_set: false,
        }
    }

    pub fn config(mut self, config: CodingAgentConfig) -> Self {
        self.config = config;
        self
    }

    pub fn max_iterations(mut self, max: u32) -> Self {
        self.config.max_iterations = max;
        self
    }

    pub fn working_dir(mut self, path: PathBuf) -> Self {
        self.config.working_dir = path.clone();
        if !self.store_dir_set {
            let basename = path
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("default"))
                .to_string_lossy();
            let home = std::env::var("HOME").unwrap_or_default();
            self.config.store_dir =
                PathBuf::from(home).join(".vol-coding").join(basename.as_ref());
        }
        self
    }

    /// Set the storage directory for sessions, logs, and other persistent data.
    /// When called, it overrides the auto-derived store_dir from working_dir.
    pub fn store_dir(mut self, path: PathBuf) -> Self {
        self.config.store_dir = path;
        self.store_dir_set = true;
        self
    }

    pub fn hitl_enabled(mut self, enabled: bool) -> Self {
        self.config.hitl_enabled = enabled;
        self
    }

    pub fn unsafe_mode(mut self, enabled: bool) -> Self {
        self.config.unsafe_mode = enabled;
        self
    }

    pub fn approval_handler(mut self, handler: vol_llm_agent::react::BoxedApprovalHandler) -> Self {
        self.config.approval_handler = Some(handler);
        self
    }

    pub fn html_report_path(mut self, path: Option<PathBuf>) -> Self {
        self.config.html_report_path = path;
        self
    }

    pub fn sandbox(mut self, sandbox: vol_llm_core::SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// Set the LLM client for this agent.
    /// The caller constructs the LLM; CodingAgent does not read env vars.
    pub fn llm(mut self, llm: Arc<dyn vol_llm_core::LLMClient>) -> Self {
        self.config.llm = Some(llm);
        self
    }

    pub fn tool_config(mut self, tool_config: ToolConfig) -> Self {
        self.config.tool_config = tool_config;
        self
    }

    /// Register LoggerPlugin to write JSONL event logs to store_dir/logs/.
    pub fn with_logger(mut self) -> Self {
        let logger = vol_llm_observability::LoggerPlugin::new(self.config.store_dir.clone());
        self.config.plugin_registry.register(logger);
        self
    }

    /// Set the shared session for conversation history.
    pub fn session(mut self, session: Arc<vol_session::Session>) -> Self {
        self.config.session = Some(session);
        self
    }

    /// Set the LLM provider ID (used when `llm` is None).
    pub fn llm_provider_id(mut self, id: String) -> Self {
        self.config.llm_provider_id = id;
        self
    }

    pub fn build(self) -> Result<CodingAgent, CodingAgentError> {
        let mut agent = CodingAgent::new(self.config)?;
        if let Some(sandbox) = self.sandbox {
            agent.sandbox = Some(sandbox);
        }
        Ok(agent)
    }
}

impl Default for CodingAgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}
