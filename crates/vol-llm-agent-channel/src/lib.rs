//! vol-llm-agent-channel: Channel-based communication layer for ReActAgent.
//!
//! Provides `AgentDispatcher` for single-agent request queueing and
//! `AgentRouter` for multi-agent request routing.

pub mod connection;
pub mod dispatcher;
pub mod error;
pub mod protocol;
pub mod request;
pub mod router;
pub mod transport;

pub use connection::{Connection, ConnectionHolder};
pub use dispatcher::AgentDispatcher;
pub use error::{ChannelError, ConnectionError};
pub use protocol::{InboundMessage, OutboundMessage};
pub use request::{AgentRequest, RunResult};
pub use router::AgentRouter;
pub use transport::{WsConnection, WsServer};
