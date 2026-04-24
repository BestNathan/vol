//! vol-llm-agent: ReAct Agent and RAG Agent workflow orchestration.

pub mod embedding;
pub mod observability;
pub mod plugins;
pub mod prompt_context;
pub mod rag;
pub mod react;

// Re-export vol-session types
pub use embedding::{DashScopeConfig, DashScopeEmbedder, DashScopeModel, Embedder};
pub use observability::{cleanup_old_logs, cleanup_run_logs, cleanup_session_logs, LogEntry, ObservabilityPlugin, RunLogLogger};
pub use plugins::{CliApprovalChannel, SimpleHttpApprovalChannel};
pub use rag::{Document, EmbeddingStore, RagAgent, RagConfig, RagResponse};
pub use react::state::{ReasoningStep, ToolCallRecord};
pub use react::{
    AgentBuilder, AgentConfig, AgentError, AgentResponse, AgentStreamEvent, AgentStreamReceiver,
    ReActAgent,
};
