use std::sync::Arc;
use tempfile::tempdir;
use vol_llm_agents::coding::LocalSandbox;
use vol_llm_sandbox::SandboxRef;
use vol_llm_tool::ToolContext;

#[test]
fn test_resolve_path_without_sandbox() {
    let ctx = ToolContext::for_test();
    let path = ctx.resolve_path("Cargo.toml").unwrap();
    // Sandbox always resolves to an absolute path; for_test() roots at "/"
    assert!(path.is_absolute());
    assert!(path.ends_with("Cargo.toml"));
}

#[test]
fn test_resolve_path_with_sandbox() {
    let dir = tempdir().unwrap();
    let sandbox: SandboxRef = Arc::new(LocalSandbox::new(Some(dir.path().to_path_buf())));
    let ctx = ToolContext::for_test().with_sandbox(sandbox);

    let path = ctx.resolve_path("src/main.rs").unwrap();
    assert_eq!(path, dir.path().join("src/main.rs"));
}

#[test]
fn test_resolve_path_traversal_blocked() {
    let dir = tempdir().unwrap();
    let sandbox: SandboxRef = Arc::new(LocalSandbox::new(Some(dir.path().to_path_buf())));
    let ctx = ToolContext::for_test().with_sandbox(sandbox);

    let result = ctx.resolve_path("../escape.txt");
    assert!(result.is_err());
}
