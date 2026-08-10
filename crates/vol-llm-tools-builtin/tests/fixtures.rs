//! Shared test fixtures for builtin-tools sandbox integration tests.
//!
//! Every fixture creates a `ToolContext` backed by a `LocalSandbox` rooted
//! at a `TempDir` — never at `/`. This simulates the real agent execution
//! environment where tools operate within a bounded sandbox.

// Test fixtures panic on setup failure by design; the crate inherits the
// workspace's deny-level unwrap/expect lints, which apply to test targets too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_sandbox::SandboxRef;
use vol_llm_tool::ToolContext;

/// Create a `ToolContext` backed by a sandbox rooted at a unique temp directory.
///
/// Returns `(context, temp_dir)` — keep `temp_dir` alive for the test duration.
/// The sandbox root is `temp_dir.path()`.
pub fn sandbox_in_tempdir() -> (ToolContext, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir for sandbox");
    let sandbox: SandboxRef = Arc::new(LocalSandbox::new(Some(temp_dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);
    (ctx, temp_dir)
}

/// Create a `ToolContext` with sandbox rooted at `tempdir/<subdir>`.
///
/// Simulates an agent whose `working_dir` is a subdirectory of the sandbox root.
/// The subdirectory is created on disk.
///
/// Note: each integration-test target compiles `fixtures.rs` independently, so
/// functions not used by a given target are dead code in that target. Consumed
/// by the centralized agent-context tests (SDD task 13).
#[allow(dead_code)]
pub fn sandbox_in_subdir(subdir: &str) -> (ToolContext, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir for sandbox");
    let subdir_path = temp_dir.path().join(subdir);
    std::fs::create_dir_all(&subdir_path).expect("Failed to create subdir");
    let sandbox: SandboxRef = Arc::new(LocalSandbox::new(Some(subdir_path)));
    let ctx = ToolContext::default().with_sandbox(sandbox);
    (ctx, temp_dir)
}

/// Populate files in the temp directory backing a sandbox.
///
/// Each tuple is `(relative_path_from_temp_dir_root, content)`.
/// Parent directories are auto-created.
///
/// Note: writes directly to the temp dir via `std::fs`, not through the sandbox
/// trait. The sandbox root is `temp_dir.path()`, so the sandbox's `read_file`
/// will see these files at the same paths.
#[allow(dead_code)]
pub fn populate_files(temp_dir: &TempDir, files: &[(&str, &str)]) {
    for (rel_path, content) in files {
        let full = temp_dir.path().join(rel_path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }
}

/// Create a `ToolContext` with an `AgentDef` — simulates real agent execution.
///
/// The working_dir in the AgentDef is informational; actual path resolution
/// goes through the sandbox.
///
/// Note: each integration-test target compiles `fixtures.rs` independently, so
/// functions not used by a given target are dead code in that target. Consumed
/// by the centralized agent-context tests (SDD task 13).
#[allow(dead_code)]
pub fn agent_context(sandbox: SandboxRef, name: &str, working_dir: Option<&str>) -> ToolContext {
    let agent_def = vol_llm_core::AgentDef {
        id: format!("test:{name}"),
        name: name.to_string(),
        r#type: "test-agent".to_string(),
        description: "test agent".to_string(),
        scope: vol_llm_core::AgentScope::Repo,
        tools: None,
        disallowed_tools: None,
        model: None,
        max_iterations: None,
        max_history_messages: None,
        prompt: "You are a test agent.".to_string(),
        working_dir: working_dir.map(PathBuf::from),
        context_files: vec![],
        sandbox: None,
        tool_config: None,
        mcps: None,
        skills: None,
    };
    ToolContext::default()
        .with_sandbox(sandbox)
        .with_agent_def(agent_def)
}
