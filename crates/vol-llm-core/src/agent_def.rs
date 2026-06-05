//! Agent definition types for file-based agent discovery.
//! Moved from vol-llm-agent to resolve circular dependency: vol-llm-tool needs AgentDef
//! for ToolContext, but vol-llm-tool cannot depend on vol-llm-agent.

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
            context_files: vec![],
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
