//! Agent builder (deprecated — use AgentConfig::builder() instead).

use super::agent::{AgentConfig, ReActAgent};
use super::plugin::AgentPlugin;
use vol_session::{InMemoryEntryStore, Session};
use std::sync::Arc;
use vol_llm_context::{ContextBuilderBuilder, ContextContributor};
use vol_llm_core::LLMClient;
use vol_llm_tool::{ExecutableTool, ToolRegistry};

/// Agent builder (deprecated — use AgentConfig::builder() instead).
pub struct AgentBuilder {
    llm: Option<Arc<dyn LLMClient>>,
    tools: Vec<Box<dyn ExecutableTool>>,
    max_iterations: u32,
    max_history_messages: usize,
    agent_id: Option<String>,
    working_dir: Option<std::path::PathBuf>,
    session: Option<Arc<Session>>,
    contributors: Vec<Box<dyn ContextContributor>>,
    plugin_registry: super::PluginRegistry,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            llm: None,
            tools: Vec::new(),
            max_iterations: 5,
            max_history_messages: 20,
            agent_id: None,
            working_dir: None,
            session: None,
            contributors: Vec::new(),
            plugin_registry: super::PluginRegistry::new(),
        }
    }

    pub fn with_llm(mut self, llm: Arc<dyn LLMClient>) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn with_tool<T: ExecutableTool + 'static>(mut self, tool: T) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        use vol_llm_context::builtin::SimpleContributor;
        self.contributors.push(Box::new(SimpleContributor::system(prompt)));
        self
    }

    pub fn with_max_history_messages(mut self, max: usize) -> Self {
        self.max_history_messages = max;
        self
    }

    pub fn with_contributor(mut self, contributor: Box<dyn ContextContributor>) -> Self {
        self.contributors.push(contributor);
        self
    }

    pub fn with_session(mut self, session: Arc<Session>) -> Self {
        self.session = Some(session);
        self
    }

    pub fn with_plugin<P: AgentPlugin + 'static>(mut self, plugin: P) -> Self {
        self.plugin_registry.register(plugin);
        self
    }

    pub fn with_plugin_registry(mut self, registry: super::PluginRegistry) -> Self {
        self.plugin_registry = registry;
        self
    }

    pub fn with_agent_id(mut self, agent_id: String) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    pub fn with_working_dir(mut self, path: std::path::PathBuf) -> Self {
        self.working_dir = Some(path);
        self
    }

    pub fn build(mut self) -> Result<ReActAgent, AgentBuilderError> {
        let llm = self.llm.ok_or(AgentBuilderError::MissingLlm)?;

        let mut registry = ToolRegistry::new();
        for tool in self.tools {
            registry.register_boxed(tool);
        }

        let session = self.session.unwrap_or_else(|| {
            let entry_store = Arc::new(InMemoryEntryStore::new());
            Arc::new(Session::new(entry_store))
        });

        let token_budget = 128_000;
        let mut builder = ContextBuilderBuilder::new(token_budget);
        for contributor in self.contributors {
            builder = builder.add_contributor(contributor);
        }

        let mut config = AgentConfig::new(llm, Arc::new(registry), session);
        config.context_builder = builder.build();
        config.max_iterations = self.max_iterations;
        config.max_history_messages = self.max_history_messages;
        config.plugin_registry = self.plugin_registry;
        if let Some(agent_id) = self.agent_id {
            config.agent_id = agent_id;
        }
        if let Some(working_dir) = self.working_dir {
            config.working_dir = working_dir;
        }

        Ok(ReActAgent::new(config))
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
