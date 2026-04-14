//! Coding Agent basic usage example.

use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let report_path = PathBuf::from("coding-report.html");

    let config = CodingAgentConfig {
        max_iterations: 20,
        working_dir: PathBuf::from("."),
        hitl_enabled: false, // Disable HITL for demo
        verbose: true,
        html_report_path: Some(report_path.clone()),
        llm_provider_id: "anthropic-main".to_string(),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        ..Default::default()
    };

    let agent = CodingAgent::new(config).await?;

    // Create observer
    let observer = Arc::new(HTMLReporter::new(
        report_path,
        "Analyze the project structure".to_string(),
    ));

    let agent = agent.with_observer(observer);

    // Run task
    let result = agent.run("List the files in the current directory and explain the project structure briefly")
        .await?;

    println!("Task completed: {}", result.summary);
    println!("Iterations: {}, Tool calls: {}", result.iterations, result.tool_calls);

    Ok(())
}
