use vol_llm_memory::{
    InMemoryStore, KeywordRetriever, MemoryFilter, MemoryItem, MemoryKind, MemoryManager,
    MemoryRetriever, MemoryStore,
};

#[tokio::test]
async fn test_add_and_get_memory() {
    let store = InMemoryStore::new();
    let item = MemoryItem::new(MemoryKind::UserPreference, "User prefers Rust");
    let id = store.add(item).await.unwrap();
    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.content, "User prefers Rust");
    assert_eq!(retrieved.kind, MemoryKind::UserPreference);
}

#[tokio::test]
async fn test_remove_memory() {
    let store = InMemoryStore::new();
    let item = MemoryItem::new(MemoryKind::ProjectFact, "Uses TDengine");
    let id = store.add(item).await.unwrap();
    assert!(store.remove(&id).await.unwrap());
    assert!(store.get(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_update_memory() {
    let store = InMemoryStore::new();
    let item = MemoryItem::new(MemoryKind::Experience, "Old content");
    let id = store.add(item).await.unwrap();
    let mut updated = store.get(&id).await.unwrap().unwrap();
    updated.content = "New content".to_string();
    store.update(updated).await.unwrap();
    let retrieved = store.get(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.content, "New content");
}

#[tokio::test]
async fn test_update_nonexistent_memory() {
    let store = InMemoryStore::new();
    let item = MemoryItem::new(MemoryKind::Experience, "test");
    let result = store.update(item).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_with_filter() {
    let store = InMemoryStore::new();
    store.add(MemoryItem::new(MemoryKind::UserPreference, "Prefers Rust").with_tags(vec!["rust".to_string()])).await;
    store.add(MemoryItem::new(MemoryKind::ProjectFact, "Uses TDengine").with_tags(vec!["database".to_string()])).await;
    let filter = MemoryFilter::new().kinds(vec![MemoryKind::UserPreference]);
    let results = store.list(filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, MemoryKind::UserPreference);
}

#[tokio::test]
async fn test_remove_many() {
    let store = InMemoryStore::new();
    store.add(MemoryItem::new(MemoryKind::Experience, "exp1")).await;
    store.add(MemoryItem::new(MemoryKind::Experience, "exp2")).await;
    store.add(MemoryItem::new(MemoryKind::UserPreference, "pref1")).await;
    let filter = MemoryFilter::new().kinds(vec![MemoryKind::Experience]);
    let removed = store.remove_many(filter).await.unwrap();
    assert_eq!(removed, 2);
    let all = store.list(MemoryFilter::new()).await.unwrap();
    assert_eq!(all.len(), 1);
}

#[tokio::test]
async fn test_keyword_retriever() {
    let store = InMemoryStore::new();
    store.add(MemoryItem::new(MemoryKind::ProjectFact, "This project uses TDengine database")).await;
    store.add(MemoryItem::new(MemoryKind::UserPreference, "User likes Rust programming")).await;
    let retriever = KeywordRetriever::new(Box::new(store));
    let results = retriever.retrieve("TDengine database", 5).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, MemoryKind::ProjectFact);
}

#[tokio::test]
async fn test_keyword_retriever_with_filter() {
    let store = InMemoryStore::new();
    store.add(MemoryItem::new(MemoryKind::ProjectFact, "Uses Rust")).await;
    store.add(MemoryItem::new(MemoryKind::UserPreference, "Rust is great")).await;
    let retriever = KeywordRetriever::new(Box::new(store));
    let filter = MemoryFilter::new().kinds(vec![MemoryKind::UserPreference]);
    let results = retriever.retrieve_with_filter("Rust", 5, filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, MemoryKind::UserPreference);
}

#[tokio::test]
async fn test_memory_manager_add_search() {
    let store = InMemoryStore::new();
    let retriever = KeywordRetriever::new(Box::new(store.clone()));
    let manager = MemoryManager::new(Box::new(store), Box::new(retriever));
    let item = MemoryItem::new(MemoryKind::UserPreference, "User prefers Rust for backend development")
        .with_tags(vec!["rust".to_string(), "backend".to_string()]);
    manager.add(item).await.unwrap();
    let results = manager.search("Rust backend", 5).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_memory_manager_inject_context() {
    let store = InMemoryStore::new();
    let retriever = KeywordRetriever::new(Box::new(store.clone()));
    let manager = MemoryManager::new(Box::new(store), Box::new(retriever));
    manager.add(MemoryItem::new(MemoryKind::ProjectFact, "Uses TDengine")).await;
    let injected = manager.inject_context("TDengine", 5).await.unwrap();
    assert!(injected.contains("Uses TDengine"));
    assert!(injected.contains("ProjectFact"));
}

#[tokio::test]
async fn test_memory_manager_inject_context_empty() {
    let store = InMemoryStore::new();
    let retriever = KeywordRetriever::new(Box::new(store.clone()));
    let manager = MemoryManager::new(Box::new(store), Box::new(retriever));
    let injected = manager.inject_context("nonexistent", 5).await.unwrap();
    assert!(injected.is_empty());
}

#[tokio::test]
async fn test_memory_filter_matches() {
    let item = MemoryItem::new(MemoryKind::Experience, "test content")
        .with_tags(vec!["rust".to_string()])
        .with_importance(0.8);

    assert!(MemoryFilter::new().kinds(vec![MemoryKind::Experience]).matches(&item));
    assert!(!MemoryFilter::new().kinds(vec![MemoryKind::UserPreference]).matches(&item));
    assert!(MemoryFilter::new().tags(vec!["rust".to_string()]).matches(&item));
    assert!(!MemoryFilter::new().tags(vec!["python".to_string()]).matches(&item));
    assert!(MemoryFilter::new().min_importance(0.5).matches(&item));
    assert!(!MemoryFilter::new().min_importance(0.9).matches(&item));
}

#[tokio::test]
async fn test_memory_item_builder() {
    let item = MemoryItem::new(MemoryKind::ConversationSummary, "Good session")
        .with_tags(vec!["productive".to_string()])
        .with_importance(0.9);
    assert_eq!(item.tags, vec!["productive"]);
    assert!((item.importance - 0.9).abs() < f32::EPSILON);
}
