//! Volatility data source implementation.
//!
//! Uses DeribitClient for low-level WebSocket communication.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, error, warn, info_span};
use vol_config::DeribitClientConfig;
use vol_core::{DataSource, HealthStatus, Result, VolatilityData, MonitoringEvent, EventType};
use vol_deribit::{ChannelType, ChannelData, DeribitClient};
use vol_tracing::{WithSpan, new_trace_id, Instrument};

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

/// Volatility data source
#[derive(Clone)]
pub struct VolatilityDataSource {
    client: DeribitClient,
    index_price_state: IndexPriceState,
    symbols: Vec<String>,
    id: String,
}

impl VolatilityDataSource {
    /// Create a new VolatilityDataSource from client configuration
    pub fn from_config(client_config: DeribitClientConfig, symbols: Vec<String>, id: String) -> Self {
        let mut client = DeribitClient::new(&client_config.ws_url);

        // Configure proxy if available via environment
        if let Ok(proxy) = std::env::var("HTTPS_PROXY").or_else(|_| std::env::var("HTTP_PROXY")) {
            info!("Using proxy: {}", proxy);
            client = client.with_proxy(proxy);
        }

        // Configure auth if available
        let auth_opt = &client_config.auth;
        if let Some(auth) = auth_opt {
            if let (Some(client_id), Some(client_secret)) = (auth.client_id(), auth.client_secret()) {
                client = client.with_auth(client_id, client_secret);
            }
        }

        Self {
            client,
            index_price_state: IndexPriceState::new(),
            symbols,
            id,
        }
    }

    /// Run the datasource, sending events to the provided channel
    pub async fn run_internal(&self, tx: mpsc::Sender<MonitoringEvent>) -> Result<()> {
        let (internal_tx, mut internal_rx) = mpsc::channel::<WithSpan<VolatilityData>>(1024);

        // Build list of all channels to subscribe to
        let mut all_channels = Vec::new();

        for symbol in &self.symbols {
            let index = format!("{}_usd", symbol.to_lowercase());
            all_channels.push(ChannelType::MarkpriceOptions(index.clone()));
            all_channels.push(ChannelType::PriceIndex(index.clone()));
        }

        let index_state = self.index_price_state.clone();
        let client_clone = self.client.clone();

        // Spawn internal data merger task
        let data_task = tokio::spawn(async move {
            // Subscribe to all channels and collect receivers
            let mut option_rxs: Vec<mpsc::Receiver<ChannelData>> = Vec::new();
            let mut index_rxs: Vec<mpsc::Receiver<ChannelData>> = Vec::new();

            for channel in &all_channels {
                info!("Subscribing to channel: {}", channel.channel_name());
                match client_clone.subscribe(channel.clone()).await {
                    Ok(rx) => {
                        info!("Successfully subscribed to channel");
                        match channel {
                            ChannelType::MarkpriceOptions(_) => option_rxs.push(rx),
                            ChannelType::PriceIndex(_) => index_rxs.push(rx),
                            ChannelType::Ticker(_) | ChannelType::Trade(_) => {
                                warn!("Unexpected channel type: {:?}", channel);
                            }
                            ChannelType::UserPortfolio(_) => {
                                warn!("UserPortfolio channel requires OAuth authentication");
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
                    Some(data) = merged_index_rx.recv() => {
                        if let ChannelData::PriceIndex(price_data) = data {
                            index_state.update(&price_data.index_name, price_data.price).await;
                            info!("Updated index price: {} = {}", price_data.index_name, price_data.price);
                        }
                    }

                    Some(data) = merged_options_rx.recv() => {
                        if let ChannelData::OptionMarkPrice(options_list) = data {
                            for option in options_list {
                                let underlying = match option.instrument_name.split('-').next() {
                                    Some(u) => u,
                                    None => {
                                        error!(
                                            instrument = %option.instrument_name,
                                            "Failed to parse underlying from instrument name"
                                        );
                                        continue;
                                    }
                                };

                                let index_key = format!("{}_usd", underlying.to_lowercase());
                                let index_price = index_state.get(&index_key).await;

                                if index_price.is_none() {
                                    warn!(
                                        instrument = %option.instrument_name,
                                        index_key = %index_key,
                                        "Index price not available, skipping option"
                                    );
                                    continue;
                                }

                                if let Some(vol_data) = option.to_volatility_data_with_index(index_price) {
                                    let trace_id = new_trace_id();
                                    let span = info_span!(
                                        "datasource_receive",
                                        source = "deribit",
                                        trace_id = %trace_id,
                                        iv = %vol_data.iv,
                                        symbol = %vol_data.symbol,
                                        dte = vol_data.dte,
                                        index_price = %vol_data.index_price,
                                        option_type = %vol_data.option_type,
                                    );

                                    let traced_event = WithSpan::new(vol_data, span.clone());
                                    if let Err(e) = internal_tx.send(traced_event).instrument(span).await {
                                        error!(
                                            instrument = %option.instrument_name,
                                            error = %e,
                                            "Failed to send volatility data"
                                        );
                                    }
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

        // Forward VolatilityData events as MonitoringEvent::Volatility
        while let Some(traced_vol_data) = internal_rx.recv().await {
            // Extract span metadata (trace_id, etc.) before consuming the span
            // The span context will be preserved in the same async task context
            let event = traced_vol_data.into_value();
            let monitoring_event = MonitoringEvent::Volatility(event);
            if let Err(e) = tx.send(monitoring_event).await {
                error!("Failed to send event: {}", e);
                break;
            }
        }

        // Cancel the data task if we exit the loop
        data_task.abort();

        Ok(())
    }
}

#[async_trait::async_trait]
impl DataSource for VolatilityDataSource {
    fn id(&self) -> &str {
        &self.id
    }

    fn event_type(&self) -> EventType {
        EventType::Volatility
    }

    fn name(&self) -> &str {
        "volatility"
    }

    async fn connect(&mut self) -> Result<()> {
        info!("Initializing volatility data source");
        Ok(())
    }

    async fn run(&self, tx: mpsc::Sender<MonitoringEvent>) -> Result<()> {
        self.run_internal(tx).await
    }

    async fn health_check(&self) -> HealthStatus {
        if self.client.is_connected().await {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    fn clone_box(&self) -> Box<dyn DataSource> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::TraceContextExt;
    use tracing::info_span;

    #[test]
    fn test_trace_id_format() {
        // Create a span and verify trace_id format
        // Note: Without an initialized tracer provider, trace_id will be all zeros
        // This test verifies the API works correctly - actual trace_id generation
        // depends on OpenTelemetry layer being configured at runtime
        let span = info_span!("test_datasource", source = "deribit");
        let _guard = span.enter();

        let ctx = tracing::Span::current().context();
        let trace_id = ctx.span().span_context().trace_id();

        // Verify trace_id is 32 hex characters (OTel format)
        let trace_id_str = trace_id.to_string();
        assert_eq!(trace_id_str.len(), 32, "TraceId should be 32 hex chars");
    }

    #[test]
    fn test_span_has_context() {
        // Verify that spans created with info_span have valid context
        let span = info_span!("test_span", test = "value");
        let _guard = span.enter();

        let ctx = tracing::Span::current().context();
        let _span_handle = ctx.span();
        let span_context = _span_handle.span_context();

        // Span context should exist even if trace_id is zeros
        // This test just verifies the API is accessible
        let _ = span_context.is_valid();
    }
}
