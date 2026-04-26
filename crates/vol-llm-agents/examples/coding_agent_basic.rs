//! Coding Agent basic usage example.
//!
//! Loads a task from coding_task.txt, configures web fetch proxy for Deribit docs access,
//! runs the coding agent, and verifies session/run logs after completion.

use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter};
use vol_llm_tool::{ToolConfig, ProxyConfig};
use vol_llm_tools_builtin::WebFetchConfig;
use vol_llm_provider::{LLMConfig, LLMProviderConfig, LLMProviderRegistry, Secret};
use vol_llm_core::LLMProvider;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load task from file
    let task_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("coding_task.txt");
    let task = std::fs::read_to_string(&task_path)
        .unwrap_or_else(|e| panic!("Failed to read task file {:?}: {}", task_path, e));
    println!("=== Task ===\n{}\n============\n", task.trim());

    // Configure web fetch with proxy for Deribit docs access
    let mut tool_config = ToolConfig::new();
    let fetch_cfg = WebFetchConfig {
        max_content_length: None,
        proxy: ProxyConfig {
            proxy_url: Some("http://192.168.2.98:8890".to_string()),
        },
    };
    tool_config.set("web_fetch", fetch_cfg);

    // Set up unique agent ID for this run
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let agent_id = format!("deribit-ws-client-{}", timestamp);
    let working_dir = PathBuf::from("/tmp/deribit-ws-client");

    println!("Agent ID: {}", agent_id);
    println!("Working dir: {:?}", working_dir);

    // Construct LLM externally — CodingAgent does not read env vars
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set for this example");
    let llm_config = LLMProviderConfig {
        id: "anthropic-main".to_string(),
        config: LLMConfig {
            provider: LLMProvider::Anthropic,
            model: "qwen3.5-plus".to_string(),
            api_key: Secret::literal(api_key),
            base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
        },
    };
    let registry = LLMProviderRegistry::from_configs(&[llm_config])
        .expect("Failed to create LLM provider registry");
    let llm = registry.get("anthropic-main")
        .expect("LLM provider 'anthropic-main' not found")
        .clone();

    let config = CodingAgentConfig {
        max_iterations: 30,
        working_dir: working_dir.clone(),
        agent_id: agent_id.clone(),
        tool_config,
        llm: Some(llm),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        ..Default::default()
    };

    let agent = CodingAgent::new(config)?;

    // Create observer for HTML report
    let report_path = PathBuf::from(format!("coding-report-{}.html", timestamp));
    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        task.trim().to_string(),
    ));

    let agent = agent.with_observer(observer);

    // Run the task
    println!("\n=== Running Agent ===\n");
    let result = agent.run(task.trim()).await?;

    println!("\n=== Task Completed ===");
    println!("Success: {}", result.success);
    println!("Iterations: {}, Tool calls: {}", result.iterations, result.tool_calls);
    println!("HTML Report: {:?}", report_path);

    // Verify session and run logs
    let session_log_dir = working_dir.join("logs/agents").join(&agent_id);
    let run_log_dir = session_log_dir.join("runs");

    println!("\n=== Session Logs ===");
    if session_log_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&session_log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "jsonl") {
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    println!("  {} ({} bytes)", path.display(), size);

                    // Read last few lines to verify completeness
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let lines: Vec<&str> = content.lines().collect();
                        let total = lines.len();
                        println!("    Lines: {}", total);
                        // Show last 3 events
                        for line in lines.iter().rev().take(3).rev() {
                            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                                let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
                                println!("    Last event: type={}", event_type);
                            }
                        }
                    }
                }
            }
        }
    } else {
        println!("  Session log directory not found: {:?}", session_log_dir);
    }

    println!("\n=== Run Logs ===");
    if run_log_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&run_log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "jsonl") {
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    println!("  {} ({} bytes)", path.display(), size);

                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let lines: Vec<&str> = content.lines().collect();
                        let total = lines.len();
                        println!("    Lines: {}", total);
                        for line in lines.iter().rev().take(3).rev() {
                            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                                let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
                                println!("    Last event: type={}", event_type);
                            }
                        }
                    }
                }
            }
        }
    } else {
        println!("  Run log directory not found: {:?}", run_log_dir);
    }

    // Check if the agent created any Go source files
    let work_dir = PathBuf::from("/tmp/deribit-ws-client");
    if work_dir.exists() {
        println!("\n=== Generated Files ===");
        if let Ok(entries) = std::fs::read_dir(&work_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.ends_with(".go") || name_str == "go.mod" || name_str == "go.sum" {
                        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                        println!("  {} ({} bytes)", path.display(), size);
                    }
                }
            }
        }
    }

    Ok(())
}
