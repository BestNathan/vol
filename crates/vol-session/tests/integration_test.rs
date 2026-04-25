//! Integration tests for FileSessionEntryStore end-to-end workflow.
//!
//! These tests verify that FileSessionEntryStore correctly persists
//! SessionEntry objects to JSONL files and supports checkpoint/resume.

use std::sync::Arc;
use vol_llm_core::Message;
use vol_session::{FileSessionEntryStore, SessionEntry, SessionEntryStore};

/// Test FileSessionEntryStore save and retrieve workflow
#[tokio::test]
async fn test_file_entry_store_save_and_get_entries() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn SessionEntryStore> =
        Arc::new(FileSessionEntryStore::new(tmp_dir.path()));

    // Save several entries
    let entry1 = SessionEntry::new_message("session-1".to_string(), Message::user("Hello"));
    let entry2 =
        SessionEntry::new_message("session-1".to_string(), Message::assistant("Hi there!"));
    let entry3 = SessionEntry::new_message("session-1".to_string(), Message::user("How are you?"));

    store.save(entry1).await.unwrap();
    store.save(entry2).await.unwrap();
    store.save(entry3).await.unwrap();

    // Get all entries
    let entries = store.get_entries("session-1").await.unwrap();
    assert_eq!(entries.len(), 3, "Expected 3 entries, got {}", entries.len());

    // Verify order and types
    assert_eq!(
        entries[0].r#type,
        vol_session::SessionEntryType::Message,
        "First entry should be a Message"
    );
    assert_eq!(
        entries[2].r#type,
        vol_session::SessionEntryType::Message,
        "Third entry should be a Message"
    );

    let count = store.get_count("session-1").await.unwrap();
    assert_eq!(count, 3, "Entry count should be 3");
}

/// Test checkpoint and resume via get_after
#[tokio::test]
async fn test_file_entry_store_checkpoint_and_resume() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn SessionEntryStore> =
        Arc::new(FileSessionEntryStore::new(tmp_dir.path()));

    // Save entries with explicit timestamps
    let mut before_cp =
        SessionEntry::new_message("session-checkpoint".to_string(), Message::user("before"));
    before_cp.created_at = 1000;

    let mut checkpoint = SessionEntry::new_checkpoint(
        "session-checkpoint".to_string(),
        vol_session::CheckpointReason::Compression,
        None,
    );
    checkpoint.created_at = 2000;

    let mut after_cp1 =
        SessionEntry::new_message("session-checkpoint".to_string(), Message::user("after 1"));
    after_cp1.created_at = 3000;

    let mut after_cp2 = SessionEntry::new_message(
        "session-checkpoint".to_string(),
        Message::assistant("response"),
    );
    after_cp2.created_at = 4000;

    store.save(before_cp).await.unwrap();
    store.save(checkpoint.clone()).await.unwrap();
    store.save(after_cp1).await.unwrap();
    store.save(after_cp2).await.unwrap();

    // Find latest checkpoint
    let cp = store.find_latest_checkpoint("session-checkpoint").await.unwrap().unwrap();
    assert_eq!(cp.r#type, vol_session::SessionEntryType::Checkpoint);

    // Get entries after checkpoint (>= so includes checkpoint itself)
    let resumed = store.get_after("session-checkpoint", cp.created_at).await.unwrap();
    assert_eq!(resumed.len(), 3, "Should have 3 entries (checkpoint + 2 after)");
}

/// Test delete session
#[tokio::test]
async fn test_file_entry_store_delete_session() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn SessionEntryStore> =
        Arc::new(FileSessionEntryStore::new(tmp_dir.path()));

    // Save some entries
    store
        .save(SessionEntry::new_message(
            "session-delete".to_string(),
            Message::user("test"),
        ))
        .await
        .unwrap();
    store
        .save(SessionEntry::new_message(
            "session-delete".to_string(),
            Message::assistant("reply"),
        ))
        .await
        .unwrap();

    assert_eq!(store.get_count("session-delete").await.unwrap(), 2);

    // Delete session
    store.delete_session("session-delete").await.unwrap();

    // Verify all entries are gone
    let count = store.get_count("session-delete").await.unwrap();
    assert_eq!(count, 0, "Entry count should be 0 after delete");

    let entries = store.get_entries("session-delete").await.unwrap();
    assert!(entries.is_empty(), "Entries should be empty after delete");
}

/// Test JSONL file persistence across re-opens
#[tokio::test]
async fn test_file_entry_store_persistence() {
    let tmp_dir = tempfile::tempdir().unwrap();

    // Create store and save entries
    {
        let store = FileSessionEntryStore::new(tmp_dir.path());
        store
            .save(SessionEntry::new_message(
                "session-persist".to_string(),
                Message::user("persistent message"),
            ))
            .await
            .unwrap();
    }

    // Create a new store instance pointing to same location
    let store2 = FileSessionEntryStore::new(tmp_dir.path());
    let entries = store2.get_entries("session-persist").await.unwrap();

    assert_eq!(entries.len(), 1, "Should persist 1 entry across re-opens");
    assert_eq!(
        entries[0].r#type,
        vol_session::SessionEntryType::Message,
        "Entry type should be Message"
    );
}

/// Test mixed entry types
#[tokio::test]
async fn test_file_entry_store_mixed_types() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let store = FileSessionEntryStore::new(tmp_dir.path());

    let mut msg = SessionEntry::new_message("session-mixed".to_string(), Message::user("hello"));
    msg.created_at = 100;

    let mut cp = SessionEntry::new_checkpoint(
        "session-mixed".to_string(),
        vol_session::CheckpointReason::Manual,
        None,
    );
    cp.created_at = 200;

    let summary_data = vol_session::SessionEntryData::Summary {
        summary: "Session summary".to_string(),
    };
    let summary = SessionEntry {
        id: "summary-1".to_string(),
        session_id: "session-mixed".to_string(),
        created_at: 300,
        parent_id: None,
        r#type: vol_session::SessionEntryType::Summary,
        data: summary_data,
    };

    store.save(msg).await.unwrap();
    store.save(cp).await.unwrap();
    store.save(summary).await.unwrap();

    let entries = store.get_entries("session-mixed").await.unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].r#type, vol_session::SessionEntryType::Message);
    assert_eq!(entries[1].r#type, vol_session::SessionEntryType::Checkpoint);
    assert_eq!(entries[2].r#type, vol_session::SessionEntryType::Summary);
}
