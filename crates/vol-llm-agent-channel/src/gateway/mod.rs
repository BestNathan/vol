//! Gateway adapters translating transport frames to/from the Agent Server protocol.

pub mod jsonrpc_ws;

pub use jsonrpc_ws::{decode_jsonrpc_frame, encode_jsonrpc_message};
