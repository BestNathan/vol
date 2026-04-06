//! Agent Advice Service
//!
//! Subscribes to alert broadcast, queries historical data from TDengine,
//! generates analysis advice using ReAct Agent, and sends to Feishu.

use tokio::sync::broadcast;
use tracing::{info, warn, error};
use vol_core::Alert;
use vol_tracing::TracedEvent;
use vol_llm_agent::{ReActAgent, AgentConfig};
use vol_llm_tool::{ToolRegistry, ToolContext, TdengineClient};
use vol_llm_provider::LLMProviderRegistry;
use vol_notification::FeishuNotification;

use std::sync::Arc;
use crate::limiter::FrequencyLimiter;
use crate::prompt::{system_prompt, build_user_prompt, get_threshold_from_alert};

/// Agent advice configuration
#[derive(Clone)]
pub struct AgentAdviceConfig {
    pub enabled: bool,
    pub cooldown_secs: u64,
    pub max_analyses_per_hour: u32,
    pub llm_provider_id: String,
}

impl Default for AgentAdviceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown_secs: 300, // 5 minutes
            max_analyses_per_hour: 20,
            llm_provider_id: "anthropic-main".to_string(),
        }
    }
}

/// Agent Advice Service
#[derive(Clone)]
pub struct AgentAdviceService {
    limiter: FrequencyLimiter,
    config: AgentAdviceConfig,
    registry: LLMProviderRegistry,
    tools: Arc<ToolRegistry>,
    tdengine: Arc<TdengineClient>,
    feishu: FeishuNotification,
}

impl AgentAdviceService {
    /// Create a new agent advice service
    pub fn new(
        config: AgentAdviceConfig,
        registry: LLMProviderRegistry,
        tools: Arc<ToolRegistry>,
        tdengine: Arc<TdengineClient>,
        feishu: FeishuNotification,
    ) -> Self {
        Self {
            limiter: FrequencyLimiter::new(config.cooldown_secs, config.max_analyses_per_hour),
            config,
            registry,
            tools,
            tdengine,
            feishu,
        }
    }

    /// Run the service, subscribing to alert broadcast
    pub async fn run(
        &self,
        mut alert_rx: broadcast::Receiver<TracedEvent<Alert>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("AgentAdviceService started");

        loop {
            match alert_rx.recv().await {
                Ok(traced_alert) => {
                    if let Err(e) = self.process_alert(traced_alert).await {
                        error!("Failed to process alert: {}", e);
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Alert channel closed, stopping AgentAdviceService");
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Lagged {} alerts", n);
                }
            }
        }

        Ok(())
    }

    /// Process a single alert
    async fn process_alert(
        &self,
        traced_alert: TracedEvent<Alert>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let alert = traced_alert.value().clone();
        let trace_id = traced_alert.trace_id().to_string();

        // Check frequency limit
        if !self.limiter.can_analyze(&alert) {
            info!(
                "Skipping analysis for {}:{} (frequency limited)",
                alert.symbol, alert.alert_type
            );
            return Ok(());
        }

        info!(
            "Analyzing alert: {}:{} (trace_id: {})",
            alert.symbol, alert.alert_type, trace_id
        );

        // Fetch history data
        let history_summary = self.fetch_history(&alert.symbol).await;

        // Build agent and generate advice
        let advice = self
            .generate_advice(&alert, history_summary)
            .await
            .unwrap_or_else(|e| format!("Failed to generate advice: {}", e));

        // Send advice to Feishu
        self.send_advice(&advice, &alert, &trace_id).await?;

        // Record this analysis
        self.limiter.record_analysis(&alert);

        Ok(())
    }

    /// Fetch history data from TDengine
    async fn fetch_history(&self, _symbol: &str) -> String {
        // TODO: Implement TDengine integration
        // For now, return placeholder
        "历史数据查询功能待实现".to_string()
    }

    /// Generate advice using ReAct Agent
    async fn generate_advice(
        &self,
        alert: &Alert,
        history_summary: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Create tool registry with default tools
        let tools = ToolRegistry::new();

        // Get provider from registry by ID
        let llm = self.registry.get(&self.config.llm_provider_id)
            .ok_or_else(|| format!("Unknown provider: {}", self.config.llm_provider_id))?;

        // Create agent
        let agent = ReActAgent::new(
            llm,
            tools,
            AgentConfig {
                max_iterations: 5,
                system_prompt: system_prompt().to_string(),
                verbose: false,
            },
        );

        // Get threshold from alert type
        let threshold = get_threshold_from_alert(&alert.alert_type);

        // Build user prompt
        let user_prompt = build_user_prompt(
            &alert.alert_type.to_string(),
            &alert.symbol,
            alert.iv,
            threshold,
            &history_summary,
        );

        // Run agent
        let context = ToolContext::default();
        let response = agent.run(&user_prompt, context).await?;

        Ok(response.content)
    }

    /// Send advice to Feishu
    async fn send_advice(
        &self,
        advice: &str,
        _alert: &Alert,
        trace_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Use actual Feishu notification
        // For now, log the advice
        info!("Feishu message (trace_id: {}):\n{}", &trace_id[..8.min(trace_id.len())], advice);
        Ok(())
    }
}
