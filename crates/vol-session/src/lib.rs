//! vol-session: Session management and entry-based persistence.
//!
//! Provides session management and multi-type entry persistence for ReAct Agent.

pub mod compressor;
pub mod compressors;
pub mod database_store;
pub mod entry;
pub mod error;
pub mod file_store;
pub mod manager;
pub mod memory_store;
pub mod message;
pub mod recorder;
pub mod session;
pub mod session_contributor;
pub mod store;

pub use session_contributor::SessionContributor;

pub use compressor::MessageCompressor;
pub use compressors::{PositionSampleCompressor, RoleFilterCompressor};
pub use database_store::{DatabaseSessionEntryStore, DatabaseSessionManager};
pub use entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType, RUN_ID_KEY};
pub use error::{Result, SessionError};
pub use file_store::{FileSessionEntryStore, SessionSummary};
pub use manager::{FileSessionManager, SessionInfo, SessionManager};
pub use memory_store::{InMemoryEntryStore, InMemoryMessageStore, InMemorySessionStore};
pub use message::SessionMessage;
pub use recorder::SessionRecorderPlugin;
pub use session::Session;
pub use store::{MessageStore, SessionEntryStore, SessionStore, StoreError};
