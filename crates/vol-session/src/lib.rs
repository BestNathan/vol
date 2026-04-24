//! vol-session: Session management and entry-based persistence.
//!
//! Provides session management and multi-type entry persistence for ReAct Agent.

pub mod compressor;
pub mod compressors;
pub mod entry;
pub mod error;
pub mod file_store;
pub mod listener;
pub mod memory_store;
pub mod message;
pub mod session;
pub mod store;

pub use compressor::MessageCompressor;
pub use compressors::{PositionSampleCompressor, RoleFilterCompressor};
pub use entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};
pub use error::{Result, SessionError};
pub use file_store::FileMessageStore;
pub use listener::SessionListener;
pub use memory_store::{InMemoryMessageStore, InMemorySessionStore};
pub use message::SessionMessage;
pub use session::Session;
pub use store::{MessageStore, SessionStore, StoreError};
