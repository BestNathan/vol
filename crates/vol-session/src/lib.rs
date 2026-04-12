//! vol-session: Session management and message persistence.
//!
//! Provides session management and message persistence for ReAct Agent.

pub mod message;
pub mod session;
pub mod store;
pub mod memory_store;
pub mod file_store;
pub mod listener;
pub mod error;

pub use message::SessionMessage;
pub use session::Session;
pub use store::{SessionStore, MessageStore, StoreError};
pub use memory_store::{InMemorySessionStore, InMemoryMessageStore};
pub use file_store::FileMessageStore;
pub use listener::SessionListener;
pub use error::{SessionError, Result};
