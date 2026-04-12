//! CodingAgent - AI-powered code assistant.

use std::sync::Arc;
use std::path::PathBuf;
use vol_llm_tool::ToolRegistry;
use vol_llm_agent::{ReActAgent, AgentConfig, Session};
use vol_llm_provider::{LLMProviderConfig, LLMProviderRegistry};

use crate::coding::config::CodingAgentConfig;
use crate::coding::error::CodingAgentError;
use crate::coding::observer::EventObserver;
use crate::coding::observer_plugin::ObserverPlugin;

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
    /// Create a new CodingAgent from config
    pub async fn new(config: CodingAgentConfig) -> Result<Self, CodingAgentError> {
        // Initialize LLM
        let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
            .map_err(|_| CodingAgentError::Config("ANTHROPIC_AUTH_TOKEN not set".to_string()))?;

        let llm_config = LLMProviderConfig {
            id: config.llm_provider_id.clone(),
            config: vol_llm_provider::LLMConfig {
                provider: vol_llm_core::LLMProvider::Anthropic,
                model: "qwen3.5-plus".to_string(),
                api_key: vol_llm_provider::Secret::literal(api_key),
                base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
            },
        };

        let registry = LLMProviderRegistry::from_configs(&[llm_config])
            .map_err(|e| CodingAgentError::Config(format!("Failed to initialize LLM: {}", e)))?;

        let llm = registry.get(&config.llm_provider_id)
            .ok_or_else(|| CodingAgentError::Config(format!("LLM provider '{}' not found", config.llm_provider_id)))?
            .clone();

        // Create tool registry with coding tools
        let mut tool_registry = ToolRegistry::new();
        Self::register_coding_tools(&mut tool_registry);

        // Create agent config - use plugin_registry from config
        let agent_config = AgentConfig {
            max_iterations: config.max_iterations,
            max_history_messages: 20,
            prompt_context: vol_llm_agent::PromptContext::new(
                vol_llm_agent::PromptTemplate::new("coding", "You are an expert coding assistant. Help users understand, modify, and improve their codebase.")
            ),
            verbose: config.verbose,
            plugin_registry: config.plugin_registry.clone(),
            agent_id: generate_agent_id(),
            log_base_path: PathBuf::from("logs/coding"),
        };

        Ok(Self {
            config,
            state: Some(CodingAgentState {
                llm,
                tool_registry: Arc::new(tool_registry),
                agent_config,
            }),
            observer: None,
            sandbox: None,
        })
    }

    /// Register coding tools to the tool registry
    fn register_coding_tools(registry: &mut ToolRegistry) {
        use vol_llm_tools_builtin::read_tool::ReadTool;
        use vol_llm_tools_builtin::edit_tool::EditTool;
        use vol_llm_tools_builtin::bash_tool::BashTool;

        registry.register(ReadTool::new());
        registry.register(EditTool::new());
        registry.register(BashTool::new());
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

    /// Set the sandbox for tool execution
    pub fn with_sandbox(mut self, sandbox: vol_llm_core::SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
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

    pub async fn build(self) -> Result<CodingAgent, CodingAgentError> {
        let mut agent = CodingAgent::new(self.config).await?;
        agent.sandbox = self.sandbox;
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
