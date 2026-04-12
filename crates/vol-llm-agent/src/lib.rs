//! vol-llm-agent: ReAct Agent and RAG Agent workflow orchestration.

pub mod embedding;
pub mod prompt_context;
pub mod react;
pub mod rag;
pub mod session;
pub mod plugins;
pub mod observability;

// Re-export vol-session types
pub use vol_session::{
    Session, SessionMessage, SessionStore, MessageStore,
    InMemorySessionStore, InMemoryMessageStore, FileMessageStore,
    SessionListener, SessionError, Result,
};
pub use react::{ReActAgent, AgentConfig, AgentBuilder, AgentResponse, AgentError, AgentStreamEvent, AgentStreamReceiver};
pub use react::state::{ReasoningStep, ToolCallRecord};
pub use embedding::{Embedder, DashScopeEmbedder, DashScopeConfig, DashScopeModel};
pub use prompt_context::{PromptTemplate, PromptFragment, FragmentType, PromptContext, MessageAssembler};
pub use rag::{RagAgent, RagResponse, RagConfig, Document, EmbeddingStore};
pub use plugins::{CliApprovalChannel, HttpApprovalChannel, SimpleHttpApprovalChannel};
pub use observability::{ObservabilityPlugin, ObservabilityLogger, LogEntry, LogType};
pub use observability::{cleanup_old_logs, cleanup_session_logs, cleanup_run_logs};
