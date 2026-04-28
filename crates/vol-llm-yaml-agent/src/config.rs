//! YAML agent configuration.

use std::path::PathBuf;
use serde::Deserialize;

/// Parsed YAML agent configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct YamlAgentConfig {
    /// Agent identifier
    pub name: String,

    /// LLM provider ID to use
    pub llm: String,

    /// Maximum reasoning iterations (default: 10)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    /// Maximum history messages to keep (default: 20)
    #[serde(default = "default_max_history")]
    pub max_history_messages: usize,

    /// Inline system prompt
    #[serde(default)]
    pub system: Option<String>,

    /// File paths to load as system prompt (content appended after inline system)
    #[serde(default)]
    pub system_files: Option<Vec<String>>,

    /// Tool names to register
    #[serde(default)]
    pub tools: Vec<String>,

    /// Per-tool parameter configs (keyed by tool name)
    #[serde(default)]
    pub tool_configs: Option<serde_yaml::Value>,

    /// Plugin names to register
    #[serde(default)]
    pub plugins: Option<Vec<String>>,

    /// Working directory (default: ".")
    #[serde(default = "default_working_dir")]
    pub working_dir: PathBuf,
}

fn default_max_iterations() -> u32 { 10 }
fn default_max_history() -> usize { 20 }
fn default_working_dir() -> PathBuf { PathBuf::from(".") }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let yaml = r#"
name: test
llm: anthropic-main
tools: [read, write]
"#;
        let config: YamlAgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "test");
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.max_history_messages, 20);
        assert_eq!(config.system, None);
        assert_eq!(config.working_dir, PathBuf::from("."));
    }

    #[test]
    fn test_parse_full_config() {
        let yaml = r#"
name: coding
llm: anthropic-main
max_iterations: 20
max_history_messages: 30
system: "You are a coding assistant."
system_files:
  - .agent/AGENT.md
  - .agent/INSTRUCTION.md
tools:
  - read
  - write
  - edit
  - bash
tool_configs:
  web_search:
    provider: tavily
plugins:
  - logger
working_dir: "/tmp/project"
"#;
        let config: YamlAgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "coding");
        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.max_history_messages, 30);
        assert_eq!(config.system.as_deref(), Some("You are a coding assistant."));
        assert_eq!(config.system_files.as_ref().unwrap().len(), 2);
        assert_eq!(config.tools, vec!["read", "write", "edit", "bash"]);
        assert_eq!(config.plugins.as_ref().unwrap(), &vec!["logger".to_string()]);
        assert_eq!(config.working_dir, PathBuf::from("/tmp/project"));
    }
}
