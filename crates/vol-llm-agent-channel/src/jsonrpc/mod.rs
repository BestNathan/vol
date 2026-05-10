//! JSON-RPC server exposing agent operations over WebSocket.

pub mod connection;
pub mod server;
pub mod serde_helpers;

pub use server::{AgentRegistration, JsonRpcServer};
