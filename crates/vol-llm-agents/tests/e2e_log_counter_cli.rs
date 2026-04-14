//! E2E test: CodingAgent writes a Rust CLI tool to count .log file lines
//! and verifies the HTML report shows events in correct order.

use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
#[ignore] // Requires real LLM API key (ANTHROPIC_AUTH_TOKEN)
async fn test_coding_agent_writes_log_counter_cli() {
    let temp_dir = tempdir().unwrap();
    let report_path = temp_dir.path().join("report.html");
    let work_dir = temp_dir.path().join("work");
    std::fs::create_dir_all(&work_dir).unwrap();

    // Create some test .log files
    std::fs::write(work_dir.join("app.log"), "line 1\nline 2\nline 3\n").unwrap();
    std::fs::write(work_dir.join("error.log"), "error 1\nerror 2\nerror 3\nerror 4\nerror 5\n").unwrap();
    std::fs::write(work_dir.join("debug.log"), "debug 1\n").unwrap();

    let config = CodingAgentConfig {
        max_iterations: 15,
        working_dir: work_dir.clone(),
        hitl_enabled: false,
        verbose: false,
        html_report_path: Some(report_path.clone()),
        llm_provider_id: "anthropic-main".to_string(),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        ..Default::default()
    };

    let agent = CodingAgent::new(config).await.unwrap();

    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        "Write Rust CLI tool to count .log file lines".to_string(),
    ));
    let agent = agent.with_observer(observer);

    let task = r#"Write a Rust CLI tool that:
1. Takes a directory path as command-line argument
2. Finds all .log files in that directory
3. Counts lines in each .log file
4. Prints results sorted by line count (descending)
Format: "{count} lines: {path}"
Use clap for CLI parsing. Create Cargo.toml and src/main.rs."#;

    let result = agent.run(task).await.unwrap();

    assert!(result.success, "CodingAgent should complete successfully");

    // Verify report was generated
    assert!(report_path.exists(), "HTML report should exist");

    // Verify report contains expected events in order
    let content = std::fs::read_to_string(&report_path).unwrap();

    // Check for timeline section
    assert!(content.contains("Timeline"), "Report should have Timeline section");

    // Check for expected event types in order
    let start_pos = content.find("Agent started").expect("Should have AgentStart");
    let thinking_pos = content.find("Thinking").expect("Should have ThinkingComplete");
    let tool_call_pos = content.find("Tool Call").expect("Should have ToolCall");
    let complete_pos = content.find("Agent completed").expect("Should have AgentComplete");

    // Verify rough order (start < thinking < tool_call < complete)
    assert!(start_pos < thinking_pos, "Start should come before Thinking");
    assert!(thinking_pos < tool_call_pos, "Thinking should come before ToolCall");
    assert!(tool_call_pos < complete_pos, "ToolCall should come before Complete");

    // Verify the CLI tool was created
    let cargo_toml = work_dir.join("Cargo.toml");
    let main_rs = work_dir.join("src").join("main.rs");

    if cargo_toml.exists() && main_rs.exists() {
        // Try to build the tool
        let output = std::process::Command::new("cargo")
            .arg("build")
            .current_dir(&work_dir)
            .output();

        if let Ok(output) = output {
            assert!(
                output.status.success(),
                "CLI tool should compile successfully\nstderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[tokio::test]
#[ignore] // Requires real LLM API key
async fn test_html_report_shows_ordered_timeline() {
    let temp_dir = tempdir().unwrap();
    let report_path = temp_dir.path().join("report.html");

    let config = CodingAgentConfig {
        max_iterations: 5,
        working_dir: temp_dir.path().to_path_buf(),
        hitl_enabled: false,
        verbose: false,
        html_report_path: Some(report_path.clone()),
        llm_provider_id: "anthropic-main".to_string(),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        ..Default::default()
    };

    let agent = CodingAgent::new(config).await.unwrap();

    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        "Simple file listing task".to_string(),
    ));
    let agent = agent.with_observer(observer);

    let result = agent.run("List all files in the current directory").await.unwrap();

    assert!(result.success);
    assert!(report_path.exists());

    let content = std::fs::read_to_string(&report_path).unwrap();

    // Extract timeline items and verify order
    // Expected order: AgentStart -> ThinkingComplete -> ToolCallBegin -> ToolCallComplete -> AgentComplete
    let event_positions: Vec<(usize, &'static str)> = vec![
        (content.find("Start").unwrap_or(usize::MAX), "Start"),
        (content.find("Thinking").unwrap_or(usize::MAX), "Thinking"),
        (content.find("Tool Call").unwrap_or(usize::MAX), "Tool Call"),
        (content.find("Complete").unwrap_or(usize::MAX), "Complete"),
    ];

    // Verify positions are in ascending order (excluding NOT_FOUND)
    let mut last_pos = 0;
    for (pos, name) in &event_positions {
        if *pos != usize::MAX {
            assert!(
                *pos >= last_pos,
                "Event '{}' at position {} should come after previous event at {}",
                name, pos, last_pos
            );
            last_pos = *pos;
        }
    }
}
