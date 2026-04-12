//! Re-export vol-session for backwards compatibility.

pub use vol_session::{
    FileMessageStore, InMemoryMessageStore, InMemorySessionStore, MessageStore, Result, Session,
    SessionError, SessionListener, SessionMessage, SessionStore,
};
