use std::collections::HashMap;
use std::path::PathBuf;
use vol_llm_sandbox::{SandboxCapabilities, SandboxProviderConfig, SandboxSpec};

#[test]
fn test_sandbox_spec_local() {
    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local {
            work_dir: Some(PathBuf::from("/tmp/test")),
        },
        metadata: HashMap::new(),
    };

    assert_eq!(spec.name, "test");
    assert_eq!(spec.provider(), "local");
    let local = spec.config.as_local().unwrap();
    assert_eq!(local.work_dir, Some(PathBuf::from("/tmp/test")));
}

#[test]
fn test_sandbox_spec_tmp() {
    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Tmp {
            sub_dir: Some("agent_1".to_string()),
        },
        metadata: HashMap::new(),
    };

    let tmp = spec.config.as_tmp().unwrap();
    assert_eq!(tmp.sub_dir, Some("agent_1".to_string()));
}

#[test]
fn test_sandbox_spec_serde() {
    let spec = SandboxSpec {
        name: "test".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };

    let json = serde_json::to_string(&spec).unwrap();
    let deserialized: SandboxSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "test");
    assert_eq!(deserialized.provider(), "local");
}

#[test]
fn test_sandbox_capabilities_serde() {
    let caps = SandboxCapabilities {
        persistent: true,
        pausable: false,
        stoppable: true,
        destroyable: false,
    };

    let json = serde_json::to_string(&caps).unwrap();
    let deserialized: SandboxCapabilities = serde_json::from_str(&json).unwrap();
    assert!(deserialized.persistent);
    assert!(!deserialized.pausable);
    assert!(deserialized.stoppable);
    assert!(!deserialized.destroyable);
}
