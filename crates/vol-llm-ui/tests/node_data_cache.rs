use vol_llm_ui::state::NodeDataCache;

#[test]
fn test_get_returns_none_for_missing_node() {
    let cache = NodeDataCache::new();
    assert!(cache.get("nonexistent-node").is_none());
}

#[test]
fn test_get_or_insert_creates_entry() {
    let mut cache = NodeDataCache::new();
    {
        let data = cache.get_or_insert("node-1");
        assert!(data.data.is_empty());
        data.data
            .insert("key".into(), serde_json::Value::String("value".into()));
    }
    // Entry persists after mutable borrow
    let data = cache.get("node-1").expect("entry should exist");
    assert_eq!(
        data.data.get("key"),
        Some(&serde_json::Value::String("value".into()))
    );
}

#[test]
fn test_invalidate_removes_entry() {
    let mut cache = NodeDataCache::new();
    cache.get_or_insert("node-1");
    assert!(cache.get("node-1").is_some());

    cache.invalidate("node-1");
    assert!(cache.get("node-1").is_none());

    // Invalidating a non-existent key is a no-op (does not panic)
    cache.invalidate("never-existed");
}
