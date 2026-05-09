//! JSON-RPC server exposing agent operations over WebSocket.

pub mod connection;
pub mod handler;
pub mod serde_helpers;
pub mod server;

pub use server::{AgentRegistration, JsonRpcServer};
