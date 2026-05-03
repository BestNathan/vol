//! WikiAgent configuration.

use std::path::PathBuf;
use std::sync::Arc;

/// WikiAgent configuration.
#[derive(Clone)]
pub struct WikiAgentConfig {
    /// Agent identifier
    pub agent_id: String,

    /// LLM client for generating responses.
    /// If None, the LLM is created from `ANTHROPIC_AUTH_TOKEN`.
    pub llm: Option<Arc<dyn vol_llm_core::LLMClient>>,

    /// LLM provider ID for env-based LLM creation (used when llm is None).
    pub llm_provider_id: String,

    /// Maximum reasoning iterations
    pub max_iterations: u32,

    /// Working directory for wiki file operations.
    /// Wiki pages are stored in `{working_dir}/.agents/wikis/`.
    pub working_dir: PathBuf,
}

impl std::fmt::Debug for WikiAgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WikiAgentConfig")
            .field("agent_id", &self.agent_id)
            .field("llm", &"<LLMClient>")
            .field("llm_provider_id", &self.llm_provider_id)
            .field("max_iterations", &self.max_iterations)
            .field("working_dir", &self.working_dir)
            .finish()
    }
}

impl Default for WikiAgentConfig {
    fn default() -> Self {
        Self {
            agent_id: "wiki-agent".to_string(),
            llm_provider_id: "anthropic-main".to_string(),
            max_iterations: 15,
            working_dir: PathBuf::from("."),
            llm: None,
        }
    }
}
