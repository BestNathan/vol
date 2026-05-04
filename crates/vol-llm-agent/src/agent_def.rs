//! Agent definition types for file-based agent discovery.

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
}

impl AgentDef {
    /// Create a new AgentDef with minimal fields.
    pub fn new(name: &str, content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            id: format!("code:{}", name),
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
        assert_eq!(format!("{}", path), "root/a/b");
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
        };
        assert!(fm.resolve_max_iterations().is_none());
    }

    #[test]
    fn test_agent_def_with_working_dir() {
        let def = AgentDef::new("test", "prompt")
            .with_working_dir(PathBuf::from("/tmp/project"));
        assert_eq!(def.working_dir, Some(PathBuf::from("/tmp/project")));
    }
}
