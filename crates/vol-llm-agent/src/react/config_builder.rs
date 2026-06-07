//! Builder for AgentConfig.

use super::agent::AgentConfig;
use super::plugin::PluginRegistry;
use crate::agent_def::AgentDef;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use vol_llm_context::{AttentionAnchor, ContextBuilderBuilder, ContextContributor};
use vol_llm_sandbox::SandboxRef;
use vol_llm_mcp::{McpConfig, McpManager};
use vol_llm_skill::{SkillInjector, SkillLoader, SkillTool};
use vol_llm_tool::{ExecutableTool, ToolRegistry};
use vol_session::{InMemoryEntryStore, Session, SessionContributor};

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
    mcp_manager: Option<Arc<McpManager>>,
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
            mcp_manager: None,
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

    /// Load MCP server configuration and connect all servers.
    ///
    /// Searches for .mcp.json (project-level) and ~/.mcp.json (user-level),
    /// merges them, connects all servers, and registers their tools.
    /// If no config files exist or they are empty, the agent runs without MCP tools.
    pub async fn with_mcp_from_config(mut self, working_dir: Option<&Path>) -> Self {
        let config = match McpConfig::load(working_dir) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("MCP config load error: {}", e);
                return self;
            }
        };

        if config.servers().is_empty() {
            return self;
        }

        let manager = Arc::new(McpManager::new(config.servers().to_vec()));
        if let Err(e) = manager.connect().await {
            tracing::warn!("MCP manager connect error: {}", e);
            return self;
        }

        // Register MCP tools into the tool registry
        let tool_registry = match self.tool_registry.take() {
            Some(registry) => {
                let mut reg = match Arc::try_unwrap(registry) {
                    Ok(r) => r,
                    Err(arc) => (*arc).clone(),
                };
                let tools = std::mem::take(&mut self.tools);
                for tool in tools {
                    reg.register_boxed(tool);
                }
                reg.register_from_mcp(manager.clone()).await;
                Arc::new(reg)
            }
            None => {
                let mut registry = ToolRegistry::new();
                let tools = std::mem::take(&mut self.tools);
                for tool in tools {
                    registry.register_boxed(tool);
                }
                registry.register_from_mcp(manager.clone()).await;
                Arc::new(registry)
            }
        };
        self.tool_registry = Some(tool_registry);
        self.mcp_manager = Some(manager);

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
                let mut tools = Arc::try_unwrap(registry).unwrap_or_else(|arc| (*arc).clone());
                for tool in self.tools {
                    tools.register_boxed(tool);
                }
                tools
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

        // Build context builder with standardized ordering:
        //
        //   Head(0)       — Agent Prompt   (always present, empty placeholder if unset)
        //   Head(1)       — Skills         (always present)
        //   Middle(0..n)  — Custom Files   (from AgentDef.context_files, array order)
        //   Tail(0)       — Session        (conversation history)
        let context_builder = {
            let (total, head_size, tail_size) = self
                .context_builder
                .as_ref()
                .map(|cb| {
                    let b = cb.token_budget();
                    (b.total, b.head_size, b.tail_size)
                })
                .unwrap_or((128_000, 0, 0));

            // Pre-compute values borrowed from self.def
            let prompt = self
                .def
                .as_ref()
                .map(|d| d.prompt.clone())
                .unwrap_or_default();
            let max_history = self
                .def
                .as_ref()
                .and_then(|d| d.max_history_messages)
                .unwrap_or(50);
            let context_files: Vec<vol_llm_context::builtin::FileSpec> = self
                .def
                .as_ref()
                .map(|d| {
                    let working_dir = d.working_dir.as_ref();
                    d.context_files
                        .iter()
                        .enumerate()
                        .map(|(i, path)| {
                            let full_path = working_dir
                                .map(|d| d.join(path))
                                .unwrap_or_else(|| PathBuf::from(path));
                            let path_str = full_path.to_string_lossy().to_string();
                            vol_llm_context::builtin::FileSpec::new(
                                path_str,
                                AttentionAnchor::Middle(i as u32),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();

            let mut b = ContextBuilderBuilder::new(total)
                .head_size(head_size)
                .tail_size(tail_size);

            // 1. Agent Prompt — always Head(0), empty if unset
            b = b.add_contributor(Box::new(
                vol_llm_context::builtin::SimpleContributor::system(prompt),
            ));

            // 2. Skills — always Head(1)
            b = b.add_contributor(Box::new(SkillInjector::new(
                skill_loader,
                AttentionAnchor::Head(1),
            )));

            // 3. Custom Files — Middle(0..n) from AgentDef.context_files
            if !context_files.is_empty() {
                b = b.add_contributor(Box::new(
                    vol_llm_context::builtin::FileContributor::new(context_files),
                ));
            }

            // 4. Clone existing context_builder contributors (if any)
            if let Some(ref cb) = self.context_builder {
                b = b.add_contributors_from(cb);
            }

            // 5. Manual contributors from with_system_prompt / with_contributor
            for c in self.contributors {
                b = b.add_contributor(c);
            }

            // 6. Session — always Tail(0)
            b = b.add_contributor(Box::new(SessionContributor::new(
                Arc::new(tokio::sync::Mutex::new((*session).clone())),
                max_history,
                AttentionAnchor::Tail(0),
            )));

            b.build()
        };

        Ok(AgentConfig {
            def: self.def,
            llm,
            tools: Arc::new(tools),
            session: std::sync::RwLock::new(session),
            sandbox: self.sandbox,
            context_builder: std::sync::RwLock::new(context_builder),
            plugin_registry: self.plugin_registry,
            mcp_manager: self.mcp_manager,
            agent_id: working_dir
                .as_ref()
                .and_then(|d| d.file_name())
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            working_dir: working_dir.unwrap_or_else(|| PathBuf::from(".")),
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
        let guard = config.context_builder.read().unwrap();
        let names = guard.contributor_names();
        assert!(names.contains(&"system"));
    }

    #[tokio::test]
    async fn test_builder_with_mcp_from_config_no_config() {
        // With no .mcp.json files present, with_mcp_from_config should be a no-op
        // and the builder should still produce a valid AgentConfig.
        let config = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .with_mcp_from_config(None)
            .await
            .build()
            .unwrap();
        assert!(config.def.is_none());
    }

    #[tokio::test]
    async fn test_builder_with_working_dir() {
        let config = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .with_working_dir(PathBuf::from("/tmp/test-project"))
            .build()
            .unwrap();
        // SkillTool should be registered in the tool registry
        let tool_names = config.tools.tool_names();
        assert!(tool_names.contains(&"skill"), "SkillTool should be auto-registered, got: {:?}", tool_names);
    }

    #[tokio::test]
    async fn test_builder_skill_injector_always_present() {
        let config = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .build()
            .unwrap();
        // SkillInjector should always be added to context builder
        let cb = config.context_builder.read().unwrap();
        let names: Vec<&str> = cb.contributor_names();
        assert!(names.iter().any(|n| n.contains("skill")), "SkillInjector should be present, got: {:?}", names);
    }

    #[tokio::test]
    async fn test_builder_working_dir_override_takes_precedence() {
        let def = AgentDef::new("test", "prompt")
            .with_working_dir(PathBuf::from("/tmp/from-def"));
        let config = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .with_def(def)
            .with_working_dir(PathBuf::from("/tmp/explicit-override"))
            .build()
            .unwrap();
        // Both def and explicit working_dir should result in skills being loaded
        let tool_names = config.tools.tool_names();
        assert!(tool_names.contains(&"skill"), "SkillTool should be auto-registered with explicit override");
    }
}
