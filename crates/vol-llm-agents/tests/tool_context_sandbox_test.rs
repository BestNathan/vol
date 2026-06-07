use vol_llm_tool::ToolContext;
use vol_llm_sandbox::SandboxRef;
use vol_llm_agents::coding::LocalSandbox;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_resolve_path_without_sandbox() {
    let ctx = ToolContext::default();
    let path = ctx.resolve_path("Cargo.toml").unwrap();
    assert_eq!(path, std::path::PathBuf::from("Cargo.toml"));
}

#[test]
fn test_resolve_path_with_sandbox() {
    let dir = tempdir().unwrap();
    let sandbox: SandboxRef = Arc::new(LocalSandbox::new(Some(dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);

    let path = ctx.resolve_path("src/main.rs").unwrap();
    assert_eq!(path, dir.path().join("src/main.rs"));
}

#[test]
fn test_resolve_path_traversal_blocked() {
    let dir = tempdir().unwrap();
    let sandbox: SandboxRef = Arc::new(LocalSandbox::new(Some(dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);

    let result = ctx.resolve_path("../escape.txt");
    assert!(result.is_err());
}
