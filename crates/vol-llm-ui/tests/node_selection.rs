use vol_llm_ui::state::NodeDataCache;

#[test]
fn test_cached_data_persists_across_node_switches() {
    let mut cache = NodeDataCache::default();

    // Load data for node-A
    let data_a = cache.get_or_insert("node-A");
    data_a
        .data
        .insert("files".to_string(), serde_json::json!({"test": "data"}));

    // Load data for node-B
    let data_b = cache.get_or_insert("node-B");
    data_b
        .data
        .insert("files".to_string(), serde_json::json!({"other": "data"}));

    // Both should be cached
    assert!(cache.get("node-A").unwrap().data.get("files").is_some());
    assert!(cache.get("node-B").unwrap().data.get("files").is_some());

    // Values should be independent
    assert_eq!(
        cache.get("node-A").unwrap().data.get("files"),
        Some(&serde_json::json!({"test": "data"}))
    );
    assert_eq!(
        cache.get("node-B").unwrap().data.get("files"),
        Some(&serde_json::json!({"other": "data"}))
    );

    // Switch back to node-A, data should still be there
    assert!(cache.get("node-A").unwrap().data.get("files").is_some());
}

#[test]
fn test_cache_invalidate_removes_entry() {
    let mut cache = NodeDataCache::default();
    cache.get_or_insert("node-A");
    cache.invalidate("node-A");
    assert!(cache.get("node-A").is_none());
}

#[test]
fn test_cache_invalidate_is_idempotent() {
    let mut cache = NodeDataCache::default();
    // Invalidating a missing key must not panic.
    cache.invalidate("never-existed");
    assert!(cache.get("never-existed").is_none());
}

#[test]
fn test_get_or_insert_is_idempotent() {
    let mut cache = NodeDataCache::default();
    cache
        .get_or_insert("node-A")
        .data
        .insert("k".into(), serde_json::json!(1));

    // Second call must return the same entry, not overwrite it.
    let second = cache.get_or_insert("node-A");
    assert_eq!(second.data.get("k"), Some(&serde_json::json!(1)));
}

#[test]
fn test_get_mut_returns_none_for_missing() {
    let mut cache = NodeDataCache::default();
    assert!(cache.get_mut("missing").is_none());
}

#[test]
fn test_clone_produces_independent_copy() {
    let mut cache = NodeDataCache::default();
    cache
        .get_or_insert("node-A")
        .data
        .insert("x".into(), serde_json::json!("v"));

    let mut cloned = cache.clone();
    cloned
        .get_or_insert("node-A")
        .data
        .insert("x".into(), serde_json::json!("modified"));

    // Original unaffected
    assert_eq!(
        cache.get("node-A").unwrap().data.get("x"),
        Some(&serde_json::json!("v"))
    );
}
