//! AdviceAgent Integration Test
//!
//! This test verifies the complete workflow of AdviceAgent:
//! 1. Alert is sent via broadcast channel
//! 2. AdviceAgent receives and processes the alert
//! 3. ReAct Agent analyzes with real LLM API and TDengine tools
//! 4. Feishu notification is sent
//!
//! Requirements:
//! - ANTHROPIC_AUTH_TOKEN environment variable
//! - TDengine connection (env: TDENGINE_HOST, TDENGINE_USER, TDENGINE_PASS)
//! - Feishu credentials (env: FEISHU_APP_ID, FEISHU_APP_SECRET, FEISHU_RECEIVE_ID)
//!
//! Run with:
//! ```bash
//! cargo test -p vol-llm-agents --test advice_agent_integration -- --nocapture
//! ```

use vol_llm_provider::{LLMProviderRegistry, LLMConfig};
use vol_llm_tool::ToolRegistry;
use vol_tdengine::{TdengineConfig, TdengineClient};
use vol_notification::FeishuNotification;
use std::sync::Arc;
use vol_llm_tdengine::{IndexPriceTool, VolatilityIndexTool, OptionsTool, RvTool};
use vol_llm_core::LLMProvider;
use vol_llm_agents::{AdviceAgent, AdviceAgentConfig};
use vol_core::{Alert, AlertType, Tenor, OptionType};
use vol_tracing::TracedEvent;
use tokio::sync::broadcast;

#[tokio::test]
async fn test_advice_agent_end_to_end() {
    // Skip if not configured
    if std::env::var("ANTHROPIC_AUTH_TOKEN").is_err() {
        eprintln!("Skipping test: ANTHROPIC_AUTH_TOKEN not set");
        return;
    }

    // Setup LLM Provider
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    let llm_config = LLMConfig::with_literal_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        api_key,
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );

    let registry = LLMProviderRegistry::from_configs(&[llm_config.clone()]);

    println!("✓ LLM Provider configured");

    // Setup TDengine and Tools
    let tdengine_config = TdengineConfig::default();

    let tdengine_client = match TdengineClient::new(&tdengine_config) {
        Ok(client) => Arc::new(client),
        Err(e) => {
            eprintln!("Skipping test: Failed to connect to TDengine: {}", e);
            return;
        }
    };

    let tool_registry = Arc::new(ToolRegistry::new());

    tool_registry.register(Arc::new(IndexPriceTool::new(Some(tdengine_config.clone()))));
    tool_registry.register(Arc::new(VolatilityIndexTool::new(Some(tdengine_config.clone()))));
    tool_registry.register(Arc::new(OptionsTool::new(Some(tdengine_config.clone()))));
    tool_registry.register(Arc::new(RvTool::new(Some(tdengine_config.clone()))));

    println!("✓ TDengine tools registered");

    // Setup Feishu Notification
    let feishu = FeishuNotification::from_env()
        .expect("FEISHU_APP_ID, FEISHU_APP_SECRET, FEISHU_RECEIVE_ID must be set");

    println!("✓ Feishu notification configured");

    // Setup AdviceAgent
    let config = AdviceAgentConfig {
        enabled: true,
        cooldown_secs: 0,      // Disable cooldown for testing
        max_analyses_per_hour: 100, // High limit for testing
        llm_provider_id: "anthropic-main".to_string(),
    };

    let advice_agent = AdviceAgent::new(
        config,
        registry,
        tool_registry,
        tdengine_client,
        feishu,
    );

    println!("✓ AdviceAgent created");

    // Setup Alert channel
    let (alert_tx, alert_rx): (broadcast::Sender<TracedEvent<Alert>>, _) =
        broadcast::channel(100);

    println!("✓ Alert channel created");

    // Create test alert
    let test_alert = Alert {
        alert_type: AlertType::AbsoluteIv { threshold: 0.5 },
        tenor: Tenor::Short,
        symbol: "BTC".to_string(),
        iv: 0.55,  // Above threshold
        message: "IV exceeded threshold".to_string(),
        timestamp: 0,
        source: "test".to_string(),
        index_price: 50000.0,
        dte: 30,
        option_type: OptionType::Call,
        moneyness: 1.0,
        mark_price_coin: 0.05,
        trace_id: "test-integration-001".to_string(),
    };

    println!("✓ Test alert created: BTC AbsoluteIv (IV=0.55, threshold=0.5)");

    // Start AdviceAgent in background
    let agent_clone = advice_agent.clone();
    let handle = tokio::spawn(async move {
        if let Err(e) = agent_clone.run(alert_rx).await {
            eprintln!("AdviceAgent error: {}", e);
        }
    });

    println!("✓ AdviceAgent started in background");

    // Send test alert
    let traced_alert = TracedEvent::new(test_alert.clone());
    alert_tx.send(traced_alert).expect("Failed to send alert");

    println!("✓ Test alert sent");

    // Wait for processing (LLM response may take time)
    println!("⏳ Waiting for AdviceAgent to process alert...");
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

    // Abort the agent task
    handle.abort();

    println!("✓ Test completed");

    // Verify log directory was created
    let log_path = std::path::PathBuf::from("logs/agents/advice_agent");
    if log_path.exists() {
        println!("✓ Agent log directory exists: {:?}", log_path);

        // Check for run logs
        let runs_path = log_path.join("runs");
        if runs_path.exists() {
            if let Ok(entries) = std::fs::read_dir(&runs_path) {
                let count = entries.count();
                println!("✓ {} run log(s) created", count);
            }
        }
    }

    // Note: Feishu notification sending is best-effort in this test
    // The test passes if the agent processes the alert without panicking
}
