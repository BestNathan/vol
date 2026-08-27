use chrono::Utc;
use std::collections::HashMap;
use vol_llm_sandbox::{
    InMemorySandboxStore, SandboxFilter, SandboxId, SandboxRecord, SandboxStatus, SandboxStore,
};

#[tokio::test]
async fn test_insert_and_get() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();
    let record = SandboxRecord {
        id: id.clone(),
        profile: "test".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record.clone()).await.unwrap();
    let retrieved = store.get(&id).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, id);
}

#[tokio::test]
async fn test_list_with_filter() {
    let store = InMemorySandboxStore::new();

    let record1 = SandboxRecord {
        id: SandboxId::new(),
        profile: "coding".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    let record2 = SandboxRecord {
        id: SandboxId::new(),
        profile: "testing".to_string(),
        provider_kind: "tmp".to_string(),
        backend_id: "backend_2".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record1).await.unwrap();
    store.insert(record2).await.unwrap();

    let filter = SandboxFilter {
        profile: Some("coding".to_string()),
        provider_kind: None,
        status: None,
    };

    let results = store.list(Some(filter)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].profile, "coding");
}

#[tokio::test]
async fn test_update_status() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();
    let record = SandboxRecord {
        id: id.clone(),
        profile: "test".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Creating,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record).await.unwrap();
    store
        .update_status(&id, SandboxStatus::Running)
        .await
        .unwrap();

    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.status, SandboxStatus::Running);
}

#[tokio::test]
async fn test_delete() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();
    let record = SandboxRecord {
        id: id.clone(),
        profile: "test".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record).await.unwrap();
    store.delete(&id).await.unwrap();

    let retrieved = store.get(&id).await.unwrap();
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_list_no_filter() {
    let store = InMemorySandboxStore::new();

    let record1 = SandboxRecord {
        id: SandboxId::new(),
        profile: "test1".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    let record2 = SandboxRecord {
        id: SandboxId::new(),
        profile: "test2".to_string(),
        provider_kind: "tmp".to_string(),
        backend_id: "backend_2".to_string(),
        status: SandboxStatus::Stopped,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record1).await.unwrap();
    store.insert(record2).await.unwrap();

    // List with no filter should return all records
    let results = store.list(None).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_list_with_provider_kind_filter() {
    let store = InMemorySandboxStore::new();

    let record1 = SandboxRecord {
        id: SandboxId::new(),
        profile: "test1".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    let record2 = SandboxRecord {
        id: SandboxId::new(),
        profile: "test2".to_string(),
        provider_kind: "tmp".to_string(),
        backend_id: "backend_2".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record1).await.unwrap();
    store.insert(record2).await.unwrap();

    let filter = SandboxFilter {
        profile: None,
        provider_kind: Some("local".to_string()),
        status: None,
    };

    let results = store.list(Some(filter)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].provider_kind, "local");
}

#[tokio::test]
async fn test_list_with_status_filter() {
    let store = InMemorySandboxStore::new();

    let record1 = SandboxRecord {
        id: SandboxId::new(),
        profile: "test1".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    let record2 = SandboxRecord {
        id: SandboxId::new(),
        profile: "test2".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_2".to_string(),
        status: SandboxStatus::Stopped,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record1).await.unwrap();
    store.insert(record2).await.unwrap();

    let filter = SandboxFilter {
        profile: None,
        provider_kind: None,
        status: Some(SandboxStatus::Stopped),
    };

    let results = store.list(Some(filter)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, SandboxStatus::Stopped);
}

#[tokio::test]
async fn test_list_with_combined_filters() {
    let store = InMemorySandboxStore::new();

    let record1 = SandboxRecord {
        id: SandboxId::new(),
        profile: "coding".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    let record2 = SandboxRecord {
        id: SandboxId::new(),
        profile: "coding".to_string(),
        provider_kind: "tmp".to_string(),
        backend_id: "backend_2".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    let record3 = SandboxRecord {
        id: SandboxId::new(),
        profile: "testing".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_3".to_string(),
        status: SandboxStatus::Stopped,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record1).await.unwrap();
    store.insert(record2).await.unwrap();
    store.insert(record3).await.unwrap();

    // Filter by profile and provider_kind
    let filter = SandboxFilter {
        profile: Some("coding".to_string()),
        provider_kind: Some("local".to_string()),
        status: None,
    };

    let results = store.list(Some(filter)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].profile, "coding");
    assert_eq!(results[0].provider_kind, "local");
}

#[tokio::test]
async fn test_list_with_all_filters() {
    let store = InMemorySandboxStore::new();

    let record1 = SandboxRecord {
        id: SandboxId::new(),
        profile: "coding".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    let record2 = SandboxRecord {
        id: SandboxId::new(),
        profile: "coding".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_2".to_string(),
        status: SandboxStatus::Stopped,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record1).await.unwrap();
    store.insert(record2).await.unwrap();

    let filter = SandboxFilter {
        profile: Some("coding".to_string()),
        provider_kind: Some("local".to_string()),
        status: Some(SandboxStatus::Running),
    };

    let results = store.list(Some(filter)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, SandboxStatus::Running);
}

#[tokio::test]
async fn test_list_empty_store() {
    let store = InMemorySandboxStore::new();

    let results = store.list(None).await.unwrap();
    assert_eq!(results.len(), 0);

    let filter = SandboxFilter {
        profile: Some("test".to_string()),
        provider_kind: None,
        status: None,
    };
    let results = store.list(Some(filter)).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_update_status_nonexistent() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();

    let result = store.update_status(&id, SandboxStatus::Running).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_nonexistent() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();

    // Delete should succeed even if record doesn't exist
    let result = store.delete(&id).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_nonexistent() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();

    let result = store.get(&id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_default_store() {
    let store = InMemorySandboxStore::default();

    let id = SandboxId::new();
    let record = SandboxRecord {
        id: id.clone(),
        profile: "test".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record).await.unwrap();
    let retrieved = store.get(&id).await.unwrap();
    assert!(retrieved.is_some());
}

#[tokio::test]
async fn test_multiple_updates() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();

    let record = SandboxRecord {
        id: id.clone(),
        profile: "test".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Creating,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record).await.unwrap();

    // Multiple status updates
    store
        .update_status(&id, SandboxStatus::Starting)
        .await
        .unwrap();
    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.status, SandboxStatus::Starting);

    store
        .update_status(&id, SandboxStatus::Running)
        .await
        .unwrap();
    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.status, SandboxStatus::Running);

    store
        .update_status(&id, SandboxStatus::Stopping)
        .await
        .unwrap();
    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.status, SandboxStatus::Stopping);
}

#[tokio::test]
async fn test_insert_overwrite() {
    let store = InMemorySandboxStore::new();
    let id = SandboxId::new();

    let record1 = SandboxRecord {
        id: id.clone(),
        profile: "test1".to_string(),
        provider_kind: "local".to_string(),
        backend_id: "backend_1".to_string(),
        status: SandboxStatus::Running,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    let record2 = SandboxRecord {
        id: id.clone(),
        profile: "test2".to_string(),
        provider_kind: "tmp".to_string(),
        backend_id: "backend_2".to_string(),
        status: SandboxStatus::Stopped,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: HashMap::new(),
    };

    store.insert(record1).await.unwrap();
    store.insert(record2).await.unwrap();

    // Second insert should overwrite first
    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.profile, "test2");
    assert_eq!(retrieved.provider_kind, "tmp");
}
