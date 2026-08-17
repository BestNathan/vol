//! Agent definition types for file-based agent discovery.
//! Moved from vol-llm-agent to resolve circular dependency: vol-llm-tool needs AgentDef
//! for ToolContext, but vol-llm-tool cannot depend on vol-llm-agent.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Discovery scope for agent definitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentScope {
    /// ~/.agents/agents/ — user personal agents
    User,
    /// {working_dir}/.agents/agents/ — project-specific agents
    Repo,
}

impl AgentScope {
    /// Returns the scope prefix string for agent IDs.
    pub fn prefix(&self) -> &str {
        match self {
            AgentScope::User => "user",
            AgentScope::Repo => "repo",
        }
    }
}

impl fmt::Display for AgentScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentScope::User => write!(f, "User"),
            AgentScope::Repo => write!(f, "Repo"),
        }
    }
}

/// A parsed agent definition from a .md file.
#[derive(Debug, Clone)]
pub struct AgentDef {
    /// Unique ID: "{scope_prefix}:{name}" e.g. "repo:test-runner".
    /// `new()` uses a placeholder prefix; the loader sets the correct scope-based ID.
    pub id: String,
    /// Agent name from frontmatter
    pub name: String,
    /// Dispatch key (defaults to name if not specified)
    pub r#type: String,
    /// Short description
    pub description: String,
    /// Discovery scope
    pub scope: AgentScope,
    /// Allowed tools (None = inherit all parent tools)
    pub tools: Option<Vec<String>>,
    /// Blacklisted tools
    pub disallowed_tools: Option<Vec<String>>,
    /// Model override
    pub model: Option<String>,
    /// Max ReAct iterations
    pub max_iterations: Option<u32>,
    /// Markdown body (system prompt)
    pub max_history_messages: Option<usize>,
    /// Markdown body (system prompt)
    pub prompt: String,
    /// Working directory for skill/agent discovery scope.
    pub working_dir: Option<PathBuf>,
    /// Custom context files injected into the Middle zone.
    /// Each path is relative to the agent's working directory.
    /// Files are loaded in array order: first file → Middle(0), second → Middle(1), etc.
    pub context_files: Vec<String>,
    /// Default sandbox name (registry key). Overrides the global default ("local").
    pub sandbox: Option<String>,
    /// Per-tool configurations. Key is the tool name (e.g. "bash"), value is a
    /// TOML table that may include a `sandbox` key and tool-specific fields.
    pub tool_config: Option<HashMap<String, serde_json::Value>>,
    /// MCP server names to use. When set, only MCP tools from these servers
    /// are registered. When absent (None), all configured MCP tools are available.
    pub mcps: Option<Vec<String>>,
    /// Skill allowlist. None = all skills available.
    pub skills: Option<Vec<String>>,
}

impl Default for AgentDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            r#type: String::new(),
            description: String::new(),
            scope: AgentScope::Repo,
            tools: None,
            disallowed_tools: None,
            model: None,
            max_iterations: None,
            max_history_messages: None,
            prompt: String::new(),
            working_dir: None,
            context_files: vec![],
            sandbox: None,
            tool_config: None,
            mcps: None,
            skills: None,
        }
    }
}

impl AgentDef {
    /// Create a new AgentDef with minimal fields.
    pub fn new(name: &str, content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            id: format!("code:{name}"),
            name: name.to_string(),
            r#type: name.to_string(),
            description: String::new(),
            scope: AgentScope::Repo,
            tools: None,
            disallowed_tools: None,
            model: None,
            max_iterations: None,
            max_history_messages: None,
            prompt: content_str,
            working_dir: None,
            context_files: vec![],
            sandbox: None,
            tool_config: None,
            mcps: None,
            skills: None,
        }
    }

    /// Set type for dispatch matching.
    pub fn with_type(mut self, r#type: impl Into<String>) -> Self {
        self.r#type = r#type.into();
        self
    }

    /// Set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set allowed tools.
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set disallowed tools.
    pub fn with_disallowed_tools(mut self, tools: Vec<String>) -> Self {
        self.disallowed_tools = Some(tools);
        self
    }

    /// Set max iterations.
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// Set max history messages.
    pub fn with_max_history_messages(mut self, max: usize) -> Self {
        self.max_history_messages = Some(max);
        self
    }

    /// Set the working directory for skill discovery scope.
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set custom context files to inject into the Middle zone.
    pub fn with_context_files(mut self, files: Vec<String>) -> Self {
        self.context_files = files;
        self
    }

    /// Set the default sandbox for this agent.
    pub fn with_sandbox(mut self, sandbox: impl Into<String>) -> Self {
        self.sandbox = Some(sandbox.into());
        self
    }

    /// Set per-tool configurations.
    pub fn with_tool_config(mut self, config: HashMap<String, serde_json::Value>) -> Self {
        self.tool_config = Some(config);
        self
    }

    /// Set MCP server names to use.
    pub fn with_mcps(mut self, mcps: Vec<String>) -> Self {
        self.mcps = Some(mcps);
        self
    }
}

/// Metadata for progressive disclosure (injected into system prompt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub description: String,
    pub scope: AgentScope,
}

impl From<&AgentDef> for AgentMetadata {
    fn from(def: &AgentDef) -> Self {
        Self {
            id: def.id.clone(),
            name: def.name.clone(),
            r#type: def.r#type.clone(),
            description: def.description.clone(),
            scope: def.scope.clone(),
        }
    }
}

/// Tracks the dispatch chain of agent invocations.
#[derive(Debug, Clone)]
pub struct AgentPath {
    segments: Vec<String>,
}

impl AgentPath {
    /// Create a root path.
    pub fn root() -> Self {
        Self {
            segments: vec!["root".to_string()],
        }
    }

    /// Push a new segment onto the path.
    pub fn push(&self, name: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(name.to_string());
        Self { segments }
    }

    /// Get the current depth (number of segments).
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Get the path as a string.
    pub fn as_str(&self) -> String {
        self.segments.join("/")
    }
}

impl fmt::Display for AgentPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.segments.join("/"))
    }
}

/// Error type for agent definition operations.
///
/// Currently reserved for future use — the loader handles errors by logging
/// warnings and skipping invalid files rather than propagating them.
#[derive(Debug, thiserror::Error)]
pub enum AgentDefError {
    #[error("Agent type '{0}' not found")]
    TypeNotFound(String),
    #[error("Dispatch depth exceeded (max {0}, path: {1})")]
    DepthExceeded(u32, String),
    #[error("Invalid agent definition: {0}")]
    InvalidDef(String),
    #[error("Loader error: {0}")]
    Loader(String),
}

impl AgentDefError {
    /// Create a TypeNotFound error. Used internally by AgentTool (via ToolError wrapper).
    pub fn type_not_found(r#type: &str) -> Self {
        Self::TypeNotFound(r#type.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_def_default_skills_is_none() {
        let def = AgentDef::default();
        assert!(def.skills.is_none());
    }

    #[test]
    fn test_agent_def_new_skills_is_none() {
        let def = AgentDef::new("test", "prompt");
        assert!(def.skills.is_none());
    }

    #[test]
    fn test_agent_scope_prefix_and_display() {
        assert_eq!(AgentScope::User.prefix(), "user");
        assert_eq!(AgentScope::Repo.prefix(), "repo");
        assert_eq!(AgentScope::User.to_string(), "User");
        assert_eq!(AgentScope::Repo.to_string(), "Repo");
    }

    #[test]
    fn test_agent_scope_serde_roundtrip() {
        let json = serde_json::to_string(&AgentScope::Repo).unwrap();
        assert_eq!(json, r#""Repo""#);
        let parsed: AgentScope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, AgentScope::Repo);
        let parsed: AgentScope = serde_json::from_str(r#""User""#).unwrap();
        assert_eq!(parsed, AgentScope::User);
    }

    #[test]
    fn test_agent_def_builders() {
        let mut tool_config = HashMap::new();
        tool_config.insert(
            "bash".to_string(),
            serde_json::json!({"sandbox": "isolated"}),
        );

        let def = AgentDef::new("dev", "You are a dev agent")
            .with_type("dev-dispatch")
            .with_description("A dev agent")
            .with_tools(vec!["bash".to_string(), "read".to_string()])
            .with_disallowed_tools(vec!["rm".to_string()])
            .with_max_iterations(25)
            .with_max_history_messages(50)
            .with_working_dir(PathBuf::from("/work"))
            .with_context_files(vec!["docs/guide.md".to_string()])
            .with_sandbox("local")
            .with_tool_config(tool_config.clone())
            .with_mcps(vec!["k8s".to_string()]);

        assert_eq!(def.id, "code:dev");
        assert_eq!(def.name, "dev");
        assert_eq!(def.r#type, "dev-dispatch");
        assert_eq!(def.description, "A dev agent");
        assert_eq!(def.scope, AgentScope::Repo);
        assert_eq!(
            def.tools,
            Some(vec!["bash".to_string(), "read".to_string()])
        );
        assert_eq!(def.disallowed_tools, Some(vec!["rm".to_string()]));
        assert_eq!(def.max_iterations, Some(25));
        assert_eq!(def.max_history_messages, Some(50));
        assert_eq!(def.working_dir, Some(PathBuf::from("/work")));
        assert_eq!(def.context_files, vec!["docs/guide.md".to_string()]);
        assert_eq!(def.sandbox.as_deref(), Some("local"));
        assert_eq!(def.tool_config, Some(tool_config));
        assert_eq!(def.mcps, Some(vec!["k8s".to_string()]));
        assert_eq!(def.prompt, "You are a dev agent");
        // Builder defaults remain unset
        assert!(def.skills.is_none());
    }

    #[test]
    fn test_agent_def_default_values() {
        let def = AgentDef::default();
        assert_eq!(def.id, "");
        assert_eq!(def.scope, AgentScope::Repo);
        assert!(def.tools.is_none());
        assert!(def.disallowed_tools.is_none());
        assert!(def.model.is_none());
        assert!(def.max_iterations.is_none());
        assert!(def.max_history_messages.is_none());
        assert!(def.working_dir.is_none());
        assert!(def.sandbox.is_none());
        assert!(def.tool_config.is_none());
        assert!(def.mcps.is_none());
        assert!(def.context_files.is_empty());
    }

    #[test]
    fn test_agent_metadata_from_def() {
        let def = AgentDef::new("qa", "prompt")
            .with_type("qa-dispatch")
            .with_description("QA agent");
        let meta = AgentMetadata::from(&def);
        assert_eq!(meta.id, "code:qa");
        assert_eq!(meta.name, "qa");
        assert_eq!(meta.r#type, "qa-dispatch");
        assert_eq!(meta.description, "QA agent");
        assert_eq!(meta.scope, AgentScope::Repo);
    }

    #[test]
    fn test_agent_path() {
        let root = AgentPath::root();
        assert_eq!(root.depth(), 1);
        assert_eq!(root.as_str(), "root");
        assert_eq!(root.to_string(), "root");

        let child = root.push("dev");
        assert_eq!(child.depth(), 2);
        assert_eq!(child.as_str(), "root/dev");
        assert_eq!(child.to_string(), "root/dev");

        let grandchild = child.push("bash");
        assert_eq!(grandchild.depth(), 3);
        assert_eq!(grandchild.as_str(), "root/dev/bash");

        // Pushing does not mutate the original path
        assert_eq!(root.as_str(), "root");
    }

    #[test]
    fn test_agent_def_error_display() {
        assert_eq!(
            AgentDefError::TypeNotFound("nope".to_string()).to_string(),
            "Agent type 'nope' not found"
        );
        assert_eq!(
            AgentDefError::DepthExceeded(5, "root/a/b".to_string()).to_string(),
            "Dispatch depth exceeded (max 5, path: root/a/b)"
        );
        assert_eq!(
            AgentDefError::InvalidDef("bad frontmatter".to_string()).to_string(),
            "Invalid agent definition: bad frontmatter"
        );
        assert_eq!(
            AgentDefError::Loader("io failed".to_string()).to_string(),
            "Loader error: io failed"
        );
        assert_eq!(
            AgentDefError::type_not_found("missing").to_string(),
            "Agent type 'missing' not found"
        );
    }
}
