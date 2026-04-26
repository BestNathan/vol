//! End-to-end test for CodingAgent
//!
//! Requires real LLM API key to run. Skip by default.

use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter};
use vol_llm_core::LLMClient;
use vol_llm_provider::{LLMConfig, LLMProviderConfig, LLMProviderRegistry, Secret};
use vol_llm_core::LLMProvider;
use vol_llm_tool::ToolConfig;
use std::sync::Arc;
use tempfile::tempdir;

/// Helper to construct the LLM client for tests.
fn create_test_llm() -> Arc<dyn LLMClient> {
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");
    let llm_config = LLMProviderConfig {
        id: "anthropic-main".to_string(),
        config: LLMConfig {
            provider: LLMProvider::Anthropic,
            model: "qwen3.5-plus".to_string(),
            api_key: Secret::literal(api_key),
            base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
        },
    };
    let registry = LLMProviderRegistry::from_configs(&[llm_config]).unwrap();
    registry.get("anthropic-main").unwrap().clone()
}

#[tokio::test]
#[ignore] // Requires real LLM API key (ANTHROPIC_AUTH_TOKEN)
async fn test_coding_agent_e2e_read_file() {
    let temp_dir = tempdir().unwrap();
    let report_path = temp_dir.path().join("report.html");

    // Create a test file
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "Hello, World!").unwrap();

    let config = CodingAgentConfig {
        max_iterations: 3,
        working_dir: temp_dir.path().to_path_buf(),
        html_report_path: Some(report_path.clone()),
        llm: Some(create_test_llm()),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        tool_config: ToolConfig::new(),
        ..Default::default()
    };

    let agent = CodingAgent::new(config).unwrap();

    // Create observer
    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        "Read test file".to_string(),
    ));
    let agent = agent.with_observer(observer);

    let result = agent.run("Read the test.txt file and tell me its content")
        .await
        .unwrap();

    assert!(result.success);
    assert!(result.summary.contains("Hello"));
    assert!(report_path.exists());
}

#[tokio::test]
#[ignore] // Requires real LLM API key
async fn test_coding_agent_e2e_edit_file() {
    let temp_dir = tempdir().unwrap();
    let report_path = temp_dir.path().join("report.html");

    // Create a test file
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(&test_file, "fn main() { println!(\"old\"); }").unwrap();

    let config = CodingAgentConfig {
        max_iterations: 5,
        working_dir: temp_dir.path().to_path_buf(),
        html_report_path: Some(report_path.clone()),
        llm: Some(create_test_llm()),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        tool_config: ToolConfig::new(),
        ..Default::default()
    };

    let agent = CodingAgent::new(config).unwrap();

    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        "Edit test file".to_string(),
    ));
    let agent = agent.with_observer(observer);

    let result = agent.run("Change the print output from \"old\" to \"new\" in test.rs")
        .await
        .unwrap();

    assert!(result.success);

    // Verify the file was modified
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("new"));

    // Verify report was generated
    assert!(report_path.exists());
}

#[tokio::test]
#[ignore] // Requires real LLM API key
async fn test_coding_agent_html_report_contains_timeline() {
    let temp_dir = tempdir().unwrap();
    let report_path = temp_dir.path().join("report.html");

    let config = CodingAgentConfig {
        max_iterations: 5,
        working_dir: temp_dir.path().to_path_buf(),
        html_report_path: Some(report_path.clone()),
        llm: Some(create_test_llm()),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        tool_config: ToolConfig::new(),
        ..Default::default()
    };

    let agent = CodingAgent::new(config).unwrap();

    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        "Read file task".to_string(),
    ));
    let agent = agent.with_observer(observer);

    // Create a test file
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "Hello, World!").unwrap();

    let result = agent.run("Read the test.txt file and tell me its content")
        .await
        .unwrap();

    assert!(result.success);

    // Verify report exists
    assert!(report_path.exists());

    // Verify report contains timeline events
    let content = std::fs::read_to_string(&report_path).unwrap();

    // Should have start event
    assert!(content.contains("Agent started"), "Report should contain AgentStart event");

    // Should have thinking events
    assert!(content.contains("Thinking"), "Report should contain ThinkingComplete event");

    // Should have tool call events (read_file)
    assert!(content.contains("Tool Call") || content.contains("read_file"), "Report should contain ToolCall events");

    // Should have completion event
    assert!(content.contains("Agent completed"), "Report should contain AgentComplete event");

    // Should have timeline section
    assert!(content.contains("Timeline"), "Report should have Timeline section");
}
