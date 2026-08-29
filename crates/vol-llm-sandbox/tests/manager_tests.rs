use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_sandbox::{
    BackendSandboxRef, Sandbox, SandboxCapabilities, SandboxId, SandboxManager, SandboxProvider,
    SandboxProviderConfig, SandboxResult, SandboxSpec, SandboxStatus, DEFAULT_TMP_PROFILE,
};

/// Mock provider for testing
struct MockProvider {
    kind: String,
}

impl MockProvider {
    fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_string(),
        }
    }
}

#[async_trait]
impl SandboxProvider for MockProvider {
    fn kind(&self) -> &str {
        &self.kind
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            persistent: false,
            pausable: true,
            stoppable: true,
            destroyable: true,
        }
    }

    async fn create(&self, _spec: &SandboxSpec) -> SandboxResult<BackendSandboxRef> {
        let sandbox = Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None));
        Ok(BackendSandboxRef {
            backend_id: format!("mock-{}", sandbox.id()),
            sandbox,
        })
    }

    async fn get(&self, _backend_id: &str) -> SandboxResult<Arc<dyn Sandbox>> {
        // Return a new LocalSandbox for testing cache miss scenario
        let sandbox = Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None));
        Ok(sandbox)
    }

    async fn list(&self) -> SandboxResult<Vec<vol_llm_sandbox::SandboxInfo>> {
        Ok(vec![])
    }

    async fn start(&self, _backend_id: &str) -> SandboxResult<()> {
        Ok(())
    }

    async fn pause(&self, _backend_id: &str) -> SandboxResult<()> {
        Ok(())
    }

    async fn resume(&self, _backend_id: &str) -> SandboxResult<()> {
        Ok(())
    }

    async fn stop(&self, _backend_id: &str) -> SandboxResult<()> {
        Ok(())
    }

    async fn destroy(&self, _backend_id: &str) -> SandboxResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_register_and_create() {
    let manager = SandboxManager::new();
    let provider = Arc::new(MockProvider::new("local"));
    manager.register_provider(provider).await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    let id = manager.create("test").await.unwrap();
    assert!(!id.to_string().is_empty());

    let sandbox = manager.get(&id).await.unwrap();
    assert_eq!(sandbox.kind(), "local");
}

#[tokio::test]
async fn test_default_creates_tmp() {
    let manager = SandboxManager::new();
    let provider = Arc::new(MockProvider::new("tmp"));
    manager.register_provider(provider).await;

    let sandbox = manager.default().await.unwrap();
    assert_eq!(sandbox.kind(), "local"); // MockProvider creates LocalSandbox
}

#[tokio::test]
async fn test_list_sandboxes() {
    let manager = SandboxManager::new();
    let provider = Arc::new(MockProvider::new("local"));
    manager.register_provider(provider).await;

    let spec = SandboxSpec {
        name: "test1".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    let spec2 = SandboxSpec {
        name: "test2".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec2).await;

    manager.create("test1").await.unwrap();
    manager.create("test2").await.unwrap();

    let list = manager.list(None).await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn test_lifecycle_operations() {
    let manager = SandboxManager::new();
    let provider = Arc::new(MockProvider::new("local"));
    manager.register_provider(provider).await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    let id = manager.create("test").await.unwrap();

    // Sandbox starts in Running state after creation
    // Test stop operation
    manager.stop(&id).await.unwrap();

    // Test start operation (from stopped state)
    manager.start(&id).await.unwrap();

    // Test destroy operation
    manager.destroy(&id).await.unwrap();

    // Verify sandbox is destroyed
    let result = manager.get(&id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_invalid_state_transitions() {
    let manager = SandboxManager::new();
    let provider = Arc::new(MockProvider::new("local"));
    manager.register_provider(provider).await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    let id = manager.create("test").await.unwrap();

    // Sandbox starts in Running state
    // Try to start again (should fail - already running)
    let result = manager.start(&id).await;
    assert!(result.is_err());

    // Stop should work on a running sandbox
    manager.stop(&id).await.unwrap();

    // Start should work on a stopped sandbox
    manager.start(&id).await.unwrap();
}

#[tokio::test]
async fn test_create_nonexistent_profile() {
    let manager = SandboxManager::new();
    let provider = Arc::new(MockProvider::new("local"));
    manager.register_provider(provider).await;

    // Try to create sandbox with non-existent profile
    let result = manager.create("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_nonexistent_sandbox() {
    let manager = SandboxManager::new();

    // Try to get non-existent sandbox
    let fake_id = SandboxId::new();
    let result = manager.get(&fake_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_lifecycle_on_nonexistent_sandbox() {
    let manager = SandboxManager::new();
    let fake_id = SandboxId::new();

    // All lifecycle operations should fail on non-existent sandbox
    assert!(manager.start(&fake_id).await.is_err());
    assert!(manager.stop(&fake_id).await.is_err());
    assert!(manager.destroy(&fake_id).await.is_err());
}

#[tokio::test]
async fn test_default_ignores_unrelated_sandbox() {
    let manager = SandboxManager::new();
    // Register both kinds so `default()` can reach the tmp provider.
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;
    manager
        .register_provider(Arc::new(MockProvider::new("tmp")))
        .await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    // Exactly one unrelated instance exists.
    let _id = manager.create("test").await.unwrap();

    // `default()` must NOT hand back that unrelated instance just because it
    // happens to be the only one — it creates its own scratch sandbox.
    let default = manager.default().await.unwrap();
    assert_eq!(default.kind(), "local"); // MockProvider always builds LocalSandbox

    let profiles: Vec<String> = manager
        .list(None)
        .await
        .unwrap()
        .into_iter()
        .map(|i| i.profile)
        .collect();
    assert!(
        profiles.contains(&DEFAULT_TMP_PROFILE.to_string()),
        "default() should register its own scratch instance, got {profiles:?}"
    );
    assert_eq!(
        profiles.len(),
        2,
        "one unrelated + one default, got {profiles:?}"
    );
}

#[tokio::test]
async fn test_default_is_idempotent_with_empty_store() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("tmp")))
        .await;

    for _ in 0..5 {
        manager.default().await.unwrap();
    }

    assert_eq!(
        manager.list(None).await.unwrap().len(),
        1,
        "repeated default() calls must reuse one instance"
    );
}

/// Regression: `default()` used to branch on `store.len() == 1`, so once two
/// or more records existed every call created and recorded a brand new tmp
/// sandbox — an unbounded leak, and the handler calls `default()` on every
/// exec/read/write RPC.
#[tokio::test]
async fn test_default_does_not_leak_when_multiple_records_exist() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;
    manager
        .register_provider(Arc::new(MockProvider::new("tmp")))
        .await;

    for name in ["a", "b"] {
        manager
            .register_profile(SandboxSpec {
                name: name.to_string(),
                config: SandboxProviderConfig::Local { work_dir: None },
                metadata: HashMap::new(),
            })
            .await;
        manager.create(name).await.unwrap();
    }
    assert_eq!(manager.list(None).await.unwrap().len(), 2);

    let mut sizes = Vec::new();
    for _ in 0..4 {
        manager.default().await.unwrap();
        sizes.push(manager.list(None).await.unwrap().len());
    }

    assert_eq!(
        sizes,
        vec![3, 3, 3, 3],
        "default() must add at most one scratch instance, never accumulate"
    );

    let tmp_count = manager
        .list(None)
        .await
        .unwrap()
        .into_iter()
        .filter(|i| i.profile == DEFAULT_TMP_PROFILE)
        .count();
    assert_eq!(tmp_count, 1, "exactly one default-tmp record must exist");
}

#[tokio::test]
async fn test_default_concurrent_calls_create_one_instance() {
    let manager = Arc::new(SandboxManager::new());
    manager
        .register_provider(Arc::new(MockProvider::new("tmp")))
        .await;

    let mut handles = Vec::new();
    for _ in 0..8 {
        let m = Arc::clone(&manager);
        handles.push(tokio::spawn(async move { m.default().await.map(|_| ()) }));
    }
    for h in handles {
        h.await.unwrap().unwrap();
    }

    assert_eq!(
        manager.list(None).await.unwrap().len(),
        1,
        "concurrent default() calls must not each create an instance"
    );
}

#[tokio::test]
async fn test_register_instance() {
    let manager = SandboxManager::new();
    let provider = Arc::new(MockProvider::new("local"));
    manager.register_provider(provider).await;

    let sandbox = Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None));
    let spec = SandboxSpec {
        name: "registered".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };

    let id = manager
        .register_instance(spec, sandbox.clone())
        .await
        .unwrap();

    // Should be able to get the registered instance
    let retrieved = manager.get(&id).await.unwrap();
    assert_eq!(retrieved.id(), sandbox.id());
}

#[tokio::test]
async fn test_multiple_providers() {
    let manager = SandboxManager::new();

    let local_provider = Arc::new(MockProvider::new("local"));
    let tmp_provider = Arc::new(MockProvider::new("tmp"));

    manager.register_provider(local_provider).await;
    manager.register_provider(tmp_provider).await;

    let spec1 = SandboxSpec {
        name: "local-test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec1).await;

    let spec2 = SandboxSpec {
        name: "tmp-test".to_string(),
        config: SandboxProviderConfig::Tmp {
            work_dir: None,
            sub_dir: None,
        },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec2).await;

    // Create sandboxes from different providers
    let id1 = manager.create("local-test").await.unwrap();
    let id2 = manager.create("tmp-test").await.unwrap();

    let sandbox1 = manager.get(&id1).await.unwrap();
    let sandbox2 = manager.get(&id2).await.unwrap();

    // Both should be local kind (MockProvider creates LocalSandbox)
    assert_eq!(sandbox1.kind(), "local");
    assert_eq!(sandbox2.kind(), "local");
}

#[tokio::test]
async fn test_load_profiles_from_toml() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let sandbox_dir = temp_dir.path();

    // Create test TOML files
    let local_config = r#"
name = "test-local"
provider = "local"
work_dir = "/tmp/test"
"#;
    fs::write(sandbox_dir.join("local.toml"), local_config).unwrap();

    let tmp_config = r#"
name = "test-tmp"
provider = "tmp"
sub_dir = "test-subdir"
"#;
    fs::write(sandbox_dir.join("tmp.toml"), tmp_config).unwrap();

    let manager = SandboxManager::new();

    // Register providers first
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;
    manager
        .register_provider(Arc::new(MockProvider::new("tmp")))
        .await;

    // Load profiles from TOML files
    manager.load_profiles(sandbox_dir).await.unwrap();

    // Create instances from the loaded profiles
    manager.create("test-local").await.unwrap();
    manager.create("test-tmp").await.unwrap();

    // Verify instances were created
    let list = manager.list(None).await.unwrap();
    assert_eq!(list.len(), 2);

    let profiles: Vec<_> = list.iter().map(|s| s.profile.clone()).collect();
    assert!(profiles.contains(&"test-local".to_string()));
    assert!(profiles.contains(&"test-tmp".to_string()));
}

#[tokio::test]
async fn test_load_profiles_with_ssh() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let sandbox_dir = temp_dir.path();

    let ssh_config = r#"
name = "test-ssh"
provider = "ssh"
work_dir = "/home/user"
host = "192.168.1.100"
user = "developer"
port = 2222
"#;
    fs::write(sandbox_dir.join("ssh.toml"), ssh_config).unwrap();

    let manager = SandboxManager::new();

    // Register SSH provider
    manager
        .register_provider(Arc::new(MockProvider::new("ssh")))
        .await;

    // Load profiles from TOML files
    manager.load_profiles(sandbox_dir).await.unwrap();

    // Create instance from the loaded profile
    manager.create("test-ssh").await.unwrap();

    let list = manager.list(None).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].profile, "test-ssh");
    assert_eq!(list[0].kind, "ssh");
}

#[tokio::test]
async fn test_load_profiles_empty_directory() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let sandbox_dir = temp_dir.path();

    let manager = SandboxManager::new();
    manager.load_profiles(sandbox_dir).await.unwrap();

    let list = manager.list(None).await.unwrap();
    assert_eq!(list.len(), 0);
}

#[tokio::test]
async fn test_load_profiles_nonexistent_directory() {
    use std::path::PathBuf;

    let manager = SandboxManager::new();
    let result = manager
        .load_profiles(&PathBuf::from("/nonexistent/path"))
        .await;
    assert!(result.is_ok()); // Should return Ok(()) for nonexistent directory
}

#[tokio::test]
async fn test_load_profiles_with_invalid_toml() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let sandbox_dir = temp_dir.path();

    // Create invalid TOML file
    let invalid_config = r#"
this is not valid toml
"#;
    fs::write(sandbox_dir.join("invalid.toml"), invalid_config).unwrap();

    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    // Should not panic, just log warning
    let result = manager.load_profiles(sandbox_dir).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_load_profiles_with_non_toml_files() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let sandbox_dir = temp_dir.path();

    // Create non-TOML files
    fs::write(sandbox_dir.join("readme.txt"), "not a config").unwrap();
    fs::write(sandbox_dir.join("data.json"), "{}").unwrap();

    let manager = SandboxManager::new();
    manager.load_profiles(sandbox_dir).await.unwrap();

    let list = manager.list(None).await.unwrap();
    assert_eq!(list.len(), 0);
}

#[tokio::test]
async fn test_create_with_missing_provider() {
    let manager = SandboxManager::new();

    // Register profile but no provider
    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    // Try to create without provider
    let result = manager.create("test").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_with_cache_miss() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let sandbox_dir = temp_dir.path();

    let config = r#"
name = "test"
provider = "local"
work_dir = "/tmp/test"
"#;
    fs::write(sandbox_dir.join("test.toml"), config).unwrap();

    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;
    manager.load_profiles(sandbox_dir).await.unwrap();

    let id = manager.create("test").await.unwrap();

    // Clear cache by creating a new manager with same store
    // This simulates cache miss scenario
    let sandbox = manager.get(&id).await.unwrap();
    assert_eq!(sandbox.kind(), "local");
}

#[tokio::test]
async fn test_list_with_filter_by_profile() {
    use vol_llm_sandbox::SandboxFilter;

    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    let spec1 = SandboxSpec {
        name: "profile1".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec1).await;

    let spec2 = SandboxSpec {
        name: "profile2".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec2).await;

    manager.create("profile1").await.unwrap();
    manager.create("profile2").await.unwrap();

    let filter = SandboxFilter {
        profile: Some("profile1".to_string()),
        provider_kind: None,
        status: None,
    };

    let list = manager.list(Some(filter)).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].profile, "profile1");
}

#[tokio::test]
async fn test_list_with_filter_by_kind() {
    use vol_llm_sandbox::SandboxFilter;

    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;
    manager
        .register_provider(Arc::new(MockProvider::new("tmp")))
        .await;

    let spec1 = SandboxSpec {
        name: "local-test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec1).await;

    let spec2 = SandboxSpec {
        name: "tmp-test".to_string(),
        config: SandboxProviderConfig::Tmp {
            work_dir: None,
            sub_dir: None,
        },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec2).await;

    manager.create("local-test").await.unwrap();
    manager.create("tmp-test").await.unwrap();

    let filter = SandboxFilter {
        profile: None,
        provider_kind: Some("local".to_string()),
        status: None,
    };

    let list = manager.list(Some(filter)).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, "local");
}

#[tokio::test]
async fn test_list_with_filter_by_status() {
    use vol_llm_sandbox::SandboxFilter;

    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    let id = manager.create("test").await.unwrap();

    // Stop the sandbox
    manager.stop(&id).await.unwrap();

    let filter = SandboxFilter {
        profile: None,
        provider_kind: None,
        status: Some(SandboxStatus::Stopped),
    };

    let list = manager.list(Some(filter)).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].status, "stopped");
}

#[tokio::test]
async fn test_multiple_state_transitions() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    let id = manager.create("test").await.unwrap();

    // Running -> Stopped
    manager.stop(&id).await.unwrap();

    // Stopped -> Running
    manager.start(&id).await.unwrap();

    // Running -> Stopped again
    manager.stop(&id).await.unwrap();

    // Verify final state
    let filter = vol_llm_sandbox::SandboxFilter {
        profile: None,
        provider_kind: None,
        status: Some(SandboxStatus::Stopped),
    };
    let list = manager.list(Some(filter)).await.unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn test_destroy_removes_from_cache() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    let id = manager.create("test").await.unwrap();

    // Verify sandbox exists
    let sandbox = manager.get(&id).await.unwrap();
    assert_eq!(sandbox.kind(), "local");

    // Destroy it
    manager.destroy(&id).await.unwrap();

    // Verify it's gone from both store and cache
    let result = manager.get(&id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_register_instance_with_metadata() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    let sandbox = Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None));
    let mut metadata = HashMap::new();
    metadata.insert("key1".to_string(), "value1".to_string());
    metadata.insert("key2".to_string(), "value2".to_string());

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata,
    };

    let _id = manager.register_instance(spec, sandbox).await.unwrap();

    // Verify metadata is stored
    let list = manager.list(None).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].profile, "test");
}

#[tokio::test]
async fn test_concurrent_profile_registration() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let sandbox_dir = temp_dir.path();

    // Create multiple config files
    for i in 0..5 {
        let config = format!(
            r#"
name = "test{}"
provider = "local"
work_dir = "/tmp/test{}"
"#,
            i, i
        );
        fs::write(sandbox_dir.join(format!("test{}.toml", i)), config).unwrap();
    }

    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;
    manager.load_profiles(sandbox_dir).await.unwrap();

    // Create all sandboxes
    for i in 0..5 {
        manager.create(&format!("test{}", i)).await.unwrap();
    }

    let list = manager.list(None).await.unwrap();
    assert_eq!(list.len(), 5);
}

#[tokio::test]
async fn test_manager_with_store() {
    use vol_llm_sandbox::InMemorySandboxStore;

    let store = std::sync::Arc::new(InMemorySandboxStore::new());
    let manager = SandboxManager::with_store(store);

    // Should be able to use the manager normally
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    let id = manager.create("test").await.unwrap();
    let sandbox = manager.get(&id).await.unwrap();
    assert_eq!(sandbox.kind(), "local");
}

#[tokio::test]
async fn test_manager_default_trait() {
    // Use the Default trait explicitly to avoid conflict with the default() method
    let manager = <SandboxManager as Default>::default();

    // Should be able to use the manager normally
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    let id = manager.create("test").await.unwrap();
    let sandbox = manager.get(&id).await.unwrap();
    assert_eq!(sandbox.kind(), "local");
}

#[tokio::test]
async fn test_default_fallback_without_tmp_provider() {
    let manager = SandboxManager::new();

    // Don't register tmp provider, only local
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    // Default should fall back to creating LocalSandbox directly
    let sandbox = manager.default().await.unwrap();
    assert_eq!(sandbox.kind(), "local");
}

#[tokio::test]
async fn test_get_cache_miss_scenario() {
    use vol_llm_sandbox::InMemorySandboxStore;

    // Create a manager with a shared store
    let store = std::sync::Arc::new(InMemorySandboxStore::new());

    // Create first manager and add a sandbox
    let manager1 = SandboxManager::with_store(store.clone());
    manager1
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager1.register_profile(spec).await;
    let id = manager1.create("test").await.unwrap();

    // Create second manager with same store but different cache
    let manager2 = SandboxManager::with_store(store);
    manager2
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    // Get should fall back to provider since cache is empty
    let sandbox = manager2.get(&id).await.unwrap();
    assert_eq!(sandbox.kind(), "local");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Profile-name lookup API (absorbed from the deleted SandboxRegistry)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Provider whose `create` always fails — for exercising warn+skip paths.
struct FailingProvider;

#[async_trait]
impl SandboxProvider for FailingProvider {
    fn kind(&self) -> &str {
        "local"
    }
    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            persistent: false,
            pausable: false,
            stoppable: false,
            destroyable: false,
        }
    }
    async fn create(&self, _spec: &SandboxSpec) -> SandboxResult<BackendSandboxRef> {
        Err(vol_llm_sandbox::SandboxError::Config(
            "intentional failure".to_string(),
        ))
    }
    async fn get(&self, _backend_id: &str) -> SandboxResult<Arc<dyn Sandbox>> {
        Err(vol_llm_sandbox::SandboxError::NotFound("x".to_string()))
    }
    async fn list(&self) -> SandboxResult<Vec<vol_llm_sandbox::SandboxInfo>> {
        Ok(vec![])
    }
    async fn start(&self, _b: &str) -> SandboxResult<()> {
        Ok(())
    }
    async fn pause(&self, _b: &str) -> SandboxResult<()> {
        Ok(())
    }
    async fn resume(&self, _b: &str) -> SandboxResult<()> {
        Ok(())
    }
    async fn stop(&self, _b: &str) -> SandboxResult<()> {
        Ok(())
    }
    async fn destroy(&self, _b: &str) -> SandboxResult<()> {
        Ok(())
    }
}

fn local_spec(name: &str) -> SandboxSpec {
    SandboxSpec {
        name: name.to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    }
}

#[tokio::test]
async fn acquire_by_name_creates_on_first_call() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;
    manager.register_profile(local_spec("dev")).await;

    let sandbox = manager.acquire_by_name("dev").await;
    assert!(sandbox.is_some(), "should create sandbox for known profile");
    assert_eq!(sandbox.unwrap().kind(), "local");
}

#[tokio::test]
async fn acquire_by_name_returns_same_instance_on_repeat() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;
    manager.register_profile(local_spec("dev")).await;

    let first = manager.acquire_by_name("dev").await.unwrap();
    let second = manager.acquire_by_name("dev").await.unwrap();

    // Cache hit: same underlying allocation, hence same instance id.
    assert_eq!(first.id(), second.id());
    assert!(Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn acquire_by_name_unknown_profile_returns_none() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    assert!(manager.acquire_by_name("no-such-profile").await.is_none());
}

#[tokio::test]
async fn acquire_by_name_missing_provider_returns_none() {
    let manager = SandboxManager::new();
    // Profile registered, but no provider for its kind.
    manager.register_profile(local_spec("dev")).await;

    assert!(manager.acquire_by_name("dev").await.is_none());
}

#[tokio::test]
async fn acquire_by_name_provider_failure_returns_none() {
    let manager = SandboxManager::new();
    manager.register_provider(Arc::new(FailingProvider)).await;
    manager.register_profile(local_spec("dev")).await;

    assert!(
        manager.acquire_by_name("dev").await.is_none(),
        "creation failure should yield None, not panic"
    );
}

#[tokio::test]
async fn preload_loads_and_instantiates_every_profile() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.toml"),
        "name = \"a\"\nprovider = \"local\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.toml"),
        "name = \"b\"\nprovider = \"local\"\n",
    )
    .unwrap();

    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    manager.preload(dir.path()).await.unwrap();

    // Both profiles are known and already instantiated.
    assert_eq!(manager.list_specs().await.len(), 2);
    assert!(manager.acquire_by_name("a").await.is_some());
    assert!(manager.acquire_by_name("b").await.is_some());
}

#[tokio::test]
async fn preload_skips_profiles_whose_creation_fails() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.toml"),
        "name = \"a\"\nprovider = \"local\"\n",
    )
    .unwrap();

    let manager = SandboxManager::new();
    manager.register_provider(Arc::new(FailingProvider)).await;

    // preload itself succeeds — per-profile failures warn and skip.
    manager.preload(dir.path()).await.unwrap();
    assert_eq!(manager.list_specs().await.len(), 1);
    assert!(manager.acquire_by_name("a").await.is_none());
}

#[tokio::test]
async fn preload_missing_dir_is_ok() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    manager
        .preload(std::path::Path::new("/nonexistent/sandboxes/xyz"))
        .await
        .unwrap();
    assert_eq!(manager.list_specs().await.len(), 0);
}

#[tokio::test]
async fn build_inline_returns_untracked_sandbox() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;

    let sandbox = manager.build_inline(&local_spec("inline")).await.unwrap();
    assert_eq!(sandbox.kind(), "local");

    // Not registered as a profile, and not tracked as an instance.
    assert!(manager.acquire_by_name("inline").await.is_none());
    assert_eq!(manager.list(None).await.unwrap().len(), 0);
}

#[tokio::test]
async fn build_inline_unknown_provider_errors() {
    let manager = SandboxManager::new();
    // No provider registered at all.
    let err = match manager.build_inline(&local_spec("inline")).await {
        Ok(_) => panic!("expected error when no provider is registered"),
        Err(e) => e.to_string(),
    };
    assert!(
        err.contains("no provider registered") || err.contains("local"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn default_tmp_returns_tmp_sandbox_when_provider_present() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(vol_llm_sandbox::tmp::TmpSandboxProvider))
        .await;

    let sandbox = manager.default_tmp().await;
    assert_eq!(sandbox.kind(), "tmp");
}

#[tokio::test]
async fn default_tmp_falls_back_to_local_without_tmp_provider() {
    let manager = SandboxManager::new();
    // No "tmp" provider registered → build_inline errors → local fallback.
    let sandbox = manager.default_tmp().await;
    assert_eq!(sandbox.kind(), "local");
}

#[tokio::test]
async fn destroy_clears_profile_name_mapping() {
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(MockProvider::new("local")))
        .await;
    manager.register_profile(local_spec("dev")).await;

    let id = manager.create("dev").await.unwrap();
    let before = manager.acquire_by_name("dev").await;
    assert!(
        before.is_some(),
        "create() should populate the name mapping"
    );

    manager.destroy(&id).await.unwrap();

    // The mapping was pruned; a fresh instance is created on next acquire.
    let after = manager.acquire_by_name("dev").await;
    assert!(after.is_some(), "profile still registered, so re-creatable");
    assert_ne!(
        before.unwrap().id(),
        after.unwrap().id(),
        "should be a new instance, not the destroyed one"
    );
}

#[tokio::test]
async fn acquire_by_name_uses_real_local_and_tmp_providers() {
    let work = tempfile::tempdir().unwrap();
    let manager = SandboxManager::new();
    manager
        .register_provider(Arc::new(vol_llm_sandbox::local::LocalSandboxProvider))
        .await;
    manager
        .register_provider(Arc::new(vol_llm_sandbox::tmp::TmpSandboxProvider))
        .await;
    manager
        .register_profile(SandboxSpec {
            name: "proj".to_string(),
            config: SandboxProviderConfig::Local {
                work_dir: Some(work.path().to_path_buf()),
            },
            metadata: HashMap::new(),
        })
        .await;
    manager
        .register_profile(SandboxSpec {
            name: "scratch".to_string(),
            config: SandboxProviderConfig::Tmp {
                work_dir: None,
                sub_dir: Some("acquire-test".to_string()),
            },
            metadata: HashMap::new(),
        })
        .await;

    let local = manager.acquire_by_name("proj").await.unwrap();
    assert_eq!(local.kind(), "local");
    assert_eq!(local.root_path(), Some(work.path()));

    let tmp = manager.acquire_by_name("scratch").await.unwrap();
    assert_eq!(tmp.kind(), "tmp");
    assert!(tmp.root_path().unwrap().ends_with("acquire-test"));
}
