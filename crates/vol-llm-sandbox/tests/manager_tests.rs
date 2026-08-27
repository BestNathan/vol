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
            pausable: false,
            stoppable: false,
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
