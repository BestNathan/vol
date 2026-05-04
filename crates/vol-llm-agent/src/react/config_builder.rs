//! Builder for AgentConfig.

use super::agent::AgentConfig;
use super::plugin::PluginRegistry;
use crate::agent_def::AgentDef;
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_skill::{SkillInjector, SkillLoader, SkillTool};
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};
use std::path::PathBuf;
use std::sync::Arc;
use vol_llm_context::ContextContributor;
use vol_llm_core::SandboxRef;
use vol_llm_tool::ExecutableTool;

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
    working_dir: Option<PathBuf>,
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
            working_dir: None,
        }
    }

    pub fn with_def(mut self, def: AgentDef) -> Self {
        self.def = Some(def);
        self
    }

    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
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

    pub fn build(self) -> Result<AgentConfig, AgentConfigBuildError> {
        let llm = self
            .llm
            .ok_or(AgentConfigBuildError::MissingLlm)?;

        // Determine effective working_dir: explicit override > def > None
        let working_dir = self.working_dir
            .or_else(|| self.def.as_ref().and_then(|d| d.working_dir.clone()));

        // Build tool registry: if tool_registry not set, build from individual tools
        let mut tools = match self.tool_registry {
            Some(registry) => {
                Arc::try_unwrap(registry).unwrap_or_else(|arc| (*arc).clone())
            }
            None => {
                let mut registry = ToolRegistry::new();
                for tool in self.tools {
                    registry.register_boxed(tool);
                }
                registry
            }
        };

        // Auto-load skills into the tool registry
        let skill_loader = Arc::new(SkillLoader::new(working_dir.clone()));
        tools.register(SkillTool::new(skill_loader.clone()));

        // Create session if not provided
        let session = self.session.unwrap_or_else(|| {
            Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())))
        });

        // Build context builder, adding SkillInjector
        let context_builder = match self.context_builder {
            Some(cb) => {
                let injector = SkillInjector::new(skill_loader);
                let budget = cb.token_budget();
                let mut b = ContextBuilderBuilder::new(budget.total)
                    .head_size(budget.head_size)
                    .tail_size(budget.tail_size)
                    .add_contributors_from(&cb)
                    .add_contributor(Box::new(injector));
                for c in self.contributors {
                    b = b.add_contributor(c);
                }
                b.build()
            }
            None => {
                let injector = SkillInjector::new(skill_loader);
                let mut b = ContextBuilderBuilder::new(128_000)
                    .add_contributor(Box::new(injector));
                for c in self.contributors {
                    b = b.add_contributor(c);
                }
                b.build()
            }
        };

        Ok(AgentConfig {
            def: self.def,
            llm,
            tools: Arc::new(tools),
            session,
            sandbox: self.sandbox,
            context_builder,
            plugin_registry: self.plugin_registry,
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
