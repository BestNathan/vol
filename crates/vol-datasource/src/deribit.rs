//! Deribit WebSocket data source implementation.
//!
//! Uses DeribitClient for low-level WebSocket communication
//! and implements the DataSource trait for integration.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, error, warn};
use vol_core::{DataSource, HealthStatus, Result, VolatilityData};
use vol_deribit::{ChannelType, ChannelData, DeribitClient};

/// Index price state - thread-safe shared state
#[derive(Debug, Clone, Default)]
pub struct IndexPriceState {
    prices: Arc<Mutex<HashMap<String, f64>>>,
}

impl IndexPriceState {
    pub fn new() -> Self {
        Self {
            prices: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn update(&self, index: &str, price: f64) {
        let mut prices = self.prices.lock().await;
        prices.insert(index.to_lowercase(), price);
    }

    pub async fn get(&self, index: &str) -> Option<f64> {
        let prices = self.prices.lock().await;
        prices.get(&index.to_lowercase()).copied()
    }
}

/// Deribit WebSocket data source
pub struct DeribitDataSource {
    client: DeribitClient,
    index_price_state: IndexPriceState,
    _symbols: Vec<String>,
}

impl DeribitDataSource {
    pub fn new(ws_url: String, symbols: Vec<String>, _poll_interval_secs: u64) -> Self {
        let client = DeribitClient::new(ws_url);
        Self {
            client,
            index_price_state: IndexPriceState::new(),
            _symbols: symbols,
        }
    }

    pub fn with_proxy(mut self, proxy_url: String) -> Self {
        self.client = self.client.with_proxy(proxy_url);
        self
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

        // Build list of all channels to subscribe to
        let mut all_channels = Vec::new();
        let mut index_symbols = Vec::new();

        for symbol in &symbols {
            let index = format!("{}_usd", symbol.to_lowercase());
            all_channels.push(ChannelType::MarkpriceOptions(index.clone()));
            all_channels.push(ChannelType::PriceIndex(index.clone()));
            index_symbols.push(index);
        }

        let index_state = self.index_price_state.clone();
        let _client_clone = self.client.clone();
        let _tx_clone = tx.clone();

        // Spawn single data merger task that handles all channels
        tokio::spawn(async move {
            // Subscribe to all channels and collect receivers
            let mut option_rxs: Vec<mpsc::Receiver<ChannelData>> = Vec::new();
            let mut index_rxs: Vec<mpsc::Receiver<ChannelData>> = Vec::new();

            for channel in &all_channels {
                info!("Subscribing to channel: {}", channel.channel_name());
                match _client_clone.subscribe(channel.clone()).await {
                    Ok(rx) => {
                        info!("Successfully subscribed to channel");
                        match channel {
                            ChannelType::MarkpriceOptions(_) => option_rxs.push(rx),
                            ChannelType::PriceIndex(_) => index_rxs.push(rx),
                            ChannelType::Ticker(_) | ChannelType::Trade(_) => {
                                // Not used in current implementation but handled for completeness
                                warn!("Unexpected channel type: {:?}", channel);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to subscribe to channel: {}", e);
                    }
                }
            }

            info!("All subscriptions complete, merging data streams");

            // Merge index price receivers into one
            let (merged_index_tx, mut merged_index_rx) = mpsc::channel(1024);
            for mut rx in index_rxs {
                let merged_tx = merged_index_tx.clone();
                tokio::spawn(async move {
                    while let Some(data) = rx.recv().await {
                        let _ = merged_tx.send(data).await;
                    }
                });
            }
            drop(merged_index_tx);

            // Merge options receivers into one
            let (merged_options_tx, mut merged_options_rx) = mpsc::channel(1024);
            for mut rx in option_rxs {
                let merged_tx = merged_options_tx.clone();
                tokio::spawn(async move {
                    while let Some(data) = rx.recv().await {
                        let _ = merged_tx.send(data).await;
                    }
                });
            }
            drop(merged_options_tx);

            // Now use tokio::select! to merge the two streams
            loop {
                tokio::select! {
                    // Handle index price updates
                    Some(data) = merged_index_rx.recv() => {
                        if let ChannelData::PriceIndex(price_data) = data {
                            index_state.update(&price_data.index_name, price_data.price).await;
                            info!("Updated index price: {} = {}", price_data.index_name, price_data.price);
                        }
                    }

                    // Handle options mark prices
                    Some(data) = merged_options_rx.recv() => {
                        if let ChannelData::OptionMarkPrice(options_list) = data {
                            for option in options_list {
                                let underlying = option.instrument_name.split('-').next().unwrap_or("BTC");
                                let index_key = format!("{}_usd", underlying.to_lowercase());
                                let index_price = index_state.get(&index_key).await;

                                if let Some(vol_data) = option.to_volatility_data_with_index(index_price) {
                                    let _ = _tx_clone.send(vol_data).await;
                                }
                            }
                        }
                    }

                    else => {
                        warn!("All channels closed");
                        break;
                    }
                }
            }
        });

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
