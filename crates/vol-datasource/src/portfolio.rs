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

        Ok(PortfolioSnapshot {
            currency: currency.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            equity: 0.0,
            balance: 0.0,
            available_funds: 0.0,
            margin_balance: 0.0,
            initial_margin: 0.0,
            maintenance_margin: 0.0,
            session_pnl: 0.0,
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
        info!("PortfolioDataSource connected (no WebSocket subscription needed)");
        Ok(())
    }

    async fn run(&self, tx: mpsc::Sender<MonitoringEvent>) -> Result<()> {
        info!("Starting portfolio data source with {} currencies", self.currencies.len());

        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(self.poll_interval_secs));

        loop {
            ticker.tick().await;

            for currency in &self.currencies {
                match self.fetch_snapshot(currency).await {
                    Ok(snapshot) => {
                        let event = MonitoringEvent::Portfolio(snapshot);
                        if let Err(e) = tx.send(event).await {
                            error!("Failed to send portfolio event: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to fetch portfolio for {}: {}", currency, e);
                    }
                }
            }
        }
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
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
