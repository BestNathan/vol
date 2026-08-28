use tempfile::tempdir;
use vol_llm_agents::coding::LocalSandbox;
use vol_llm_sandbox::Sandbox;

#[tokio::test]
async fn test_local_sandbox_new_with_path() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));
    assert_eq!(sandbox.kind(), "local");
    assert_eq!(sandbox.root_path(), Some(dir.path()));
}

#[tokio::test]
async fn test_local_sandbox_new_temp() {
    let sandbox = LocalSandbox::new(None);
    assert_eq!(sandbox.kind(), "local");
    assert!(sandbox
        .root_path()
        .unwrap()
        .to_string_lossy()
        .contains("sandbox"));
}

#[tokio::test]
async fn test_local_sandbox_new_with_existing_dir() {
    let dir = tempdir().unwrap();
    let new_path = dir.path().join("new-sandbox");
    std::fs::create_dir_all(&new_path).unwrap();
    let sandbox = LocalSandbox::new(Some(new_path.clone()));
    assert!(new_path.exists());
    assert_eq!(sandbox.root_path(), Some(new_path.as_path()));

    // caller-owned dirs are NOT deleted on drop
    drop(sandbox);
    assert!(new_path.exists());
}

#[tokio::test]
async fn test_local_sandbox_new_existing_dir_preserved() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));
    assert!(dir.path().exists());

    // caller-owned dirs NOT deleted on drop
    drop(sandbox);
    assert!(dir.path().exists());
}

#[tokio::test]
async fn test_local_sandbox_resolve_path() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));

    let resolved = sandbox.resolve_path("Cargo.toml").unwrap();
    assert_eq!(resolved, dir.path().join("Cargo.toml"));

    let resolved = sandbox.resolve_path("src/main.rs").unwrap();
    assert_eq!(resolved, dir.path().join("src/main.rs"));

    // Absolute paths are rejected (sandbox escape prevention)
    assert!(sandbox.resolve_path("/etc/passwd").is_err());
}

#[tokio::test]
async fn test_local_sandbox_resolve_path_traversal_blocked() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));

    assert!(sandbox.resolve_path("../escape.txt").is_err());
    assert!(sandbox.resolve_path("../../etc/passwd").is_err());
    assert!(sandbox.resolve_path("foo/../../escape.txt").is_err());
}

#[tokio::test]
async fn test_local_sandbox_temp_cleanup_on_drop() {
    let path;
    {
        let sandbox = LocalSandbox::new(None);
        path = sandbox.root_path().unwrap().to_path_buf();
        // Create the temp dir so we can verify it's cleaned up
        std::fs::create_dir_all(&path).unwrap();
        assert!(path.exists());
        // sandbox is dropped here — temp dir should be removed
    }
    assert!(!path.exists());
}
