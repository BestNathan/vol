//! Builder for AgentConfig.

use super::agent::AgentConfig;
use super::plugin::PluginRegistry;
use crate::agent_def::AgentDef;
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};
use std::path::PathBuf;
use std::sync::Arc;
use vol_llm_context::ContextContributor;
use vol_llm_core::SandboxRef;
use vol_llm_tool::ExecutableTool;

/// Generate a short random agent ID if not provided.
fn generate_agent_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("agent_{:x}", timestamp % 0xFFFFFF)
}

/// Builder for AgentConfig.
pub struct AgentConfigBuilder {
    def: Option<AgentDef>,
    llm: Option<Arc<dyn vol_llm_core::LLMClient>>,
    tools: Vec<Box<dyn ExecutableTool>>,
    tool_registry: Option<Arc<ToolRegistry>>,
    session: Option<Arc<Session>>,
    sandbox: Option<SandboxRef>,
    context_builder: Option<vol_llm_context::ContextBuilder>,
    plugin_registry: PluginRegistry,
    contributors: Vec<Box<dyn ContextContributor>>,
    agent_id: Option<String>,
    working_dir: Option<PathBuf>,
    max_iterations: u32,
    max_history_messages: usize,
}

impl AgentConfigBuilder {
    pub fn new() -> Self {
        Self {
            def: None,
            llm: None,
            tools: Vec::new(),
            tool_registry: None,
            session: None,
            sandbox: None,
            context_builder: None,
            plugin_registry: PluginRegistry::new(),
            contributors: Vec::new(),
            agent_id: None,
            working_dir: None,
            max_iterations: 5,
            max_history_messages: 20,
        }
    }

    pub fn with_def(mut self, def: AgentDef) -> Self {
        self.def = Some(def);
        self
    }

    pub fn with_llm(mut self, llm: Arc<dyn vol_llm_core::LLMClient>) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn with_tool<T: ExecutableTool + 'static>(mut self, tool: T) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    pub fn with_tools(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    pub fn with_session(mut self, session: Arc<Session>) -> Self {
        self.session = Some(session);
        self
    }

    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    pub fn with_context_builder(mut self, cb: vol_llm_context::ContextBuilder) -> Self {
        self.context_builder = Some(cb);
        self
    }

    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        use vol_llm_context::builtin::SimpleContributor;
        self.contributors.push(Box::new(SimpleContributor::system(prompt)));
        self
    }

    pub fn with_contributor(mut self, contributor: Box<dyn ContextContributor>) -> Self {
        self.contributors.push(contributor);
        self
    }

    pub fn with_plugin<P: super::AgentPlugin + 'static>(mut self, plugin: P) -> Self {
        self.plugin_registry.register(plugin);
        self
    }

    pub fn with_plugin_registry(mut self, registry: PluginRegistry) -> Self {
        self.plugin_registry = registry;
        self
    }

    pub fn with_agent_id(mut self, agent_id: String) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    pub fn with_working_dir(mut self, path: PathBuf) -> Self {
        self.working_dir = Some(path);
        self
    }

    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_max_history_messages(mut self, max: usize) -> Self {
        self.max_history_messages = max;
        self
    }

    pub fn build(mut self) -> Result<AgentConfig, AgentConfigBuildError> {
        let llm = self
            .llm
            .ok_or(AgentConfigBuildError::MissingLlm)?;

        // Build tool registry: if tool_registry not set, build from individual tools
        let tools = match self.tool_registry {
            Some(registry) => registry,
            None => {
                let mut registry = ToolRegistry::new();
                for tool in self.tools {
                    registry.register_boxed(tool);
                }
                Arc::new(registry)
            }
        };

        // Create session if not provided
        let session = self.session.unwrap_or_else(|| {
            Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())))
        });

        // Build context builder
        let context_builder = match self.context_builder {
            Some(cb) => {
                if self.contributors.is_empty() {
                    cb
                } else {
                    let budget = cb.token_budget();
                    let mut b = ContextBuilderBuilder::new(budget.total)
                        .head_size(budget.head_size)
                        .tail_size(budget.tail_size);
                    for c in self.contributors {
                        b = b.add_contributor(c);
                    }
                    b.build()
                }
            }
            None => {
                let mut b = ContextBuilderBuilder::new(128_000);
                for c in self.contributors {
                    b = b.add_contributor(c);
                }
                b.build()
            }
        };

        Ok(AgentConfig {
            def: self.def,
            llm,
            tools,
            session,
            sandbox: self.sandbox,
            context_builder,
            plugin_registry: self.plugin_registry,
            max_iterations: self.max_iterations,
            max_history_messages: self.max_history_messages,
            agent_id: self.agent_id.unwrap_or_else(generate_agent_id),
            working_dir: self.working_dir.unwrap_or_else(|| PathBuf::from(".")),
        })
    }
}

impl Default for AgentConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder error for AgentConfig.
#[derive(Debug, thiserror::Error)]
pub enum AgentConfigBuildError {
    #[error("LLM client is required")]
    MissingLlm,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{
        ConversationRequest, ConversationResponse, LLMClient, LLMProvider, StreamReceiver,
        SupportedParam,
    };

    struct MockLlm;
    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> LLMProvider {
            LLMProvider::Anthropic
        }
        fn model(&self) -> &str {
            "mock"
        }
        fn supported_params(&self) -> &[SupportedParam] {
            &[]
        }
        async fn converse(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<ConversationResponse> {
            unimplemented!()
        }
        async fn converse_stream(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<StreamReceiver> {
            let (_tx, rx) = tokio::sync::mpsc::channel(10);
            Ok(StreamReceiver::new(rx))
        }
    }

    #[tokio::test]
    async fn test_builder_minimal() {
        let result = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .build();
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.def.is_none());
        assert_eq!(config.max_iterations, 5);
        assert_eq!(config.max_history_messages, 20);
        assert_eq!(config.working_dir, PathBuf::from("."));
    }

    #[tokio::test]
    async fn test_builder_missing_llm() {
        let result = AgentConfigBuilder::new().build();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_builder_with_def() {
        let def = AgentDef::new("test", "prompt");
        let config = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .with_def(def.clone())
            .build()
            .unwrap();
        assert!(config.def.is_some());
        assert_eq!(config.def.as_ref().unwrap().name, "test");
    }

    #[tokio::test]
    async fn test_builder_with_custom_values() {
        let config = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .with_agent_id("custom-id".to_string())
            .with_working_dir(PathBuf::from("/tmp/test"))
            .with_max_iterations(10)
            .with_max_history_messages(50)
            .build()
            .unwrap();
        assert_eq!(config.agent_id, "custom-id");
        assert_eq!(config.working_dir, PathBuf::from("/tmp/test"));
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.max_history_messages, 50);
    }

    #[tokio::test]
    async fn test_builder_with_system_prompt() {
        let config = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .with_system_prompt("You are a helpful assistant.".to_string())
            .build()
            .unwrap();
        let names = config.context_builder.contributor_names();
        assert!(names.contains(&"system"));
    }
}
