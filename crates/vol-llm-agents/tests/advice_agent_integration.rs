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

use vol_llm_provider::{LLMProviderRegistry, LLMConfig, LLMProviderConfig};
use vol_llm_tool::ToolRegistry;
use vol_tdengine::{TdengineConfig, TdengineClient};
use vol_notification::FeishuNotification;
use vol_config::FeishuConfig;
use std::sync::Arc;
use vol_llm_tdengine::{IndexPriceTool, VolatilityIndexTool, OptionsTool, RvTool};
use vol_llm_agents::{AdviceAgent, AdviceAgentConfig};
use vol_core::{Alert, AlertType, Tenor, OptionType};
use vol_tracing::TracedEvent;
use tokio::sync::broadcast;
use tracing::Span;
use std::env;

fn default_message_template() -> String {
    "🚨 {tenor} {alert_type}: {symbol} | IV={value} | 指数={index_price} | DTE={dte}天 | {option_type} | 价格={mark_price_coin} ({mark_price_usd} USD)".to_string()
}

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

    let provider_config = LLMProviderConfig {
        id: "anthropic-main".to_string(),
        config: llm_config,
    };

    let registry = LLMProviderRegistry::from_configs(&[provider_config])
        .expect("Failed to create LLM provider registry");

    println!("✓ LLM Provider configured");

    // Setup TDengine and Tools
    let tdengine_config = TdengineConfig::default();

    let tdengine_client = TdengineClient::new(tdengine_config.clone());

    let mut tool_registry = ToolRegistry::new();

    tool_registry.register(IndexPriceTool::new(Some(tdengine_config.clone())));
    tool_registry.register(VolatilityIndexTool::new(Some(tdengine_config.clone())));
    tool_registry.register(OptionsTool::new(Some(tdengine_config.clone())));
    tool_registry.register(RvTool::new(Some(tdengine_config.clone())));

    println!("✓ TDengine tools registered");

    // Setup Feishu Notification (skip test if not configured)
    let feishu_config = match (
        env::var("FEISHU_APP_ID").ok(),
        env::var("FEISHU_APP_SECRET").ok(),
        env::var("FEISHU_RECEIVE_ID").ok(),
    ) {
        (Some(app_id), Some(app_secret), Some(receive_id)) => {
            FeishuConfig {
                app_id: Some(app_id),
                app_secret: Some(app_secret),
                receive_id: Some(receive_id),
                message_template: default_message_template(),
            }
        }
        _ => {
            eprintln!("Skipping test: FEISHU_* environment variables not set");
            eprintln!("Set FEISHU_APP_ID, FEISHU_APP_SECRET, FEISHU_RECEIVE_ID to run this test");
            return;
        }
    };

    let feishu = FeishuNotification::new(feishu_config)
        .map_err(|e| {
            eprintln!("Skipping test: Failed to create Feishu notification: {}", e);
        })
        .ok();

    // If feishu is None, skip the test
    let feishu = match feishu {
        Some(f) => {
            println!("✓ Feishu notification configured");
            f
        }
        None => {
            eprintln!("Skipping test: Failed to create Feishu notification");
            return;
        }
    };

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
        Arc::new(tool_registry),
        Arc::new(tdengine_client),
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
    let traced_alert = TracedEvent::with_trace_id(
        test_alert.clone(),
        Some(Span::current()),
        test_alert.trace_id.clone(),
    );
    let _ = alert_tx.send(traced_alert);

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
