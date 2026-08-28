// Integration test: verify sandbox profiles are loaded from TOML files
use std::sync::Arc;
use vol_llm_sandbox::SandboxManager;

#[tokio::test]
async fn test_load_sandbox_profiles_integration() {
    // Create a temporary directory with sandbox configs
    let temp_dir = tempfile::tempdir().unwrap();
    let sandbox_dir = temp_dir.path().join(".agents/sandboxes");
    std::fs::create_dir_all(&sandbox_dir).unwrap();

    // Write a local sandbox config
    let local_config = r#"
name = "test-local"
provider = "local"
work_dir = "/tmp/test"
"#;
    std::fs::write(sandbox_dir.join("local.toml"), local_config).unwrap();

    // Write a tmp sandbox config
    let tmp_config = r#"
name = "test-tmp"
provider = "tmp"
sub_dir = "test-subdir"
"#;
    std::fs::write(sandbox_dir.join("tmp.toml"), tmp_config).unwrap();

    // Create SandboxManager and load profiles
    let manager = Arc::new(SandboxManager::new());
    manager
        .register_provider(Arc::new(vol_llm_sandbox::local::LocalSandboxProvider))
        .await;
    manager
        .register_provider(Arc::new(vol_llm_sandbox::tmp::TmpSandboxProvider))
        .await;

    manager.load_profiles(&sandbox_dir).await.unwrap();

    // Create instances from loaded profiles
    let id1 = manager.create("test-local").await.unwrap();
    let id2 = manager.create("test-tmp").await.unwrap();

    // Verify instances were created
    let list = manager.list(None).await.unwrap();
    assert_eq!(list.len(), 2);

    let profiles: Vec<_> = list.iter().map(|s| s.profile.clone()).collect();
    assert!(profiles.contains(&"test-local".to_string()));
    assert!(profiles.contains(&"test-tmp".to_string()));

    // Verify we can get the sandboxes
    let sandbox1 = manager.get(&id1).await.unwrap();
    let sandbox2 = manager.get(&id2).await.unwrap();
    assert_eq!(sandbox1.kind(), "local");
    assert_eq!(sandbox2.kind(), "tmp");
}

#[tokio::test]
async fn test_load_sandbox_profiles_with_ssh() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sandbox_dir = temp_dir.path().join(".agents/sandboxes");
    std::fs::create_dir_all(&sandbox_dir).unwrap();

    // Write an SSH sandbox config
    let ssh_config = r#"
name = "test-ssh"
provider = "ssh"
work_dir = "/home/user"
host = "192.168.1.100"
user = "developer"
port = 2222
"#;
    std::fs::write(sandbox_dir.join("ssh.toml"), ssh_config).unwrap();

    let manager = Arc::new(SandboxManager::new());

    // Create SSH provider with a config
    let ssh_provider = vol_llm_sandbox::ssh::SSHSandboxProvider::new();
    ssh_provider.add_config(vol_llm_sandbox::registry::SshConfig {
        host: "192.168.1.100".to_string(),
        port: 2222,
        user: "developer".to_string(),
        identity_file: "/tmp/test_key".to_string(),
        passphrase: None,
        known_hosts_file: None,
        host_key: None,
        idle_timeout_secs: 300,
        connect_timeout_secs: 10,
    });

    manager.register_provider(Arc::new(ssh_provider)).await;

    manager.load_profiles(&sandbox_dir).await.unwrap();

    // Create instance from loaded profile
    let id = manager.create("test-ssh").await.unwrap();

    let list = manager.list(None).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].profile, "test-ssh");
    assert_eq!(list[0].kind, "ssh");

    let sandbox = manager.get(&id).await.unwrap();
    assert_eq!(sandbox.kind(), "ssh");
}
