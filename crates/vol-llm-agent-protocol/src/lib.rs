//! vol-llm-agent-channel: protocol and transport abstractions for agent services.
//!
//! Provides Agent Server Protocol types, connection abstractions, domain handler
//! registry plumbing, and generic WebSocket/JSON-RPC transports. Concrete agent
//! execution, routing, and connection-holder implementations live in server crates.

pub mod agent_server_protocol;
pub mod connection;
pub mod domain;
pub mod error;
pub mod operation_codec;
pub mod request;
pub mod service;
pub mod transport;

pub use agent_server_protocol::{
    AgentServerMessage, FileOperation, MessageKind, MessageMeta, Operation, Payload, ProtocolError,
};
pub use connection::Connection;
pub use domain::handler::DomainHandler;
pub use domain::registry::HandlerRegistry;
pub use error::{ChannelError, ConnectionError};
pub use operation_codec::{decode_payload, method_to_operation};
pub use request::{AgentRequest, RunResult};
pub use service::JsonRpcMessageService;
pub use transport::jsonrpc::JsonRpcServer;
pub use transport::{MemoryConnection, MemoryHandle, WsConnection, WsServer};
