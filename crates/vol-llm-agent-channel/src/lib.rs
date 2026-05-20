//! vol-llm-agent-channel: Channel-based communication layer for ReActAgent.
//!
//! Provides `AgentDispatcher` for single-agent request queueing and
//! `AgentRouter` for multi-agent request routing.

pub mod agent_server_protocol;
pub mod connection;
pub mod dispatcher;
pub mod domain;
pub mod error;
pub mod gateway;
pub mod jsonrpc;
pub mod operation_codec;
pub mod protocol;
pub mod request;
pub mod router;
pub mod server_core;
pub mod transport;

pub use agent_server_protocol::{AgentServerMessage, FileOperation, MessageKind, MessageMeta, Operation, Payload, ProtocolError};
pub use connection::{Connection, ConnectionHolder};
pub use dispatcher::AgentDispatcher;
pub use error::{ChannelError, ConnectionError};
pub use jsonrpc::JsonRpcServer;
pub use operation_codec::{decode_payload, method_to_operation};
pub use protocol::Message;
pub use request::{AgentRequest, RunResult};
pub use router::AgentRouter;
pub use domain::handler::DomainHandler;
pub use domain::registry::HandlerRegistry;
pub use server_core::AgentServerCore;
pub use transport::{HttpEventConnection, HttpTransport, MemoryConnection, MemoryHandle, WsConnection, WsServer};
