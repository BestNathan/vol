//! Coding Agent configuration.

use std::path::PathBuf;

/// Coding Agent configuration
#[derive(Clone, Debug)]
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
        }
    }
}
