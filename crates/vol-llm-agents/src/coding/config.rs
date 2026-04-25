//! Coding Agent configuration.

use std::path::PathBuf;
use std::sync::Arc;
use vol_llm_agent::react::{BoxedApprovalHandler, PluginRegistry};
use vol_llm_tool::ToolConfig;

/// Coding Agent configuration
#[derive(Clone)]
pub struct CodingAgentConfig {
    /// Agent identifier
    pub agent_id: String,

    /// LLM client for generating responses.
    /// Caller constructs this; CodingAgent does not read env vars.
    /// If None, the LLM is created from llm_provider_id using ANTHROPIC_AUTH_TOKEN.
    pub llm: Option<Arc<dyn vol_llm_core::LLMClient>>,

    /// LLM provider ID for env-based LLM creation (used when llm is None).
    pub llm_provider_id: String,

    /// Maximum reasoning iterations
    pub max_iterations: u32,

    /// Working directory for code operations
    pub working_dir: PathBuf,

    /// Base path for logs
    pub log_base_path: PathBuf,

    /// Enable HITL confirmation for dangerous operations
    pub hitl_enabled: bool,

    /// Skip HITL approval (auto-approve all tool calls)
    pub unsafe_mode: bool,

    /// HTML report output path (None = no report)
    pub html_report_path: Option<PathBuf>,

    /// Plugin registry for extending agent functionality
    pub plugin_registry: PluginRegistry,

    /// Tool configuration container (for web tools, etc.)
    pub tool_config: ToolConfig,

    /// Shared session for conversation history across runs.
    /// If provided, CodingAgent::run() reuses it instead of creating a new InMemory session.
    pub session: Option<Arc<vol_session::Session>>,

    /// Custom approval handler for TUI/HTTP-based approval flows.
    pub approval_handler: Option<BoxedApprovalHandler>,
}

impl std::fmt::Debug for CodingAgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingAgentConfig")
            .field("agent_id", &self.agent_id)
            .field("llm", &"<LLMClient>")
            .field("llm_provider_id", &self.llm_provider_id)
            .field("max_iterations", &self.max_iterations)
            .field("working_dir", &self.working_dir)
            .field("log_base_path", &self.log_base_path)
            .field("hitl_enabled", &self.hitl_enabled)
            .field("unsafe_mode", &self.unsafe_mode)
            .field("html_report_path", &self.html_report_path)
            .field("plugin_registry", &"<PluginRegistry>")
            .field("tool_config", &self.tool_config)
            .field("session", &"<Session>")
            .field("approval_handler", &"<ApprovalHandler>")
            .finish()
    }
}

impl Default for CodingAgentConfig {
    fn default() -> Self {
        Self {
            agent_id: "coding-agent".to_string(),
            llm_provider_id: "anthropic-main".to_string(),
            max_iterations: 10,
            working_dir: PathBuf::from("."),
            log_base_path: PathBuf::from("logs"),
            hitl_enabled: true,
            unsafe_mode: false,
            html_report_path: None,
            llm: None,
            plugin_registry: PluginRegistry::new(),
            tool_config: ToolConfig::new(),
            session: None,
            approval_handler: None,
        }
    }
}
