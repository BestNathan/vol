//! Session management with entry-based persistence.

use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData};
use crate::message::SessionMessage;
use crate::store::{Result, SessionEntryStore};
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::Message;

/// Session metadata key holding the bound task ids, as an array of canonical id
/// strings.
///
/// Companion to [`crate::RUN_ID_KEY`], which is per-message; this one is
/// per-session.
pub const TASK_IDS_KEY: &str = "task_ids";

/// Session management
pub struct Session {
    pub id: String,
    pub created_at: i64,
    pub(crate) entry_store: Arc<dyn SessionEntryStore>,
}

impl Session {
    /// Create a new session — self-generates UUID, current timestamp.
    pub fn new(entry_store: Arc<dyn SessionEntryStore>) -> Self {
        Self::with_id(uuid::Uuid::new_v4().to_string(), entry_store)
    }

    /// Create a new session with an explicit ID.
    pub fn with_id(id: String, entry_store: Arc<dyn SessionEntryStore>) -> Self {
        Self {
            id,
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

    /// Read all session-level metadata.
    ///
    /// A session with no metadata — or no entries at all — reads as an empty
    /// map; absence is not an error.
    pub async fn metadata(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        self.entry_store.get_session_metadata(&self.id).await
    }

    /// Shallow-merge a patch into session-level metadata.
    ///
    /// Keys in `patch` replace existing keys wholesale; other keys are left
    /// alone. This is the general-purpose path for scalar metadata.
    ///
    /// Do NOT use it to accumulate values inside an array. The read you would
    /// need first releases the backend's lock before this write takes it, so a
    /// concurrent writer's addition is silently overwritten — see
    /// `test_read_then_merge_loses_a_concurrent_bind`. Use
    /// [`Self::bind_task_ids`], or add another atomic store operation alongside
    /// `append_session_metadata_values`.
    pub async fn merge_metadata(
        &self,
        patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        self.entry_store
            .merge_session_metadata(&self.id, patch)
            .await
    }

    /// Task ids bound to this session, ascending.
    ///
    /// Numeric ids sort numerically, so `1, 2, 10` — never `1, 10, 2`.
    /// Non-numeric ids sort after every numeric one, lexicographically among
    /// themselves, keeping the order total and deterministic. Ordering is a
    /// read-side presentation choice: the stored array preserves bind order.
    ///
    /// A missing key, a non-array value, or non-string elements read as empty
    /// or are skipped rather than failing — a session must stay readable even
    /// if a foreign writer put something unexpected there.
    pub async fn task_ids(&self) -> Result<Vec<String>> {
        let mut ids: Vec<String> = self
            .metadata()
            .await?
            .get(TASK_IDS_KEY)
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        ids.sort_by(|a, b| {
            let ka = a.parse::<u64>().unwrap_or(u64::MAX);
            let kb = b.parse::<u64>().unwrap_or(u64::MAX);
            ka.cmp(&kb).then_with(|| a.cmp(b))
        });
        ids.dedup();
        Ok(ids)
    }

    /// Add task ids to this session's binding.
    ///
    /// Union semantics: the set only grows. There is no unbind, and binding the
    /// same id twice is a no-op. Ids are not validated against any task store —
    /// the session layer has none and must not acquire one. An empty slice is a
    /// no-op that writes nothing.
    ///
    /// One store call, not read-then-write: the union happens inside the
    /// backend's own lock or transaction
    /// ([`crate::SessionEntryStore::append_session_metadata_values`]), so two
    /// concurrent binds cannot lose an id. The file backend serializes only
    /// writers sharing one store instance — see
    /// [`crate::FileSessionEntryStore`] for that residual limitation.
    ///
    /// Fails with `StoreError::InvalidInput` if the `task_ids` key already
    /// holds a non-array value, rather than clobbering it.
    pub async fn bind_task_ids(&self, ids: &[String]) -> Result<()> {
        self.entry_store
            .append_session_metadata_values(&self.id, TASK_IDS_KEY, ids)
            .await
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

    #[tokio::test]
    async fn test_bind_task_ids_then_read_back() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session
            .bind_task_ids(&["1".to_string(), "2".to_string()])
            .await
            .expect("bind");
        assert_eq!(session.task_ids().await.expect("read"), vec!["1", "2"]);
    }

    #[tokio::test]
    async fn test_bind_task_ids_unions_across_calls() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session
            .bind_task_ids(&["1".into(), "2".into()])
            .await
            .expect("first");
        session
            .bind_task_ids(&["2".into(), "3".into()])
            .await
            .expect("second");

        // Union, not replacement; no duplicates.
        assert_eq!(session.task_ids().await.expect("read"), vec!["1", "2", "3"]);
    }

    #[tokio::test]
    async fn test_bind_task_ids_is_idempotent() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session.bind_task_ids(&["7".into()]).await.expect("first");
        session.bind_task_ids(&["7".into()]).await.expect("second");
        assert_eq!(session.task_ids().await.expect("read"), vec!["7"]);
    }

    #[tokio::test]
    async fn test_bind_task_ids_sorts_numerically_not_lexicographically() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session
            .bind_task_ids(&["10".into(), "2".into(), "1".into()])
            .await
            .expect("bind");
        assert_eq!(
            session.task_ids().await.expect("read"),
            vec!["1", "2", "10"]
        );
    }

    #[tokio::test]
    async fn test_task_ids_orders_non_numeric_ids_last_and_deterministically() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session
            .bind_task_ids(&["zeta".into(), "10".into(), "alpha".into(), "2".into()])
            .await
            .expect("bind");
        assert_eq!(
            session.task_ids().await.expect("read"),
            vec!["2", "10", "alpha", "zeta"]
        );
    }

    #[tokio::test]
    async fn test_task_ids_empty_when_never_bound() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        assert!(session.task_ids().await.expect("read").is_empty());
    }

    #[tokio::test]
    async fn test_bind_nonexistent_task_id_succeeds() {
        // No validation: the session layer has no TaskStore and should not
        // acquire one for a metadata write.
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session
            .bind_task_ids(&["999999".into()])
            .await
            .expect("bind");
        assert_eq!(session.task_ids().await.expect("read"), vec!["999999"]);
    }

    #[tokio::test]
    async fn test_bind_empty_slice_is_a_noop() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session.bind_task_ids(&[]).await.expect("bind");
        assert!(session.task_ids().await.expect("read").is_empty());
        // Not even an empty array key was created.
        assert!(session.metadata().await.expect("meta").is_empty());
    }

    #[tokio::test]
    async fn test_merge_metadata_leaves_task_ids_alone() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session.bind_task_ids(&["1".into()]).await.expect("bind");

        let mut patch = serde_json::Map::new();
        patch.insert("project_id".into(), serde_json::json!("p1"));
        session.merge_metadata(patch).await.expect("merge");

        assert_eq!(session.task_ids().await.expect("read"), vec!["1"]);
        assert_eq!(
            session.metadata().await.expect("meta")["project_id"],
            serde_json::json!("p1")
        );
    }

    #[tokio::test]
    async fn test_task_ids_ignores_a_non_array_value() {
        // A foreign writer could put anything there; reads degrade to empty
        // rather than failing the session.
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        let mut patch = serde_json::Map::new();
        patch.insert(TASK_IDS_KEY.into(), serde_json::json!("not-an-array"));
        session.merge_metadata(patch).await.expect("merge");

        assert!(session.task_ids().await.expect("read").is_empty());
    }

    #[tokio::test]
    async fn test_bind_task_ids_refuses_to_clobber_a_non_array_value() {
        // Writes are stricter than reads: overwriting whatever is there would
        // destroy data we cannot interpret.
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        let mut patch = serde_json::Map::new();
        patch.insert(TASK_IDS_KEY.into(), serde_json::json!(42));
        session.merge_metadata(patch).await.expect("merge");

        assert!(matches!(
            session.bind_task_ids(&["1".into()]).await,
            Err(crate::store::StoreError::InvalidInput(_))
        ));
        // The unreadable value survives.
        assert_eq!(
            session.metadata().await.expect("meta")[TASK_IDS_KEY],
            serde_json::json!(42)
        );
    }

    #[tokio::test]
    async fn test_bind_task_ids_preserves_unrelated_metadata_keys() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        let mut patch = serde_json::Map::new();
        patch.insert("project_id".into(), serde_json::json!("p1"));
        session.merge_metadata(patch).await.expect("merge");

        session.bind_task_ids(&["1".into()]).await.expect("bind");

        let meta = session.metadata().await.expect("meta");
        assert_eq!(meta["project_id"], serde_json::json!("p1"));
        assert_eq!(meta[TASK_IDS_KEY], serde_json::json!(["1"]));
    }

    #[tokio::test]
    async fn test_read_then_merge_loses_a_concurrent_bind() {
        // Why bind_task_ids must not be implemented as
        // metadata() + union + merge_metadata(): the read releases the
        // backend's lock before the write takes it, so anything bound in
        // between is overwritten. This is the deterministic form of the race —
        // it pins the hazard that the atomic append exists to avoid, and keeps
        // the doc comments on merge_metadata honest.
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session.bind_task_ids(&["1".into()]).await.expect("bind 1");

        // Reader A reads [1] ...
        let stale = session.task_ids().await.expect("stale read");
        // ... writer B binds 2 ...
        session.bind_task_ids(&["2".into()]).await.expect("bind 2");
        // ... A writes back its own union of the stale value.
        let mut union = stale;
        union.push("3".into());
        let mut patch = serde_json::Map::new();
        patch.insert(TASK_IDS_KEY.into(), serde_json::json!(union));
        session.merge_metadata(patch).await.expect("merge");

        assert_eq!(
            session.task_ids().await.expect("read"),
            vec!["1", "3"],
            "the read-then-merge path drops id 2 — bind_task_ids must not use it"
        );

        // The atomic path, given the same interleaving, cannot lose anything.
        session.bind_task_ids(&["2".into()]).await.expect("rebind");
        assert_eq!(session.task_ids().await.expect("read"), vec!["1", "2", "3"]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_binds_never_lose_an_id() {
        // 32 tasks, each binding a distinct id, all released together so their
        // read-modify-write windows overlap as much as possible. Every id must
        // survive: the union happens inside the store's write lock.
        //
        // Not a deterministic reproducer — an uncontended tokio RwLock acquire
        // completes without yielding, so a get-then-merge implementation is not
        // *guaranteed* to interleave here. `test_read_then_merge_loses_a_concurrent_bind`
        // is the deterministic pin; this is the end-to-end guard.
        const WRITERS: usize = 32;
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        let barrier = Arc::new(tokio::sync::Barrier::new(WRITERS));

        let mut handles = Vec::new();
        for idx in 0..WRITERS {
            let session = session.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                session.bind_task_ids(&[idx.to_string()]).await
            }));
        }
        for handle in handles {
            handle.await.expect("join").expect("bind");
        }

        let bound = session.task_ids().await.expect("read");
        let expected: Vec<String> = (0..WRITERS).map(|i| i.to_string()).collect();
        assert_eq!(bound, expected, "a concurrent bind lost an id");
    }
}
