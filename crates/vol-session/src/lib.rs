//! vol-session: Session management and entry-based persistence.
//!
//! Provides session management and multi-type entry persistence for ReAct Agent.

pub mod compressor;
pub mod compressors;
pub mod entry;
pub mod error;
pub mod file_store;
pub mod memory_store;
pub mod recorder;
pub mod message;
pub mod session;
pub mod session_contributor;
pub mod store;

pub use session_contributor::SessionContributor;

pub use compressor::MessageCompressor;
pub use compressors::{PositionSampleCompressor, RoleFilterCompressor};
pub use entry::{CheckpointReason, RUN_ID_KEY, SessionEntry, SessionEntryData, SessionEntryType};
pub use error::{Result, SessionError};
pub use file_store::{FileSessionEntryStore, SessionSummary};
pub use recorder::SessionRecorderPlugin;
pub use memory_store::{InMemoryEntryStore, InMemoryMessageStore, InMemorySessionStore};
pub use message::SessionMessage;
pub use session::Session;
pub use store::{MessageStore, SessionEntryStore, SessionStore, StoreError};
