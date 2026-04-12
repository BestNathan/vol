use vol_llm_agents::coding::LocalSandbox;
use vol_llm_core::Sandbox;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_local_sandbox_new_with_path() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));
    assert_eq!(sandbox.kind(), "local");
    assert_eq!(sandbox.root_path(), dir.path());
}

#[test]
fn test_local_sandbox_new_temp() {
    let sandbox = LocalSandbox::new(None);
    assert_eq!(sandbox.kind(), "local");
    assert!(sandbox.root_path().to_string_lossy().contains("sandbox"));
}

#[test]
fn test_local_sandbox_start_creates_dir() {
    let dir = tempdir().unwrap();
    let new_path = dir.path().join("new-sandbox");
    let sandbox = LocalSandbox::new(Some(new_path.clone()));
    assert!(!new_path.exists());

    sandbox.start().unwrap();
    assert!(new_path.exists());

    // caller-owned dirs are NOT deleted on cleanup (even if start created them)
    sandbox.cleanup().unwrap();
    assert!(new_path.exists());
}

#[test]
fn test_local_sandbox_start_existing_dir() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));

    sandbox.start().unwrap();
    assert!(dir.path().exists());

    sandbox.cleanup().unwrap();
    assert!(dir.path().exists()); // caller-owned dirs NOT deleted
}

#[test]
fn test_local_sandbox_resolve_path() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));

    let resolved = sandbox.resolve_path("Cargo.toml").unwrap();
    assert_eq!(resolved, dir.path().join("Cargo.toml"));

    let resolved = sandbox.resolve_path("src/main.rs").unwrap();
    assert_eq!(resolved, dir.path().join("src/main.rs"));

    // Absolute paths returned as-is
    let resolved = sandbox.resolve_path("/etc/passwd").unwrap();
    assert_eq!(resolved, PathBuf::from("/etc/passwd"));
}

#[test]
fn test_local_sandbox_resolve_path_traversal_blocked() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));

    assert!(sandbox.resolve_path("../escape.txt").is_err());
    assert!(sandbox.resolve_path("../../etc/passwd").is_err());
    assert!(sandbox.resolve_path("foo/../../escape.txt").is_err());
}

#[test]
fn test_local_sandbox_temp_cleanup() {
    let sandbox = LocalSandbox::new(None);
    sandbox.start().unwrap();
    let path = sandbox.root_path().to_path_buf();
    assert!(path.exists());

    sandbox.cleanup().unwrap();
    assert!(!path.exists());
}
