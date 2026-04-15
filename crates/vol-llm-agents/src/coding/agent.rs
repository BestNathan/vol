//! CodingAgent - AI-powered code assistant.

use std::sync::Arc;
use std::path::PathBuf;
use vol_llm_tool::{ToolRegistry, ToolConfig};
use vol_llm_agent::{ReActAgent, AgentConfig, Session};
use crate::coding::config::CodingAgentConfig;
use crate::coding::error::CodingAgentError;
use crate::coding::observer::EventObserver;
use crate::coding::observer_plugin::ObserverPlugin;
use vol_llm_core::Sandbox;
use crate::coding::sandbox::LocalSandbox;

/// Coding Agent response
#[derive(Debug, Clone)]
pub struct CodingAgentResponse {
    pub success: bool,
    pub summary: String,
    pub iterations: u32,
    pub tool_calls: u32,
}

/// Internal state for CodingAgent
struct CodingAgentState {
    llm: Arc<dyn vol_llm_core::LLMClient>,
    tool_registry: Arc<ToolRegistry>,
    agent_config: AgentConfig,
}

/// Coding Agent
pub struct CodingAgent {
    config: CodingAgentConfig,
    state: Option<CodingAgentState>,
    observer: Option<Arc<dyn EventObserver>>,
    sandbox: Option<vol_llm_core::SandboxRef>,
}

impl CodingAgent {
    /// Create a new CodingAgent from config.
    ///
    /// The caller must provide an LLMClient via `config.llm`.
    /// If `config.working_dir` is not ".", a LocalSandbox is automatically
    /// created and passed to the ReActAgent.
    pub async fn new(config: CodingAgentConfig) -> Result<Self, CodingAgentError> {
        // Get LLM from config — caller constructs this
        let llm = config.llm.clone()
            .ok_or_else(|| CodingAgentError::Config("llm not set: config.llm must be provided by caller".to_string()))?;

        // Create tool registry with coding tools
        let mut tool_registry = ToolRegistry::new();
        Self::register_coding_tools(&mut tool_registry, &config.tool_config);

        // Create agent config - use plugin_registry from config
        let agent_config = AgentConfig {
            max_iterations: config.max_iterations,
            max_history_messages: 20,
            prompt_context: vol_llm_agent::PromptContext::new(
                vol_llm_agent::PromptTemplate::new("coding", "You are an expert coding assistant. Help users understand, modify, and improve their codebase.")
            ),
            verbose: config.verbose,
            plugin_registry: config.plugin_registry.clone(),
            unsafe_mode: config.unsafe_mode,
            agent_id: if config.agent_id.is_empty() { generate_agent_id() } else { config.agent_id.clone() },
            log_base_path: config.log_base_path.clone(),
        };

        // Auto-init sandbox from working_dir if not current directory
        let sandbox: Option<vol_llm_core::SandboxRef> = if config.working_dir != PathBuf::from(".") {
            let sandbox = LocalSandbox::new(Some(config.working_dir.clone()));
            sandbox.start().map_err(|e| CodingAgentError::Config(
                format!("Failed to start sandbox at {:?}: {}", config.working_dir, e)
            ))?;
            Some(Arc::new(sandbox))
        } else {
            None
        };

        Ok(Self {
            config,
            state: Some(CodingAgentState {
                llm,
                tool_registry: Arc::new(tool_registry),
                agent_config,
            }),
            observer: None,
            sandbox,
        })
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
        // Also update the state's agent_config
        if let Some(ref mut state) = self.state {
            state.agent_config.agent_id = self.config.agent_id.clone();
        }
        self
    }

    /// Set the log base path
    pub fn with_log_base_path(mut self, log_base_path: PathBuf) -> Self {
        self.config.log_base_path = log_base_path;
        // Also update the state's agent_config
        if let Some(ref mut state) = self.state {
            state.agent_config.log_base_path = self.config.log_base_path.clone();
        }
        self
    }

    /// Run a coding task
    pub async fn run(&self, task: &str) -> Result<CodingAgentResponse, CodingAgentError> {
        // Get state - take ownership of components
        let state = self.state.as_ref()
            .ok_or_else(|| CodingAgentError::Config("CodingAgent already consumed".to_string()))?;

        // Create session for this run
        use vol_llm_agent::session::{InMemorySessionStore, InMemoryMessageStore};
        let session = Arc::new(Session::new(
            format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryMessageStore::new()),
        ));

        // Create ReActAgent with the plugin_registry that may have been modified by with_observer()
        // Note: We need to use the config's plugin_registry, not the agent_config's
        let agent_config = AgentConfig {
            plugin_registry: self.config.plugin_registry.clone(),
            ..state.agent_config.clone()
        };

        let mut react_agent = ReActAgent::new(
            state.llm.clone(),
            state.tool_registry.clone(),
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
}

/// Builder pattern for CodingAgent
pub struct CodingAgentBuilder {
    config: CodingAgentConfig,
    sandbox: Option<vol_llm_core::SandboxRef>,
}

impl CodingAgentBuilder {
    pub fn new() -> Self {
        Self {
            config: CodingAgentConfig::default(),
            sandbox: None,
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
        self.config.working_dir = path;
        self
    }

    pub fn hitl_enabled(mut self, enabled: bool) -> Self {
        self.config.hitl_enabled = enabled;
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

    pub fn unsafe_mode(mut self, enabled: bool) -> Self {
        self.config.unsafe_mode = enabled;
        self
    }

    pub async fn build(self) -> Result<CodingAgent, CodingAgentError> {
        let mut agent = CodingAgent::new(self.config).await?;
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

/// Generate a short random agent ID
fn generate_agent_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("coding_{:x}", timestamp % 0xFFFFFF)
}
