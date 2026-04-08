//! Agent builder.

use std::sync::Arc;
use vol_llm_core::LLMClient;
use vol_llm_tool::{Tool, ToolRegistry};
use super::agent::{AgentConfig, ReActAgent};
use super::plugin::AgentPlugin;
use crate::session::{Session, InMemorySessionStore, InMemoryMessageStore};

/// Agent builder
pub struct AgentBuilder {
    llm: Option<Arc<dyn LLMClient>>,
    tools: Vec<Box<dyn Tool>>,
    config: AgentConfig,
    session: Option<Arc<Session>>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            llm: None,
            tools: Vec::new(),
            config: AgentConfig::default(),
            session: None,
        }
    }

    pub fn with_llm(mut self, llm: Arc<dyn LLMClient>) -> Self {
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

    pub fn with_max_history_messages(mut self, max: usize) -> Self {
        self.config.max_history_messages = max;
        self
    }

    pub fn with_session(mut self, session: Arc<Session>) -> Self {
        self.session = Some(session);
        self
    }

    pub fn with_plugin<P: AgentPlugin + 'static>(mut self, plugin: P) -> Self {
        self.config.plugin_registry.register(plugin);
        self
    }

    pub fn with_plugin_registry(mut self, registry: super::PluginRegistry) -> Self {
        self.config.plugin_registry = registry;
        self
    }

    pub fn build(self) -> Result<ReActAgent, AgentBuilderError> {
        let llm = self.llm.ok_or(AgentBuilderError::MissingLlm)?;

        let mut registry = ToolRegistry::new();
        for tool in self.tools {
            registry.register_boxed(tool);
        }

        // Create session if not provided
        let session = self.session.unwrap_or_else(|| {
            Arc::new(Session::new(
                uuid::Uuid::new_v4().to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            ))
        });

        Ok(ReActAgent::new(llm, Arc::new(registry), self.config, session))
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
