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
