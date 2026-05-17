//! Agent builder.

use super::agent::{AgentConfig, ReActAgent};
use super::plugin::AgentPlugin;
use vol_session::{InMemoryEntryStore, Session};
use std::sync::Arc;
use vol_llm_context::{ContextBuilderBuilder, ContextContributor};
use vol_llm_core::LLMClient;
use vol_llm_tool::{ExecutableTool, ToolRegistry};

/// Agent builder
pub struct AgentBuilder {
    llm: Option<Arc<dyn LLMClient>>,
    tools: Vec<Box<dyn ExecutableTool>>,
    config: AgentConfig,
    session: Option<Arc<Session>>,
    contributors: Vec<Box<dyn ContextContributor>>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            llm: None,
            tools: Vec::new(),
            config: AgentConfig::default(),
            session: None,
            contributors: Vec::new(),
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
        self.config.max_iterations = max;
        self
    }

    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        use vol_llm_context::builtin::SimpleContributor;

        self.contributors.push(Box::new(SimpleContributor::system(prompt)));
        self
    }

    pub fn with_max_history_messages(mut self, max: usize) -> Self {
        self.config.max_history_messages = max;
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
        self.config.plugin_registry.register(plugin);
        self
    }

    pub fn with_plugin_registry(mut self, registry: super::PluginRegistry) -> Self {
        self.config.plugin_registry = registry;
        self
    }

    pub fn with_agent_id(mut self, agent_id: String) -> Self {
        self.config.agent_id = agent_id;
        self
    }

    pub fn with_working_dir(mut self, path: std::path::PathBuf) -> Self {
        self.config.working_dir = path;
        self
    }

    pub fn build(mut self) -> Result<ReActAgent, AgentBuilderError> {
        let llm = self.llm.ok_or(AgentBuilderError::MissingLlm)?;

        let mut registry = ToolRegistry::new();
        for tool in self.tools {
            registry.register_boxed(tool);
        }

        // Create session if not provided
        let session = self.session.unwrap_or_else(|| {
            let entry_store = Arc::new(InMemoryEntryStore::new());
            Arc::new(Session::new(entry_store))
        });

        // Build ContextBuilder: start from config defaults, add stored contributors
        let token_budget = self.config.context_builder.token_budget().total;
        let head_size = self.config.context_builder.token_budget().head_size;
        let tail_size = self.config.context_builder.token_budget().tail_size;

        let mut builder = ContextBuilderBuilder::new(token_budget)
            .head_size(head_size)
            .tail_size(tail_size);

        for contributor in self.contributors {
            builder = builder.add_contributor(contributor);
        }

        self.config.context_builder = builder.build();

        Ok(ReActAgent::new(
            llm,
            Arc::new(registry),
            self.config,
            session,
        ))
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
