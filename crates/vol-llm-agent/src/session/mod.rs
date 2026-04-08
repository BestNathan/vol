//! Session and Message Store Module
//!
//! Provides session management and message persistence for ReAct Agent.
//!
//! # Architecture
//!
//! - `SessionMessage` - Wrapper around `core::Message` with session context
//! - `Session` - Session container with store references
//! - `SessionStore` / `MessageStore` - Storage traits
//! - `InMemoryMessageStore` / `InMemorySessionStore` - In-memory implementations

pub mod message;
pub mod session;
pub mod store;
pub mod memory_store;

// Types will be exported once implemented in subsequent tasks
// pub use message::SessionMessage;
// pub use session::Session;
// pub use store::{SessionStore, MessageStore};
// pub use memory_store::{InMemorySessionStore, InMemoryMessageStore};
