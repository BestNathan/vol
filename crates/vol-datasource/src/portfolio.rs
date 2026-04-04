//! Portfolio data source - Deribit WebSocket + REST polling.

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use vol_core::{DataSource, MonitoringEvent, PortfolioSnapshot, Result, HealthStatus, EventType};
use vol_deribit::DeribitClient;

/// Portfolio data source configuration
#[derive(Debug, Clone)]
pub struct PortfolioDataSourceConfig {
    pub id: String,
    pub poll_interval_secs: u64,
    pub currencies: Vec<String>,
}

impl Default for PortfolioDataSourceConfig {
    fn default() -> Self {
        Self {
            id: "deribit-portfolio".to_string(),
            poll_interval_secs: 60,
            currencies: vec!["BTC".to_string(), "ETH".to_string()],
        }
    }
}

pub struct PortfolioDataSource {
    id: String,
    client: DeribitClient,
    poll_interval_secs: u64,
    currencies: Vec<String>,
}

impl PortfolioDataSource {
    pub fn new(id: String, client: DeribitClient, poll_interval_secs: u64, currencies: Vec<String>) -> Self {
        Self { id, client, poll_interval_secs, currencies }
    }

    /// Create from configuration
    pub fn from_config(config: PortfolioDataSourceConfig, client: DeribitClient) -> Self {
        Self::new(config.id, client, config.poll_interval_secs, config.currencies)
    }

    /// Fetch portfolio snapshot for a currency
    async fn fetch_snapshot(&self, currency: &str) -> Result<PortfolioSnapshot> {
        // Get account summary from REST API
        let summary = self.client.get_account_summary(currency).await?;

        // Get positions from REST API
        let positions = self.client.get_positions(Some(currency)).await?;

        // Aggregate Greeks from positions
        let mut delta_total = 0.0;
        let mut gamma_total = 0.0;
        let mut vega_total = 0.0;
        let mut theta_total = 0.0;

        for pos in &positions {
            delta_total += pos.delta;
            gamma_total += pos.gamma;
            vega_total += pos.vega;
            theta_total += pos.theta;
        }

        info!("Fetched portfolio {}: equity={:.4}, balance={:.4}, delta={:.2}, vega={:.2}",
              currency, summary.equity, summary.balance, delta_total, vega_total);

        Ok(PortfolioSnapshot {
            currency: summary.currency,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            equity: summary.equity,
            balance: summary.balance,
            available_funds: summary.available_funds,
            margin_balance: summary.margin_balance,
            initial_margin: summary.initial_margin,
            maintenance_margin: summary.maintenance_margin,
            session_pnl: summary.session_upl + summary.session_rpl,
            delta_total,
            options_delta: delta_total,
            options_gamma: gamma_total,
            options_theta: theta_total,
            options_vega: vega_total,
        })
    }
}

#[async_trait]
impl DataSource for PortfolioDataSource {
    fn id(&self) -> &str {
        &self.id
    }

    fn event_type(&self) -> EventType {
        EventType::Portfolio(vol_core::PortfolioMetricType::Greeks)
    }

    fn name(&self) -> &str {
        "deribit-portfolio"
    }

    async fn connect(&mut self) -> Result<()> {
        // Verify client has authentication configured
        if !self.client.has_auth() {
            return Err(vol_core::VolError::Connection(
                "PortfolioDataSource requires authenticated client".to_string()
            ));
        }
        info!("PortfolioDataSource connected and authenticated");
        Ok(())
    }

    async fn run(&self, tx: mpsc::Sender<MonitoringEvent>) -> Result<()> {
        info!("Starting portfolio data source with {} currencies", self.currencies.len());

        let base_interval = self.poll_interval_secs;
        let mut error_count = 0;
        let mut current_interval = base_interval;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(current_interval)).await;

            let mut success = true;
            for currency in &self.currencies {
                match self.fetch_snapshot(currency).await {
                    Ok(snapshot) => {
                        let event = MonitoringEvent::Portfolio(snapshot);
                        if let Err(e) = tx.send(event).await {
                            error!("Failed to send portfolio event: {}", e);
                            success = false;
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to fetch portfolio for {}: {}", currency, e);
                        success = false;
                    }
                }
            }

            // Adjust interval based on success/failure
            if success {
                error_count = 0;
                current_interval = base_interval;
            } else {
                error_count += 1;
                // Exponential backoff: double interval, max 5x base
                current_interval = std::cmp::min(base_interval * 2_u64.pow(error_count), base_interval * 5);
            }
        }
    }

    async fn health_check(&self) -> HealthStatus {
        // Try to fetch positions for first currency
        if let Some(currency) = self.currencies.first() {
            match self.client.get_positions(Some(currency)).await {
                Ok(_) => HealthStatus::Healthy,
                Err(_) => HealthStatus::Unhealthy,
            }
        } else {
            HealthStatus::Degraded
        }
    }

    fn clone_box(&self) -> Box<dyn DataSource> {
        Box::new(self.clone())
    }
}

impl Clone for PortfolioDataSource {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            client: self.client.clone(),
            poll_interval_secs: self.poll_interval_secs,
            currencies: self.currencies.clone(),
        }
    }
}
