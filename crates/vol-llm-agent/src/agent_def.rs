//! Agent definition types — re-exported from vol-llm-core.
//!
//! This module re-exports the core AgentDef types that were moved to vol-llm-core
//! to resolve a circular dependency between vol-llm-tool and vol-llm-agent.
//! It also retains AgentFrontmatter which is specific to the agent loading layer.

use serde::Deserialize;

pub use vol_llm_core::agent_def::{AgentDef, AgentDefError, AgentMetadata, AgentPath, AgentScope};

/// Frontmatter schema for agent definition files.
#[derive(Debug, Deserialize)]
pub struct AgentFrontmatter {
    /// Required. Unique identifier for this agent template
    pub name: String,
    /// Optional. Dispatch key (defaults to name if not specified)
    #[serde(default)]
    pub r#type: Option<String>,
    /// Required. Short description for LLM matching
    pub description: String,
    /// Optional. Allowed tool names
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    /// Optional. Blacklisted tool names
    #[serde(default)]
    pub disallowed_tools: Option<Vec<String>>,
    /// Optional. Model override
    #[serde(default)]
    pub model: Option<String>,
    /// Optional. Max ReAct iterations
    #[serde(default)]
    pub max_iterations: Option<u32>,
    /// Optional. Alias for max_iterations
    #[serde(default)]
    pub max_turns: Option<u32>,
    /// Optional. Max history messages from session
    #[serde(default)]
    pub max_history_messages: Option<usize>,
    /// Optional. Working directory root for skill/agent discovery.
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Optional. Context files to inject into the Middle zone (relative to working_dir).
    #[serde(default)]
    pub context_files: Option<Vec<String>>,
    /// Optional. Default sandbox for this agent (registry name).
    #[serde(default)]
    pub sandbox: Option<String>,
    /// Optional. Per-tool configuration. Key = tool name, value = tool config (can include `sandbox` key).
    #[serde(default)]
    pub tool_config: Option<std::collections::HashMap<String, serde_json::Value>>,
    /// Optional. MCP server names to use. When set, only MCP tools from these
    /// servers are registered for the agent. When absent, all MCP tools are available.
    #[serde(default)]
    pub mcps: Option<Vec<String>>,
}

impl AgentFrontmatter {
    /// Resolve the type field (defaults to name if not specified).
    pub fn resolve_type(&self) -> String {
        self.r#type.clone().unwrap_or_else(|| self.name.clone())
    }

    /// Resolve max_iterations (checks max_turns alias).
    pub fn resolve_max_iterations(&self) -> Option<u32> {
        self.max_iterations.or(self.max_turns)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_agent_path_root() {
        let path = AgentPath::root();
        assert_eq!(path.depth(), 1);
        assert_eq!(path.as_str(), "root");
    }

    #[test]
    fn test_agent_path_push() {
        let root = AgentPath::root();
        let child = root.push("test-runner");
        assert_eq!(child.depth(), 2);
        assert_eq!(child.as_str(), "root/test-runner");
        assert_eq!(root.as_str(), "root");
    }

    #[test]
    fn test_agent_path_display() {
        let path = AgentPath::root().push("a").push("b");
        assert_eq!(format!("{path}"), "root/a/b");
    }

    #[test]
    fn test_agent_def_new() {
        let def = AgentDef::new("test-agent", "You are a test agent.");
        assert_eq!(def.name, "test-agent");
        assert_eq!(def.r#type, "test-agent");
        assert_eq!(def.prompt, "You are a test agent.");
        assert!(def.tools.is_none());
    }

    #[test]
    fn test_agent_def_builder() {
        let def = AgentDef::new("test-agent", "prompt")
            .with_type("code-reviewer")
            .with_description("Reviews code")
            .with_tools(vec!["Read".to_string()])
            .with_disallowed_tools(vec!["Write".to_string()])
            .with_max_iterations(10);
        assert_eq!(def.r#type, "code-reviewer");
        assert_eq!(def.description, "Reviews code");
        assert_eq!(def.tools, Some(vec!["Read".to_string()]));
        assert_eq!(def.disallowed_tools, Some(vec!["Write".to_string()]));
        assert_eq!(def.max_iterations, Some(10));
    }

    #[test]
    fn test_agent_scope_prefix() {
        assert_eq!(AgentScope::User.prefix(), "user");
        assert_eq!(AgentScope::Repo.prefix(), "repo");
    }

    #[test]
    fn test_agent_metadata_from_def() {
        let def = AgentDef::new("test", "content").with_type("reviewer");
        let meta = AgentMetadata::from(&def);
        assert_eq!(meta.name, "test");
        assert_eq!(meta.r#type, "reviewer");
    }

    #[test]
    fn test_frontmatter_resolve_type_defaults_to_name() {
        let fm = AgentFrontmatter {
            name: "my-agent".to_string(),
            r#type: None,
            description: "desc".to_string(),
            tools: None,
            disallowed_tools: None,
            model: None,
            max_iterations: None,
            max_turns: None,
            max_history_messages: None,
            working_dir: None,
            context_files: None,
            sandbox: None,
            tool_config: None,
            mcps: None,
        };
        assert_eq!(fm.resolve_type(), "my-agent");
    }

    #[test]
    fn test_frontmatter_resolve_type_explicit() {
        let fm = AgentFrontmatter {
            name: "my-agent".to_string(),
            r#type: Some("code-reviewer".to_string()),
            description: "desc".to_string(),
            tools: None,
            disallowed_tools: None,
            model: None,
            max_iterations: None,
            max_turns: None,
            max_history_messages: None,
            working_dir: None,
            context_files: None,
            sandbox: None,
            tool_config: None,
            mcps: None,
        };
        assert_eq!(fm.resolve_type(), "code-reviewer");
    }

    #[test]
    fn test_frontmatter_resolve_max_iterations_prefers_explicit() {
        let fm = AgentFrontmatter {
            name: "a".to_string(),
            r#type: None,
            description: "d".to_string(),
            tools: None,
            disallowed_tools: None,
            model: None,
            max_iterations: Some(10),
            max_turns: Some(20),
            max_history_messages: None,
            working_dir: None,
            context_files: None,
            sandbox: None,
            tool_config: None,
            mcps: None,
        };
        assert_eq!(fm.resolve_max_iterations(), Some(10));
    }

    #[test]
    fn test_frontmatter_resolve_max_iterations_falls_back_to_turns() {
        let fm = AgentFrontmatter {
            name: "a".to_string(),
            r#type: None,
            description: "d".to_string(),
            tools: None,
            disallowed_tools: None,
            model: None,
            max_iterations: None,
            max_turns: Some(20),
            max_history_messages: None,
            working_dir: None,
            context_files: None,
            sandbox: None,
            tool_config: None,
            mcps: None,
        };
        assert_eq!(fm.resolve_max_iterations(), Some(20));
    }

    #[test]
    fn test_frontmatter_resolve_max_iterations_none() {
        let fm = AgentFrontmatter {
            name: "a".to_string(),
            r#type: None,
            description: "d".to_string(),
            tools: None,
            disallowed_tools: None,
            model: None,
            max_iterations: None,
            max_turns: None,
            max_history_messages: None,
            working_dir: None,
            context_files: None,
            sandbox: None,
            tool_config: None,
            mcps: None,
        };
        assert!(fm.resolve_max_iterations().is_none());
    }

    #[test]
    fn test_agent_def_with_working_dir() {
        let def = AgentDef::new("test", "prompt").with_working_dir(PathBuf::from("/tmp/project"));
        assert_eq!(def.working_dir, Some(PathBuf::from("/tmp/project")));
    }

    #[test]
    fn test_agent_def_with_mcps() {
        let def = AgentDef::new("test", "prompt")
            .with_mcps(vec!["docs-rs".to_string(), "weather".to_string()]);
        assert_eq!(
            def.mcps,
            Some(vec!["docs-rs".to_string(), "weather".to_string()])
        );
    }

    #[test]
    fn test_frontmatter_mcps_defaults_to_none() {
        let fm = AgentFrontmatter {
            name: "a".to_string(),
            r#type: None,
            description: "d".to_string(),
            tools: None,
            disallowed_tools: None,
            model: None,
            max_iterations: None,
            max_turns: None,
            max_history_messages: None,
            working_dir: None,
            context_files: None,
            sandbox: None,
            tool_config: None,
            mcps: None,
        };
        assert!(fm.mcps.is_none());
    }
}
