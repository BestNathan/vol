//! Session management with entry-based persistence.

use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData};
use crate::message::SessionMessage;
use crate::store::{Result, SessionEntryStore};
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::Message;

/// Session management
pub struct Session {
    pub id: String,
    pub created_at: i64,
    pub(crate) entry_store: Arc<dyn SessionEntryStore>,
}

impl Session {
    /// Create a new session — self-generates UUID, current timestamp.
    pub fn new(entry_store: Arc<dyn SessionEntryStore>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: {
                #[allow(clippy::unwrap_used)]
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                ts
            },
            entry_store,
        }
    }

    /// Resume from existing session — external ID provided.
    /// Loads created_at from the first entry if available.
    pub async fn resume(id: String, entry_store: Arc<dyn SessionEntryStore>) -> Result<Self> {
        let entries = entry_store.get_entries(&id).await?;
        let created_at = entries.first().map(|e| e.created_at).unwrap_or(0);

        Ok(Self {
            id,
            created_at,
            entry_store,
        })
    }

    /// Add a message entry.
    pub async fn add_message(&self, message: SessionMessage) -> Result<()> {
        let entry = SessionEntry::from_message(message);
        self.entry_store.save(entry).await
    }

    /// Write a checkpoint entry.
    pub async fn checkpoint(&self, reason: CheckpointReason, note: Option<String>) -> Result<()> {
        let entry = SessionEntry::new_checkpoint(self.id.clone(), reason, note);
        self.entry_store.save(entry).await
    }

    /// Write a summary entry (from compression).
    pub async fn add_summary(&self, summary: String) -> Result<()> {
        let entry = SessionEntry::new_summary(self.id.clone(), summary);
        self.entry_store.save(entry).await
    }

    /// Get all messages after the latest checkpoint.
    /// If no checkpoint exists, returns all messages.
    /// Summary entries are converted to synthetic SessionMessage with system role.
    pub async fn get_messages(&self) -> Result<Vec<SessionMessage>> {
        let entries = match self.entry_store.find_latest_checkpoint(&self.id).await? {
            Some(cp) => {
                // Get entries strictly after the checkpoint
                let all = self.entry_store.get_entries(&self.id).await?;
                all.into_iter()
                    .filter(|e| e.created_at > cp.created_at)
                    .collect()
            }
            None => self.entry_store.get_entries(&self.id).await?,
        };

        let mut messages = Vec::new();

        for entry in entries {
            match entry.data {
                SessionEntryData::Message { message } => {
                    messages.push(message);
                }
                SessionEntryData::Summary { summary } => {
                    messages.push(SessionMessage {
                        id: entry.id,
                        session_id: entry.session_id,
                        message: Message::system(summary),
                        parent_id: entry.parent_id,
                        created_at: entry.created_at,
                        metadata: HashMap::new(),
                    });
                }
                SessionEntryData::Checkpoint { .. } => {
                    // Checkpoints are not returned as messages
                }
            }
        }

        Ok(messages)
    }

    /// Get resume messages as raw Messages (after latest checkpoint).
    /// Used for repopulating context on session resume.
    pub async fn resume_messages(&self) -> Result<Vec<Message>> {
        let entries = match self.entry_store.find_latest_checkpoint(&self.id).await? {
            Some(cp) => {
                let all = self.entry_store.get_entries(&self.id).await?;
                all.into_iter()
                    .filter(|e| e.created_at > cp.created_at)
                    .collect()
            }
            None => self.entry_store.get_entries(&self.id).await?,
        };

        let mut messages = Vec::new();

        for entry in entries {
            match entry.data {
                SessionEntryData::Message { message } => {
                    messages.push(message.message);
                }
                SessionEntryData::Summary { summary } => {
                    messages.push(Message::system(summary));
                }
                SessionEntryData::Checkpoint { .. } => {
                    // Checkpoints are not included
                }
            }
        }

        Ok(messages)
    }

    /// Add metadata (no-op, kept for backward compatibility during transition).
    pub fn with_metadata(self, _key: &str, _value: &str) -> Self {
        self
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            created_at: self.created_at,
            entry_store: self.entry_store.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_store::InMemoryEntryStore;

    #[tokio::test]
    async fn test_session_new_self_generates_id() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);

        assert!(!session.id.is_empty());
        assert!(session.created_at > 0);
    }

    #[tokio::test]
    async fn test_session_get_messages() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let session = Session::new(entry_store.clone());

        let msg = SessionMessage::new(session.id.clone(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_with_metadata_noop() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store).with_metadata("user_id", "user-123");

        // with_metadata is a no-op now, session should still work
        let messages = session.get_messages().await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_session_resume_constructor() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        // Create and populate a session
        let session = Session::new(entry_store.clone());
        let session_id = session.id.clone();

        let msg = SessionMessage::new(session_id.clone(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        // Resume from the same entry_store
        let resumed = Session::resume(session_id.clone(), entry_store.clone())
            .await
            .unwrap();
        assert_eq!(resumed.id, session_id);

        // get_messages should return the messages after checkpoint (all, since no checkpoint)
        let messages = resumed.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_checkpoint_and_get_messages() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store.clone());

        // Add a message before checkpoint
        let msg1 = SessionMessage::new(session.id.clone(), Message::user("before cp"));
        session.add_message(msg1).await.unwrap();

        // Write a checkpoint
        session
            .checkpoint(CheckpointReason::Manual, None)
            .await
            .unwrap();

        // Ensure timestamp increments (second-precision timestamps)
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Add a message after checkpoint
        let msg2 = SessionMessage::new(session.id.clone(), Message::user("after cp"));
        session.add_message(msg2).await.unwrap();

        // get_messages should only return messages after the checkpoint
        let messages = session.get_messages().await.unwrap();
        assert_eq!(
            messages.len(),
            1,
            "should only get post-checkpoint messages"
        );
        assert_eq!(
            messages[0].message.content.as_ref().unwrap().as_str(),
            "after cp"
        );
    }

    #[tokio::test]
    async fn test_session_add_summary() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store.clone());

        session
            .add_summary("summarized content".to_string())
            .await
            .unwrap();

        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].message.role == vol_llm_core::MessageRole::System);
        assert_eq!(
            messages[0].message.content.as_ref().unwrap().as_str(),
            "summarized content"
        );
    }

    #[tokio::test]
    async fn test_session_checkpoint_with_note() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store.clone());

        session
            .checkpoint(CheckpointReason::Manual, Some("compact note".into()))
            .await
            .unwrap();

        // Ensure timestamp increments (second-precision timestamps)
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // After checkpoint, no messages before it
        let msg = SessionMessage::new(session.id.clone(), Message::user("post cp"));
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_resume_messages() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store.clone());

        let msg = SessionMessage::new(session.id.clone(), Message::user("hello"));
        session.add_message(msg).await.unwrap();

        let messages = session.resume_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content.as_ref().unwrap().as_str(), "hello");
    }

    #[tokio::test]
    async fn test_session_resume_messages_after_checkpoint() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store.clone());

        // Pre-checkpoint message
        let msg1 = SessionMessage::new(session.id.clone(), Message::user("before"));
        session.add_message(msg1).await.unwrap();

        session
            .checkpoint(CheckpointReason::Manual, None)
            .await
            .unwrap();

        // Ensure timestamp increments (second-precision timestamps)
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Post-checkpoint message
        let msg2 = SessionMessage::new(session.id.clone(), Message::user("after"));
        session.add_message(msg2).await.unwrap();

        let messages = session.resume_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content.as_ref().unwrap().as_str(), "after");
    }

    #[test]
    fn test_session_clone_shares_entry_store() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);
        let cloned = session.clone();

        assert_eq!(cloned.id, session.id);
        assert_eq!(cloned.created_at, session.created_at);
    }

    #[tokio::test]
    async fn test_session_get_messages_no_checkpoint_returns_all() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store.clone());

        let msg1 = SessionMessage::new(session.id.clone(), Message::user("first"));
        let msg2 = SessionMessage::new(session.id.clone(), Message::assistant("second"));
        session.add_message(msg1).await.unwrap();
        session.add_message(msg2).await.unwrap();

        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);
    }
}
