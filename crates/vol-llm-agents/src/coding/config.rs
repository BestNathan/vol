//! Coding Agent configuration.

use std::path::PathBuf;
use vol_llm_agent::react::PluginRegistry;

/// Coding Agent configuration
#[derive(Clone)]
pub struct CodingAgentConfig {
    /// Maximum reasoning iterations
    pub max_iterations: u32,

    /// Working directory for code operations
    pub working_dir: PathBuf,

    /// Enable HITL confirmation for dangerous operations
    pub hitl_enabled: bool,

    /// Verbose output
    pub verbose: bool,

    /// HTML report output path (None = no report)
    pub html_report_path: Option<PathBuf>,

    /// LLM provider ID
    pub llm_provider_id: String,

    /// Plugin registry for extending agent functionality
    pub plugin_registry: PluginRegistry,
}

impl std::fmt::Debug for CodingAgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingAgentConfig")
            .field("max_iterations", &self.max_iterations)
            .field("working_dir", &self.working_dir)
            .field("hitl_enabled", &self.hitl_enabled)
            .field("verbose", &self.verbose)
            .field("html_report_path", &self.html_report_path)
            .field("llm_provider_id", &self.llm_provider_id)
            .field("plugin_registry", &"<PluginRegistry>")
            .finish()
    }
}

impl Default for CodingAgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            working_dir: PathBuf::from("."),
            hitl_enabled: true,
            verbose: false,
            html_report_path: None,
            llm_provider_id: "anthropic-main".to_string(),
            plugin_registry: PluginRegistry::new(),
        }
    }
}
