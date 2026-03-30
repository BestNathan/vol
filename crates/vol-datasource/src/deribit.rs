//! Deribit WebSocket data source implementation.
//!
//! Uses DeribitClient for low-level WebSocket communication
//! and implements the DataSource trait for integration.

use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::info;
use vol_core::{DataSource, HealthStatus, Result, VolatilityData};
use vol_deribit::{DeribitClient, subscription};

/// Deribit WebSocket data source
pub struct DeribitDataSource {
    client: DeribitClient,
    state: Arc<Mutex<DeribitState>>,
}

struct DeribitState {
    connected: bool,
    subscriptions: Vec<String>,
}

impl DeribitDataSource {
    pub fn new(ws_url: String, _symbols: Vec<String>, _poll_interval_secs: u64) -> Self {
        let client = DeribitClient::new(ws_url);
        Self {
            client,
            state: Arc::new(Mutex::new(DeribitState {
                connected: false,
                subscriptions: Vec::new(),
            })),
        }
    }

    pub fn with_proxy(mut self, proxy_url: String) -> Self {
        self.client = self.client.with_proxy(proxy_url);
        self
    }

    /// Spawn the WebSocket reader loop
    fn spawn_ws_loop(
        client: DeribitClient,
        channels: Vec<String>,
        tx: mpsc::Sender<VolatilityData>,
        state: Arc<Mutex<DeribitState>>,
    ) {
        tokio::spawn(async move {
            // Create a channel that the client will use to send data
            let (client_tx, mut client_rx) = mpsc::channel::<VolatilityData>(1024);

            // Spawn the client run loop
            let client_clone = client.clone();
            tokio::spawn(async move {
                client_clone.run(channels, client_tx).await;
            });

            // Forward messages from client to datasource tx, updating state
            while let Some(vol_data) = client_rx.recv().await {
                // Update state on first message if not connected
                {
                    let mut s = state.lock().await;
                    if !s.connected {
                        s.connected = true;
                        s.subscriptions = client.subscriptions().await;
                    }
                }

                if let Err(e) = tx.send(vol_data).await {
                    tracing::warn!("Failed to send volatility data: {}", e);
                    break;
                }
            }
        });
    }
}

#[async_trait::async_trait]
impl DataSource for DeribitDataSource {
    fn name(&self) -> &str {
        "deribit"
    }

    async fn connect(&mut self) -> Result<()> {
        info!("Initializing Deribit data source");
        Ok(())
    }

    fn subscribe(&self, symbols: Vec<String>) -> Result<mpsc::Receiver<VolatilityData>> {
        let (tx, rx) = mpsc::channel(1024);

        // Build channels list for options mark prices with IV data
        let channels: Vec<String> = symbols
            .iter()
            .map(|s| subscription::markprice_options(&format!("{}_usd", s.to_lowercase())))
            .collect();

        info!("Subscribing to channels: {:?}", channels);

        // Spawn WebSocket reader task
        let state = self.state.clone();
        Self::spawn_ws_loop(self.client.clone(), channels, tx, state);

        Ok(rx)
    }

    async fn health_check(&self) -> HealthStatus {
        if self.client.is_connected().await {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }
}
