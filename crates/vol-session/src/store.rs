//! Session and Entry store traits.

use crate::entry::SessionEntry;
use crate::message::SessionMessage;
use crate::session::Session;
use async_trait::async_trait;
use thiserror::Error;

/// Store operation error
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Session agent scope conflict for {session_id}: expected {expected}, actual {actual}")]
    SessionAgentScopeConflict {
        session_id: String,
        expected: String,
        actual: String,
    },
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Session storage interface
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Create a session
    async fn create(&self, session: Session) -> Result<()>;

    /// Get a session by ID
    async fn get(&self, session_id: &str) -> Result<Option<Session>>;

    /// Delete a session
    async fn delete(&self, session_id: &str) -> Result<()>;

    /// Update a session
    async fn update(&self, session: Session) -> Result<()>;
}

/// Entry storage interface — supports Message, Checkpoint, and Summary entry types.
/// Multi-session: methods accept session_id to scope operations.
#[async_trait]
pub trait SessionEntryStore: Send + Sync {
    /// Append an entry (entry already carries session_id).
    async fn save(&self, entry: SessionEntry) -> Result<()>;

    /// Get all entries for a session.
    async fn get_entries(&self, session_id: &str) -> Result<Vec<SessionEntry>>;

    /// Get entries after a timestamp for a session.
    async fn get_after(&self, session_id: &str, after: i64) -> Result<Vec<SessionEntry>>;

    /// Find the latest checkpoint entry for a session.
    async fn find_latest_checkpoint(&self, session_id: &str) -> Result<Option<SessionEntry>>;

    /// Delete all entries for a session.
    async fn delete_session(&self, session_id: &str) -> Result<()>;

    /// Get entry count for a session.
    async fn get_count(&self, session_id: &str) -> Result<usize>;

    /// Read session-level metadata.
    ///
    /// Returns an empty map for a session that does not exist — absence of
    /// metadata is not an error.
    async fn get_session_metadata(
        &self,
        session_id: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>>;

    /// Shallow-merge a patch into session-level metadata.
    ///
    /// Keys in `patch` replace existing keys wholesale; keys absent from
    /// `patch` are left alone. Creates the session record if it does not yet
    /// exist — a binding can be written before the first entry.
    ///
    /// Not a way to accumulate array values — see
    /// [`Self::append_session_metadata_values`], which is atomic.
    async fn merge_session_metadata(
        &self,
        session_id: &str,
        patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()>;

    /// Union `values` into the JSON array held at `key`, as a single atomic
    /// read-modify-write.
    ///
    /// Existing elements keep their positions and each value not already
    /// present is appended in the order given, so the array only grows and
    /// appending the same value twice is a no-op. Creates the key — and the
    /// session record — when absent, so a binding can be written before the
    /// first entry.
    ///
    /// Use this to accumulate array values, never
    /// [`Self::get_session_metadata`] followed by
    /// [`Self::merge_session_metadata`]: that read releases the backend's lock
    /// before the write takes it, so two concurrent callers can both observe
    /// the same array and one silently overwrites the other's addition.
    /// Implementations must perform the union inside whatever lock or
    /// transaction already guards the metadata write.
    ///
    /// An empty `values` is a no-op: nothing is read, written, or created, and
    /// no ownership check is performed.
    ///
    /// Fails with [`StoreError::InvalidInput`] when `key` already holds a
    /// non-array value, leaving that value untouched rather than clobbering
    /// data it cannot interpret.
    async fn append_session_metadata_values(
        &self,
        session_id: &str,
        key: &str,
        values: &[String],
    ) -> Result<()>;
}

/// Union `values` into the array at `key` of an already-loaded metadata map.
///
/// Defines the semantics of [`SessionEntryStore::append_session_metadata_values`]
/// once, so all three backends agree. The caller must already hold the lock or
/// transaction guarding `metadata` — this function provides the *what*, the
/// backend provides the atomicity.
///
/// Returns whether `metadata` changed, letting a backend skip a write that
/// would store identical bytes. Never mutates `metadata` on error.
pub(crate) fn union_metadata_values(
    metadata: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    values: &[String],
) -> Result<bool> {
    if values.is_empty() {
        return Ok(false);
    }

    let mut array = match metadata.get(key) {
        Some(serde_json::Value::Array(existing)) => existing.clone(),
        None => Vec::new(),
        Some(_) => {
            return Err(StoreError::InvalidInput(format!(
                "session metadata key `{key}` holds a non-array value; refusing to overwrite it"
            )))
        }
    };

    let mut changed = false;
    for value in values {
        // Comparison is by string element. Foreign non-string elements are
        // preserved but never match, so they cannot suppress an append.
        if !array.iter().any(|v| v.as_str() == Some(value.as_str())) {
            array.push(serde_json::Value::String(value.clone()));
            changed = true;
        }
    }

    if changed {
        metadata.insert(key.to_string(), serde_json::Value::Array(array));
    }
    Ok(changed)
}

/// Legacy MessageStore trait — kept for backward compatibility.
/// New code should use SessionEntryStore instead.
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// Save a message
    async fn save(&self, message: SessionMessage) -> Result<()>;

    /// Get messages by session ID
    async fn get_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>>;

    /// Get messages before a timestamp (pagination)
    async fn get_before(
        &self,
        session_id: &str,
        before: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>>;

    /// Get messages after a timestamp (for compressed history)
    async fn get_after(
        &self,
        session_id: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>>;

    /// Delete all messages for a session
    async fn delete_session(&self, session_id: &str) -> Result<()>;

    /// Update a message
    async fn update(&self, id: &str, message: SessionMessage) -> Result<()>;

    /// Get message count for a session
    async fn get_count(&self, session_id: &str) -> Result<usize>;

    /// Cleanup expired messages
    async fn cleanup_expired(&self, before: i64) -> Result<()>;
}

#[cfg(test)]
mod union_tests {
    use super::{union_metadata_values, StoreError};

    fn map(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn test_union_creates_the_key_when_absent() {
        let mut metadata = serde_json::Map::new();
        let changed =
            union_metadata_values(&mut metadata, "task_ids", &["1".into(), "2".into()]).unwrap();
        assert!(changed);
        assert_eq!(metadata["task_ids"], serde_json::json!(["1", "2"]));
    }

    #[test]
    fn test_union_appends_only_missing_values_in_bind_order() {
        let mut metadata = map(&[("task_ids", serde_json::json!(["2"]))]);
        let changed = union_metadata_values(
            &mut metadata,
            "task_ids",
            &["10", "2", "1"].map(String::from),
        )
        .unwrap();
        assert!(changed);
        // Existing elements keep their position; new ones append in bind order.
        // Sorting is the reader's job.
        assert_eq!(metadata["task_ids"], serde_json::json!(["2", "10", "1"]));
    }

    #[test]
    fn test_union_dedupes_within_one_call() {
        let mut metadata = serde_json::Map::new();
        union_metadata_values(&mut metadata, "task_ids", &["1".into(), "1".into()]).unwrap();
        assert_eq!(metadata["task_ids"], serde_json::json!(["1"]));
    }

    #[test]
    fn test_union_reports_no_change_when_everything_is_present() {
        let mut metadata = map(&[("task_ids", serde_json::json!(["1", "2"]))]);
        let changed = union_metadata_values(&mut metadata, "task_ids", &["2".into()]).unwrap();
        assert!(!changed, "a backend must be free to skip the write");
        assert_eq!(metadata["task_ids"], serde_json::json!(["1", "2"]));
    }

    #[test]
    fn test_union_of_no_values_creates_nothing() {
        let mut metadata = serde_json::Map::new();
        let changed = union_metadata_values(&mut metadata, "task_ids", &[]).unwrap();
        assert!(!changed);
        assert!(
            metadata.is_empty(),
            "an empty append must not create the key"
        );
    }

    #[test]
    fn test_union_refuses_a_non_array_value_and_leaves_it_intact() {
        let mut metadata = map(&[("task_ids", serde_json::json!("oops"))]);
        let err = union_metadata_values(&mut metadata, "task_ids", &["1".into()]).unwrap_err();
        assert!(matches!(err, StoreError::InvalidInput(_)), "got {err:?}");
        assert_eq!(metadata["task_ids"], serde_json::json!("oops"));
    }

    #[test]
    fn test_union_preserves_foreign_elements_without_matching_them() {
        // A number 2 is not the string "2": it stays, and it does not suppress
        // the append that would otherwise look like a duplicate.
        let mut metadata = map(&[("task_ids", serde_json::json!([2, null]))]);
        union_metadata_values(&mut metadata, "task_ids", &["2".into()]).unwrap();
        assert_eq!(metadata["task_ids"], serde_json::json!([2, null, "2"]));
    }

    #[test]
    fn test_union_leaves_other_keys_alone() {
        let mut metadata = map(&[("project_id", serde_json::json!("p1"))]);
        union_metadata_values(&mut metadata, "task_ids", &["1".into()]).unwrap();
        assert_eq!(metadata["project_id"], serde_json::json!("p1"));
        assert_eq!(metadata["task_ids"], serde_json::json!(["1"]));
    }
}
