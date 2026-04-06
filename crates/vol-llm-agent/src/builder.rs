//! Agent builder.

use vol_llm_core::LLMClient;
use vol_llm_tool::{Tool, ToolRegistry};
use crate::agent::{AgentConfig, ReActAgent};

/// Agent builder
pub struct AgentBuilder {
    llm: Option<Box<dyn LLMClient>>,
    tools: Vec<Box<dyn Tool>>,
    config: AgentConfig,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            llm: None,
            tools: Vec::new(),
            config: AgentConfig::default(),
        }
    }

    pub fn with_llm(mut self, llm: Box<dyn LLMClient>) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn with_tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.config.max_iterations = max;
        self
    }

    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.config.system_prompt = prompt;
        self
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }

    pub fn build(self) -> Result<ReActAgent, AgentBuilderError> {
        let llm = self.llm.ok_or(AgentBuilderError::MissingLlm)?;

        let mut registry = ToolRegistry::new();
        for tool in self.tools {
            registry.register_boxed(tool);
        }

        Ok(ReActAgent::new(llm, registry, self.config))
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder error
#[derive(Debug, thiserror::Error)]
pub enum AgentBuilderError {
    #[error("LLM client is required")]
    MissingLlm,
}
