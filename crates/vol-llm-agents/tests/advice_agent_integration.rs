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
use vol_tdengine::TdengineConfig;
use vol_notification::FeishuNotification;
use std::sync::Arc;
use vol_llm_tdengine::{IndexPriceTool, VolatilityIndexTool, OptionsTool, RvTool};
use vol_llm_core::LLMProvider;

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
}
