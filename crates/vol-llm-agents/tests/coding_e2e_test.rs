//! End-to-end test for CodingAgent
//!
//! Requires real LLM API key to run. Skip by default.

use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter};
use std::sync::Arc;
use tempfile::tempdir;

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
        hitl_enabled: false,
        verbose: false,
        html_report_path: Some(report_path.clone()),
        llm_provider_id: "anthropic-main".to_string(),
    };

    let agent = CodingAgent::new(config).await.unwrap();

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
        hitl_enabled: false,
        verbose: false,
        html_report_path: Some(report_path.clone()),
        llm_provider_id: "anthropic-main".to_string(),
    };

    let agent = CodingAgent::new(config).await.unwrap();

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
