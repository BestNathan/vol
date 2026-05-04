//! Build ReActAgent from YAML configuration.

use std::path::Path;
use std::sync::Arc;
use vol_llm_agent::ReActAgent;
use vol_llm_agent::react::AgentConfig;
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_provider::LLMProviderRegistry;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

use crate::config::YamlAgentConfig;
use crate::error::YamlAgentError;
use crate::tools::register_tools_by_name;
use crate::plugins::register_plugins_by_name;

/// Builder that creates a ReActAgent from YAML config.
pub struct YamlAgentBuilder {
    config: YamlAgentConfig,
    llm_registry: LLMProviderRegistry,
}

impl YamlAgentBuilder {
    /// Load YAML from a file path.
    pub fn from_file(path: &Path) -> Result<Self, YamlAgentError> {
        let yaml = std::fs::read_to_string(path)
            .map_err(YamlAgentError::Io)?;
        Self::from_yaml(&yaml)
    }

    /// Load YAML from a string.
    pub fn from_yaml(yaml: &str) -> Result<Self, YamlAgentError> {
        let config: YamlAgentConfig = serde_yaml::from_str(yaml)?;
        let llm_registry = LLMProviderRegistry::new();
        Ok(Self { config, llm_registry })
    }

    /// Get a reference to the parsed config.
    pub fn config(&self) -> &YamlAgentConfig {
        &self.config
    }

    /// Set the LLM provider registry.
    ///
    /// Must be called before `build()` if the YAML references an LLM provider.
    pub fn with_llm_registry(mut self, registry: LLMProviderRegistry) -> Self {
        self.llm_registry = registry;
        self
    }

    /// Build the ReActAgent.
    pub fn build(self) -> Result<ReActAgent, YamlAgentError> {
        // 1. Resolve LLM
        let llm = self.llm_registry.get(&self.config.llm)
            .ok_or_else(|| YamlAgentError::LlmNotFound(self.config.llm.clone()))?;

        // 2. Register tools
        let mut tool_registry = ToolRegistry::new();
        register_tools_by_name(&mut tool_registry, &self.config.tools)?;

        // 3. Build system prompt: inline + files
        let system_prompt = self.build_system_prompt();

        // 4. Build context
        let mut context_builder_builder = ContextBuilderBuilder::new(128_000);
        if !system_prompt.is_empty() {
            context_builder_builder = context_builder_builder.add_contributor(Box::new(
                vol_llm_context::builtin::SimpleContributor::system(system_prompt),
            ));
        }
        let context_builder = context_builder_builder.build();

        // 5. Build plugins
        let mut plugin_registry = vol_llm_agent::react::PluginRegistry::new();
        register_plugins_by_name(
            &mut plugin_registry,
            self.config.plugins.as_ref().unwrap_or(&vec![]),
            &self.config.working_dir,
        )?;

        // 6. Create session
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Arc::new(Session::new(entry_store));

        let agent_config = AgentConfig::builder()
            .with_llm(llm)
            .with_tools(Arc::new(tool_registry))
            .with_session(session)
            .with_context_builder(context_builder)
            .with_plugin_registry(plugin_registry)
            .with_agent_id(self.config.name.clone())
            .with_working_dir(self.config.working_dir.clone())
            .with_max_iterations(self.config.max_iterations)
            .with_max_history_messages(self.config.max_history_messages)
            .build()
            .map_err(|e| YamlAgentError::Config(format!("Failed to build agent config: {}", e)))?;

        Ok(ReActAgent::new(agent_config))
    }

    /// Build combined system prompt: inline string + file contents.
    fn build_system_prompt(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref inline) = self.config.system {
            parts.push(inline.clone());
        }

        if let Some(ref files) = self.config.system_files {
            for path in files {
                match std::fs::read_to_string(path) {
                    Ok(content) => parts.push(content),
                    Err(e) => {
                        tracing::warn!(path = path.as_str(), error = %e, "Failed to load system file, skipping");
                    }
                }
            }
        }

        parts.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_yaml_valid() {
        let yaml = r#"
name: test-agent
llm: test-provider
tools: [read, write]
"#;
        let builder = YamlAgentBuilder::from_yaml(yaml).unwrap();
        assert_eq!(builder.config.name, "test-agent");
        assert_eq!(builder.config.tools, vec!["read", "write"]);
    }

    #[test]
    fn test_from_yaml_invalid() {
        let yaml = "not: valid: yaml: [";
        let result = YamlAgentBuilder::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_system_prompt_inline_only() {
        let yaml = r#"
name: test
llm: p
system: "Hello world"
"#;
        let builder = YamlAgentBuilder::from_yaml(yaml).unwrap();
        let prompt = builder.build_system_prompt();
        assert_eq!(prompt, "Hello world");
    }

    #[test]
    fn test_build_system_prompt_with_missing_files() {
        let yaml = r#"
name: test
llm: p
system: "Base"
system_files:
  - /nonexistent/file.md
"#;
        let builder = YamlAgentBuilder::from_yaml(yaml).unwrap();
        let prompt = builder.build_system_prompt();
        // Should only contain the inline part, file is skipped with warning
        assert_eq!(prompt, "Base");
    }

    #[test]
    fn test_build_system_prompt_with_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("instructions.md");
        std::fs::write(&file_path, "File content here.").unwrap();

        let yaml = format!(r#"
name: test
llm: p
system: "Inline prefix"
system_files:
  - {}
"#, file_path.display());
        let builder = YamlAgentBuilder::from_yaml(&yaml).unwrap();
        let prompt = builder.build_system_prompt();
        assert!(prompt.contains("Inline prefix"));
        assert!(prompt.contains("File content here."));
    }
}
