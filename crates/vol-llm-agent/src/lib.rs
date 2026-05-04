//! vol-llm-agent: ReAct Agent and RAG Agent workflow orchestration.

pub mod agent_def;
pub mod agent_loader;
pub mod agent_tool;
pub mod embedding;
pub mod plugins;
pub mod prompt_context;
pub mod rag;
pub mod react;

// Re-export vol-session types
pub use agent_def::{AgentDef, AgentDefError, AgentPath, AgentScope};
pub use agent_loader::AgentLoader;
pub use agent_tool::AgentTool;
pub use embedding::{DashScopeConfig, DashScopeEmbedder, DashScopeModel, Embedder};
pub use plugins::{CliApprovalChannel, SimpleHttpApprovalChannel};
pub use rag::{Document, EmbeddingStore, RagAgent, RagConfig, RagResponse};
pub use react::state::{ReasoningStep, ToolCallRecord};
pub use react::{
    AgentConfig, AgentConfigBuilder, AgentConfigBuildError, AgentError, AgentResponse, AgentStreamEvent, AgentStreamReceiver,
    ReActAgent,
};
