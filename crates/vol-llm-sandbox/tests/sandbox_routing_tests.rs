//! Integration tests for sandbox registry and path normalization.
//!
//! Verifies SandboxRegistry::acquire, default sandbox, and normalize_path behavior.

use std::path::Path;
use std::sync::Arc;
use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_sandbox::registry::SandboxRegistry;
use vol_llm_sandbox::{normalize_path, Sandbox};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// normalize_path tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_normalize_path_simple() {
    assert_eq!(normalize_path(Path::new("foo/bar")), Path::new("foo/bar"));
}

#[test]
fn test_normalize_path_parent_traversal() {
    // "foo/../bar" → "bar"
    assert_eq!(normalize_path(Path::new("foo/../bar")), Path::new("bar"));
}

#[test]
fn test_normalize_path_multiple_parent() {
    assert_eq!(normalize_path(Path::new("a/b/../../c")), Path::new("c"));
}

#[test]
fn test_normalize_path_current_dir() {
    assert_eq!(
        normalize_path(Path::new("./foo/./bar")),
        Path::new("foo/bar")
    );
}

#[test]
fn test_normalize_path_only_curdir() {
    // "." normalizes to "." (guard prevents empty path)
    assert_eq!(normalize_path(Path::new(".")), Path::new("."));
}

#[test]
fn test_normalize_path_empty() {
    assert_eq!(normalize_path(Path::new("")), Path::new(""));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SandboxRegistry routing tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_registry_acquire_local_returns_working_sandbox() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = SandboxRegistry::load(tmp.path()).await.unwrap();

    let sandbox = registry
        .acquire("local")
        .expect("local sandbox should exist");
    assert_eq!(sandbox.name(), "local");
    assert_eq!(sandbox.kind(), "local");

    // Verify it works: write and read a file
    let root = sandbox.root_path().to_path_buf();
    sandbox
        .write_file(&root.join("test.txt"), b"hello")
        .await
        .unwrap();
    let content = sandbox
        .read_file(&root.join("test.txt"), None, None)
        .await
        .unwrap();
    assert_eq!(content, b"hello");
}

#[tokio::test]
async fn test_registry_acquire_nonexistent_returns_none() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = SandboxRegistry::load(tmp.path()).await.unwrap();
    assert!(registry.acquire("nonexistent_sandbox_xyz").is_none());
}

#[tokio::test]
async fn test_registry_default_is_local() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = SandboxRegistry::load(tmp.path()).await.unwrap();
    let default = registry.default();
    assert_eq!(default.name(), "local");
}

#[tokio::test]
async fn test_registry_names_includes_local() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = SandboxRegistry::load(tmp.path()).await.unwrap();
    assert!(registry.names().contains(&"local"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Sandbox path resolution edge cases
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_resolve_path_traversal_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let sandbox = Arc::new(LocalSandbox::new(Some(dir.path().to_path_buf())));
    sandbox.start().await.unwrap();

    let result = sandbox.resolve_path("../../../etc/passwd");
    assert!(result.is_err(), "Path traversal should be rejected");
}

#[tokio::test]
async fn test_resolve_path_relative_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let sandbox = Arc::new(LocalSandbox::new(Some(dir.path().to_path_buf())));
    sandbox.start().await.unwrap();

    let resolved = sandbox.resolve_path("subdir/file.txt").unwrap();
    assert!(resolved.starts_with(dir.path()));
    assert!(resolved.ends_with("subdir/file.txt"));
}
