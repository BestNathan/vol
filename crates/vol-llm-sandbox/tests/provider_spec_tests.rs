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
            work_dir: None,
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

#[test]
fn test_sandbox_spec_ssh() {
    let spec = SandboxSpec {
        name: "remote-dev".to_string(),
        config: SandboxProviderConfig::Ssh {
            host: "192.168.1.100".to_string(),
            user: "developer".to_string(),
            work_dir: PathBuf::from("/home/developer/workspace"),
            port: 2222,
            key_path: Some(PathBuf::from("/home/user/.ssh/id_rsa")),
            identity_file: None,
            passphrase: None,
            known_hosts_file: None,
            host_key: None,
            idle_timeout_secs: 300,
            connect_timeout_secs: 10,
        },
        metadata: HashMap::new(),
    };

    assert_eq!(spec.name, "remote-dev");
    assert_eq!(spec.provider(), "ssh");
    let ssh = spec.config.as_ssh().unwrap();
    assert_eq!(ssh.host, "192.168.1.100");
    assert_eq!(ssh.user, "developer");
    assert_eq!(ssh.work_dir, PathBuf::from("/home/developer/workspace"));
    assert_eq!(ssh.port, 2222);
    assert_eq!(ssh.key_path, Some(PathBuf::from("/home/user/.ssh/id_rsa")));
}

#[test]
fn test_sandbox_spec_ssh_defaults() {
    let spec = SandboxSpec {
        name: "remote".to_string(),
        config: SandboxProviderConfig::Ssh {
            host: "example.com".to_string(),
            user: "user".to_string(),
            work_dir: PathBuf::from("/workspace"),
            port: 22,
            key_path: None,
            identity_file: None,
            passphrase: None,
            known_hosts_file: None,
            host_key: None,
            idle_timeout_secs: 300,
            connect_timeout_secs: 10,
        },
        metadata: HashMap::new(),
    };

    let ssh = spec.config.as_ssh().unwrap();
    assert_eq!(ssh.host, "example.com");
    assert_eq!(ssh.port, 22);
    assert_eq!(ssh.key_path, None);
}

#[test]
fn test_sandbox_spec_with_metadata() {
    let mut metadata = HashMap::new();
    metadata.insert("env".to_string(), "production".to_string());
    metadata.insert("team".to_string(), "backend".to_string());

    let spec = SandboxSpec {
        name: "prod-sandbox".to_string(),
        config: SandboxProviderConfig::Local {
            work_dir: Some(PathBuf::from("/opt/app")),
        },
        metadata,
    };

    assert_eq!(spec.metadata.get("env"), Some(&"production".to_string()));
    assert_eq!(spec.metadata.get("team"), Some(&"backend".to_string()));
}

#[test]
fn test_provider_config_type_matching() {
    let local_spec = SandboxSpec {
        name: "local".to_string(),
        config: SandboxProviderConfig::Local { work_dir: None },
        metadata: HashMap::new(),
    };
    assert!(local_spec.config.as_local().is_some());
    assert!(local_spec.config.as_tmp().is_none());
    assert!(local_spec.config.as_ssh().is_none());

    let tmp_spec = SandboxSpec {
        name: "tmp".to_string(),
        config: SandboxProviderConfig::Tmp {
            work_dir: None,
            sub_dir: None,
        },
        metadata: HashMap::new(),
    };
    assert!(tmp_spec.config.as_local().is_none());
    assert!(tmp_spec.config.as_tmp().is_some());
    assert!(tmp_spec.config.as_ssh().is_none());

    let ssh_spec = SandboxSpec {
        name: "ssh".to_string(),
        config: SandboxProviderConfig::Ssh {
            host: "host".to_string(),
            user: "user".to_string(),
            work_dir: PathBuf::from("/tmp"),
            port: 22,
            key_path: None,
            identity_file: None,
            passphrase: None,
            known_hosts_file: None,
            host_key: None,
            idle_timeout_secs: 300,
            connect_timeout_secs: 10,
        },
        metadata: HashMap::new(),
    };
    assert!(ssh_spec.config.as_local().is_none());
    assert!(ssh_spec.config.as_tmp().is_none());
    assert!(ssh_spec.config.as_ssh().is_some());
}
