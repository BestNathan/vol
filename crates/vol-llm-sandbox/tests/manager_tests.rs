use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_sandbox::{
    BackendSandboxRef, Sandbox, SandboxCapabilities, SandboxId, SandboxManager, SandboxProvider,
    SandboxProviderConfig, SandboxResult, SandboxSpec, SandboxStatus,
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

    async fn create(&self, spec: &SandboxSpec) -> SandboxResult<BackendSandboxRef> {
        let sandbox = Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None));
        Ok(BackendSandboxRef {
            backend_id: format!("mock-{}", sandbox.id()),
            sandbox,
        })
    }

    async fn get(&self, backend_id: &str) -> SandboxResult<Arc<dyn Sandbox>> {
        Err(vol_llm_sandbox::SandboxError::NotFound(
            backend_id.to_string(),
        ))
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
async fn test_default_with_existing_sandbox() {
    let manager = SandboxManager::new();
    let provider = Arc::new(MockProvider::new("local"));
    manager.register_provider(provider).await;

    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    manager.register_profile(spec).await;

    // Create one sandbox
    let id = manager.create("test").await.unwrap();

    // Default should return the existing sandbox
    let default = manager.default().await.unwrap();
    assert_eq!(default.kind(), "local");
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
        config: SandboxProviderConfig::Tmp { sub_dir: None },
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
