//! CodingAgent - AI-powered code assistant.

use std::sync::Arc;
use std::path::PathBuf;
use vol_llm_tool::ToolRegistry;
use vol_llm_agent::{ReActAgent, AgentConfig, Session};
use vol_llm_agent::react::PluginRegistry;
use vol_llm_provider::{LLMProviderConfig, LLMProviderRegistry};

use crate::coding::config::CodingAgentConfig;
use crate::coding::error::CodingAgentError;
use crate::coding::observer::EventObserver;

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
    react_agent: ReActAgent,
    observer: Option<Arc<dyn EventObserver>>,
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

        // Create agent config
        let agent_config = AgentConfig {
            max_iterations: config.max_iterations,
            max_history_messages: 20,
            prompt_context: vol_llm_agent::PromptContext::new(
                vol_llm_agent::PromptTemplate::new("coding", "You are an expert coding assistant. Help users understand, modify, and improve their codebase.")
            ),
            verbose: config.verbose,
            plugin_registry: PluginRegistry::new(),
            agent_id: generate_agent_id(),
            log_base_path: PathBuf::from("logs/coding"),
        };

        // Create session
        use vol_llm_agent::session::{InMemorySessionStore, InMemoryMessageStore};
        let session = Arc::new(Session::new(
            format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryMessageStore::new()),
        ));

        // Create ReActAgent
        let react_agent = ReActAgent::new(
            llm,
            Arc::new(tool_registry),
            agent_config,
            session,
        );

        Ok(Self {
            config,
            react_agent,
            observer: None,
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

    /// Set the event observer
    pub fn with_observer(mut self, observer: Arc<dyn EventObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// Run a coding task
    pub async fn run(&self, task: &str) -> Result<CodingAgentResponse, CodingAgentError> {
        // TODO: Implement full run logic with observer integration
        // For MVP, just return a placeholder response
        Ok(CodingAgentResponse {
            success: true,
            summary: format!("Task completed: {}", task),
            iterations: 0,
            tool_calls: 0,
        })
    }
}

/// Builder pattern for CodingAgent
pub struct CodingAgentBuilder {
    config: CodingAgentConfig,
}

impl CodingAgentBuilder {
    pub fn new() -> Self {
        Self {
            config: CodingAgentConfig::default(),
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

    pub async fn build(self) -> Result<CodingAgent, CodingAgentError> {
        CodingAgent::new(self.config).await
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
