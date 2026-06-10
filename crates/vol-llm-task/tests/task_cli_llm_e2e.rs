//! End-to-end test: real LLM using TaskCliTool to manage tasks.
//!
//! Requires model service at http://192.168.2.162:31693. Skip by default.
//!
//! Run with: cargo test -p vol-llm-task --test task_cli_llm_e2e -- --ignored --nocapture

use std::sync::Arc;

use vol_llm_agent::react::{AgentConfigBuilder, ReActAgent};
use vol_llm_core::LLMProvider;
use vol_llm_provider::{LLMConfig, LLMProviderConfig, LLMProviderRegistry, Secret};
use vol_llm_task::tools::TaskCliTool;
use vol_llm_task::{InMemoryTaskStore, TaskStore};

/// Build an LLM client pointing at the internal model service.
fn create_test_llm() -> Arc<dyn vol_llm_core::LLMClient> {
    let llm_config = LLMProviderConfig {
        id: "task-cli-test-llm".to_string(),
        config: LLMConfig {
            provider: LLMProvider::Anthropic,
            model: "qwen3.6-plus".to_string(),
            api_key: Secret::literal("not-needed"),
            base_url: "http://192.168.2.162:31693".to_string(),
            body: None,
            headers: None,
        },
    };
    let registry = LLMProviderRegistry::from_configs(&[llm_config]).unwrap();
    registry.get("task-cli-test-llm").unwrap().clone()
}

#[tokio::test]
#[ignore] // Requires model service at 192.168.2.162:31693
async fn test_task_cli_tool_with_real_llm_create_and_list() {
    let store = Arc::new(InMemoryTaskStore::new());
    let llm = create_test_llm();

    let config = AgentConfigBuilder::new()
        .with_llm(llm)
        .with_tool(TaskCliTool::new(store.clone()))
        .with_system_prompt(
            "You are a task manager. You have ONE tool: 'task'.\n\
             It uses CLI syntax. Pass a JSON object with a 'command' string field.\n\n\
             Examples:\n\
             - Create: {\"command\": \"create --name 'Fix bug' --desc 'Repair login'\"}\n\
             - Quick create: {\"command\": \"+task --name 'Quick task'\"}\n\
             - List: {\"command\": \"list\"}\n\
             - Get: {\"command\": \"get --id 1\"}\n\
             - Update: {\"command\": \"update --id 1 --status completed\"}\n\
             - Claim: {\"command\": \"+claim\"}\n\
             - Done: {\"command\": \"+done --id 1\"}\n\
             - Scheme: {\"command\": \"scheme create\"}\n\
             - JSON output: {\"command\": \"get --id 1 --json\"}\n\n\
             IMPORTANT: The argument to the 'task' tool is a JSON object with a 'command' key.\n\
             Example: {\"command\": \"create --name 'Task' --desc 'Description'\"}"
                .to_string(),
        )
        .build()
        .expect("AgentConfig build should succeed");

    let agent = ReActAgent::new(config);

    // Step 1: Create tasks
    let result = agent
        .run(
            "Create a task named 'Fix login bug' with description 'Handle OAuth callback error' \
             and assignee 'backend-team'. Then create another task named 'Write tests' \
             using the quick create shortcut.",
        )
        .await
        .unwrap();

    println!("=== Create result ===\n{}\n", result.content);
    assert!(!result.content.is_empty(), "Response should not be empty");
    assert!(result.iterations > 0, "Should have at least 1 iteration");

    // Step 2: List tasks
    let result = agent.run("List all tasks.").await.unwrap();

    println!("=== List result ===\n{}\n", result.content);
    assert!(!result.content.is_empty());

    // Verify tasks exist in store
    let tasks: Vec<vol_llm_task::Task> = store.list(None).await.unwrap();
    assert!(
        tasks.len() >= 2,
        "Expected at least 2 tasks, got {}",
        tasks.len()
    );
    println!("Store has {} tasks:", tasks.len());
    for t in &tasks {
        println!("  {}: \"{}\" [{:?}]", t.id, t.subject, t.status);
    }
}

#[tokio::test]
#[ignore] // Requires model service at 192.168.2.162:31693
async fn test_task_cli_tool_json_output_mode() {
    let store = Arc::new(InMemoryTaskStore::new());
    let llm = create_test_llm();

    let config = AgentConfigBuilder::new()
        .with_llm(llm)
        .with_tool(TaskCliTool::new(store.clone()))
        .with_system_prompt(
            "You are a task manager. Your ONLY tool is 'task'.\n\
             The argument is a JSON object: {\"command\": \"<cli command>\"}\n\n\
             Use '--json' flag for JSON output.\n\
             Example: {\"command\": \"list --json\"}"
                .to_string(),
        )
        .build()
        .expect("AgentConfig build should succeed");

    let agent = ReActAgent::new(config);

    // Create a task, then list with JSON
    let result = agent
        .run(
            "1. Create a task named 'JSON test' with desc 'Testing JSON mode'\n\
             2. List tasks with JSON output",
        )
        .await
        .unwrap();

    println!("=== JSON mode result ===\n{}\n", result.content);
    assert!(!result.content.is_empty());
}
