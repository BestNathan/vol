//! Agent Advice Service
//!
//! Subscribes to alert broadcast, queries historical data from TDengine,
//! generates analysis advice using ReAct Agent, and sends to Feishu.

use tokio::sync::broadcast;
use tracing::{info, warn, error};
use vol_core::{Alert, NotificationHandler, Result as VolResult};
use vol_tracing::TracedEvent;
use vol_llm_agent::{ReActAgent, AgentConfig, AgentStreamEvent};
use vol_llm_tool::{ToolRegistry, ToolContext};
use vol_tdengine::TdengineClient;
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
    /// TDengine client - reserved for future TDengine integration
    #[allow(dead_code)]
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
                    let alert = traced_alert.value().clone();
                    if let Err(e) = self.process_alert(&alert).await {
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

    /// Process a single alert and send AI advice
    async fn process_alert(
        &self,
        alert: &Alert,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let trace_id = &alert.trace_id;

        tracing::info!(
            "Processing alert for AI analysis: {}:{} (trace_id: {})",
            alert.symbol,
            alert.alert_type,
            trace_id
        );

        // Generate advice using ReAct Agent
        let advice = self.generate_advice(alert).await
            .unwrap_or_else(|e| format!("Failed to generate advice: {}", e));

        // Send advice to Feishu
        self.send_advice(&advice, alert, trace_id).await?;

        Ok(())
    }

    /// Fetch history data from TDengine
    ///
    /// TODO: Integrate into generate_advice for contextual analysis
    #[allow(dead_code)]
    async fn fetch_history(&self, _symbol: &str) -> String {
        // TODO: Implement TDengine integration
        // For now, return placeholder
        "历史数据查询功能待实现".to_string()
    }

    /// Generate advice using ReAct Agent
    async fn generate_advice(
        &self,
        alert: &Alert,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Get provider from registry by ID
        let llm = self.registry.get(&self.config.llm_provider_id)
            .ok_or_else(|| format!("Unknown provider: {}", self.config.llm_provider_id))?;

        // Create agent with tools
        let agent = ReActAgent::new(
            llm,
            self.tools.clone(),
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
            "History data will be queried by agent",
        );

        // Run agent with context
        let context = ToolContext {
            messages: Vec::new(),
        };

        // Consume stream to get final response
        let mut stream = agent.run(&user_prompt, context).await?;
        let mut final_response = None;

        while let Some(event) = stream.recv().await {
            match event? {
                AgentStreamEvent::AgentComplete { response } => {
                    final_response = Some(response);
                    break;
                }
                _ => {}
            }
        }

        let response = final_response.ok_or_else(|| vol_llm_agent::AgentError::Context(
            "No final response from agent".to_string()
        ))?;

        Ok(response.content)
    }

    /// Send advice to Feishu
    async fn send_advice(
        &self,
        advice: &str,
        alert: &Alert,
        trace_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.feishu.send_advice(advice, alert, trace_id).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl NotificationHandler for AgentAdviceService {
    fn name(&self) -> &str {
        "agent_advice"
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    async fn send(&self, alert: &Alert) -> VolResult<()> {
        // Check frequency limit first
        if !self.limiter.can_analyze(alert) {
            tracing::info!(
                "Skipping AI analysis for {}:{} (frequency limited)",
                alert.symbol,
                alert.alert_type
            );
            return Ok(());
        }

        // Process the alert
        if let Err(e) = self.process_alert(alert).await {
            tracing::error!("AgentAdviceService failed to process alert: {}", e);
            // Don't return error - we don't want to block other notifications
        }

        // Record this analysis
        self.limiter.record_analysis(alert);

        Ok(())
    }

    fn clone_box(&self) -> Box<dyn NotificationHandler> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_core::{AlertType, Tenor, OptionType};

    #[test]
    fn test_agent_advice_service_creation() {
        // This is a basic smoke test - full integration testing
        // would require mocking LLM provider, Feishu, ToolRegistry, etc.
        // For now, just verify the struct can be created and compiles correctly

        // Verify AlertType and related types exist and can be constructed
        let _alert_type = AlertType::AbsoluteIv { threshold: 0.75 };
        let _tenor = Tenor::Short;
        let _option_type = OptionType::Call;

        // Verify AgentAdviceConfig can be created
        let _config = AgentAdviceConfig {
            enabled: true,
            cooldown_secs: 300,
            max_analyses_per_hour: 20,
            llm_provider_id: "anthropic-main".to_string(),
        };

        // Verify default config works
        let _default_config = AgentAdviceConfig::default();

        // Test passes if code compiles - actual AgentAdviceService creation
        // requires real LLMProviderRegistry, ToolRegistry, TdengineClient, FeishuNotification
    }
}
