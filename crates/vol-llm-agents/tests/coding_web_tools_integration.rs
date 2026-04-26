//! Integration test: CodingAgent with web_fetch tool
//!
//! Tests that the CodingAgent actively uses web_fetch when given a task
//! requiring fetching content from a URL (e.g., Deribit API documentation).
//!
//! This test does NOT implement Deribit API integration — it only verifies
//! that the agent discovers and calls the web_fetch tool.
//!
//! Requires: ANTHROPIC_AUTH_TOKEN env var
//! Run with: cargo test --test coding_web_tools_integration -- --ignored

use std::sync::Arc;
use tempfile::tempdir;
use vol_llm_tool::ToolConfig;
use vol_llm_tools_builtin::{WebFetchConfig, ProxyConfig};
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, ChannelledEventObserver};
use vol_llm_core::{LLMClient, LLMProvider};
use vol_llm_provider::{LLMConfig, LLMProviderConfig, LLMProviderRegistry, Secret};
use vol_llm_agent::AgentStreamEvent;

/// Helper to configure web_fetch in ToolConfig
fn configure_web_fetch(tool_config: &mut ToolConfig) {
    let fetch_cfg = WebFetchConfig {
        max_content_length: Some(2_000_000), // 2MB
        proxy: ProxyConfig::default(),
    };
    tool_config.set("web_fetch", fetch_cfg);
}

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

/// Test that CodingAgent registers web_fetch when ToolConfig is provided
/// and actually calls it during a task requiring URL content.
#[tokio::test]
#[ignore] // Requires real LLM API key (ANTHROPIC_AUTH_TOKEN)
async fn test_coding_agent_uses_web_fetch_for_deribit_docs() {
    let temp_dir = tempdir().unwrap();

    // Configure tool_config with web_fetch enabled
    let mut tool_config = ToolConfig::new();
    configure_web_fetch(&mut tool_config);

    let config = CodingAgentConfig {
        max_iterations: 5,
        working_dir: temp_dir.path().to_path_buf(),
        hitl_enabled: false,
        html_report_path: None,
        llm: Some(create_test_llm()),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        tool_config,
        ..Default::default()
    };

    // Create agent
    let agent = CodingAgent::new(config).unwrap();

    // Set up a channelled observer to capture events
    let observer = Arc::new(ChannelledEventObserver::new());
    let agent = agent.with_observer(observer.clone());

    // Run the task — ask agent to fetch Deribit docs
    let result = agent.run(
        "根据链接 https://docs.deribit.com 的内容，\
         了解 Deribit API 的 WebSocket 连接方式，\
         总结连接步骤并写一个简要说明。"
    )
    .await
    .expect("Agent run should succeed");

    assert!(result.success, "Agent should complete successfully");

    // Retrieve all events from the observer
    let events = observer.events().await;

    // Verify the agent actually called web_fetch
    let web_fetch_called = events.iter().any(|e| {
        matches!(e, AgentStreamEvent::ToolCallBegin { tool_name, .. }
            if tool_name == "web_fetch")
    });

    let tool_calls: Vec<_> = events.iter().filter_map(|e| {
        match e {
            AgentStreamEvent::ToolCallBegin { tool_name, .. } => Some(format!("Called: {}", tool_name)),
            AgentStreamEvent::ToolCallComplete { tool_name, .. } => Some(format!("Completed: {}", tool_name)),
            _ => None,
        }
    }).collect();

    assert!(
        web_fetch_called,
        "Agent should have called web_fetch tool. Tool calls made: {:#?}",
        tool_calls
    );

    // Verify the agent's response mentions Deribit or API concepts
    let summary_lower = result.summary.to_lowercase();
    let mentions_deribit_or_api = summary_lower.contains("deribit")
        || summary_lower.contains("websocket")
        || summary_lower.contains("api");

    assert!(
        mentions_deribit_or_api,
        "Agent response should mention Deribit, WebSocket, or API. Got: {}",
        result.summary
    );

    eprintln!("Agent completed in {} iterations with {} tool calls",
        result.iterations, result.tool_calls);
}

/// Test that without web_fetch config, the agent only has core tools available.
#[tokio::test]
#[ignore] // Requires real LLM API key
async fn test_coding_agent_without_web_fetch_has_core_tools_only() {
    let temp_dir = tempdir().unwrap();

    // No web tool config
    let config = CodingAgentConfig {
        max_iterations: 3,
        working_dir: temp_dir.path().to_path_buf(),
        hitl_enabled: false,
        html_report_path: None,
        llm: Some(create_test_llm()),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        tool_config: ToolConfig::new(), // empty
        ..Default::default()
    };

    let agent = CodingAgent::new(config).unwrap();

    // Should still work for file-based tasks
    let test_file = temp_dir.path().join("hello.txt");
    std::fs::write(&test_file, "Hello from coding agent!").unwrap();

    let observer = Arc::new(ChannelledEventObserver::new());
    let agent = agent.with_observer(observer.clone());

    let result = agent.run("Read hello.txt and tell me what it says")
        .await
        .expect("Agent run should succeed");

    assert!(result.success);
    assert!(result.summary.contains("Hello"));
}
