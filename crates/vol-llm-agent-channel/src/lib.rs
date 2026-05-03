//! vol-llm-agent-channel: Channel-based communication layer for ReActAgent.
//!
//! Provides `AgentDispatcher` for single-agent request queueing and
//! `AgentRouter` for multi-agent request routing.

pub mod dispatcher;
pub mod error;
pub mod request;
pub mod router;

pub use dispatcher::AgentDispatcher;
pub use error::ChannelError;
pub use request::{AgentRequest, RunResult};
pub use router::AgentRouter;
