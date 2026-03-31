//! Deribit WebSocket client implementation.
//!
//! Provides low-level WebSocket connection, subscription management,
//! and message parsing for Deribit API v2.

use std::sync::Arc;
use std::time::Duration;
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{info, warn, error, debug};
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    WebSocketStream,
};
use tokio_rustls::TlsConnector;
use rustls::{RootCertStore, ClientConfig, pki_types::ServerName};
use webpki_roots::TLS_SERVER_ROOTS;

use crate::{ChannelType, ChannelData, DeribitNotification, OptionMarkPrice, SubscriptionNotification};
use crate::subscription_manager::SubscriptionManager;
use vol_core::VolatilityData;

/// Type alias for WebSocket write half
type WsWriter = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
type WsSplitSink = futures_util::stream::SplitSink<WsWriter, Message>;

/// Deribit WebSocket client state
#[derive(Debug, Clone)]
pub struct ClientState {
    pub connected: bool,
    pub subscriptions: Vec<String>,
    connection_started: bool,  // Track if connection task has been started
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            connected: false,
            subscriptions: Vec::new(),
            connection_started: false,
        }
    }
}

/// Deribit WebSocket client for low-level connection management
pub struct DeribitClient {
    ws_url: String,
    state: Arc<Mutex<ClientState>>,
    proxy_url: Option<String>,
    subscription_manager: Arc<SubscriptionManager>,
    subscribed_channels: Arc<Mutex<Vec<ChannelType>>>,
    /// Shared write sender for dynamic subscription updates
    /// Outer Arc<Mutex<>> allows cloning the client, inner Option<Arc<Mutex<>>> allows sharing the writer
    ws_sender: Arc<Mutex<Option<Arc<Mutex<WsSplitSink>>>>>,
}

impl DeribitClient {
    /// Create a new Deribit client
    pub fn new(ws_url: impl Into<String>) -> Self {
        Self {
            ws_url: ws_url.into(),
            state: Arc::new(Mutex::new(ClientState::default())),
            proxy_url: None,
            subscription_manager: Arc::new(SubscriptionManager::new()),
            subscribed_channels: Arc::new(Mutex::new(Vec::new())),
            ws_sender: Arc::new(Mutex::new(None)),
        }
    }

    /// Configure HTTP proxy
    pub fn with_proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.proxy_url = Some(proxy_url.into());
        self
    }

    /// Get connection state
    pub async fn is_connected(&self) -> bool {
        self.state.lock().await.connected
    }

    /// Get current subscriptions
    pub async fn subscriptions(&self) -> Vec<String> {
        self.state.lock().await.subscriptions.clone()
    }

    /// Run the WebSocket reader loop and stream parsed messages
    #[deprecated(note = "Use subscribe(ChannelType) instead")]
    pub async fn run(
        &self,
        channels: Vec<String>,
        tx: mpsc::Sender<VolatilityData>,
    ) {
        let mut retry_delay = Duration::from_secs(1);
        const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

        loop {
            info!("Connecting to Deribit WebSocket: {}", self.ws_url);
            if let Some(proxy) = &self.proxy_url {
                info!("Using proxy: {}", proxy);
            }

            let connect_result = if let Some(proxy) = &self.proxy_url {
                Self::connect_via_proxy(&self.ws_url, proxy).await
            } else {
                connect_async(&self.ws_url).await.map(|(ws, _)| ws).ok()
            };

            match connect_result {
                Some(ws_stream) => {
                    info!("Connected to Deribit");

                    // Update state
                    {
                        let mut s = self.state.lock().await;
                        s.connected = true;
                        s.subscriptions = channels.clone();
                    }

                    let (mut write, mut read) = ws_stream.split();

                    // Send subscription message
                    let subscribe_msg = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "public/subscribe",
                        "params": {
                            "channels": &channels
                        }
                    });

                    if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                        error!("Failed to send subscription: {}", e);
                        continue;
                    }

                    info!("Subscribed to channels: {:?}", channels);

                    // Read messages
                    while let Some(msg_result) = read.next().await {
                        match msg_result {
                            Ok(Message::Text(text)) => {
                                if let Some(vol_data) = Self::parse_message(&text) {
                                    for vol in vol_data {
                                        if let Err(e) = tx.send(vol).await {
                                            warn!("Failed to send volatility data: {}", e);
                                            break;
                                        }
                                    }
                                }
                            }
                            Ok(Message::Ping(data)) => {
                                if let Err(e) = write.send(Message::Pong(data)).await {
                                    warn!("Failed to send pong: {}", e);
                                }
                            }
                            Ok(Message::Close(frame)) => {
                                warn!("WebSocket closed: {:?}", frame);
                                break;
                            }
                            Ok(_) => {}
                            Err(e) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                        }
                    }

                    // Connection lost - reset state
                    {
                        let mut s = self.state.lock().await;
                        s.connected = false;
                        s.subscriptions = Vec::new();
                    }
                }
                None => {
                    error!("Failed to connect to Deribit");
                }
            }

            // Wait before reconnecting (exponential backoff)
            tokio::time::sleep(retry_delay).await;
            retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY);
        }
    }

    /// Connect to WebSocket via HTTP proxy using CONNECT tunnel
    async fn connect_via_proxy(
        ws_url: &str,
        proxy_url: &str,
    ) -> Option<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> {
        use tokio::net::TcpStream;
        use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
        use tokio_tungstenite::{client_async, MaybeTlsStream};

        let ws_url_owned = ws_url.to_string();

        // Parse proxy host:port
        let proxy_parts: Vec<&str> = proxy_url.trim_start_matches("http://").split(':').collect();
        if proxy_parts.len() != 2 {
            error!("Invalid proxy URL format: {}", proxy_url);
            return None;
        }
        let proxy_host = proxy_parts[0];
        let proxy_port: u16 = proxy_parts[1].parse().ok()?;

        // Parse WebSocket host for CONNECT request
        let ws_host = ws_url.trim_start_matches("wss://").trim_start_matches("ws://");
        let ws_host_parts: Vec<&str> = ws_host.split('/').collect();
        let target_host = format!("{}:443", ws_host_parts[0]);

        // Connect to proxy
        let mut proxy_stream = TcpStream::connect((proxy_host, proxy_port)).await.ok()?;

        // Send CONNECT request
        let connect_request = format!(
            "CONNECT {} HTTP/1.1\r\nHost: {}\r\nProxy-Connection: Keep-Alive\r\n\r\n",
            target_host, target_host
        );
        proxy_stream.write_all(connect_request.as_bytes()).await.ok()?;

        // Read response
        let mut reader = BufReader::new(proxy_stream);
        let mut response = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.ok()?;
            if line == "\r\n" || line.is_empty() {
                break;
            }
            response.push_str(&line);
        }

        // Check if CONNECT succeeded (2xx status)
        if !response.contains("200") {
            error!("Proxy CONNECT failed: {}", response);
            return None;
        }

        info!("Proxy tunnel established");

        // Convert back to TcpStream for TLS upgrade
        let tcp_stream = reader.into_inner();

        // Create TLS configuration with webpki roots
        let mut root_store = RootCertStore::empty();
        root_store.extend(TLS_SERVER_ROOTS.iter().cloned());

        let config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        // Extract domain for TLS (without port)
        let domain = target_host.split(':').next().unwrap();
        let server_name = ServerName::try_from(domain)
            .ok()?
            .to_owned();
        let tls_connector = Arc::new(config);

        // Perform TLS handshake
        let tls_result = TlsConnector::from(tls_connector)
            .connect(server_name, tcp_stream)
            .await;

        match tls_result {
            Ok(tls_stream) => {
                info!("TLS connection established to {}", domain);

                let stream = MaybeTlsStream::Rustls(tls_stream);

                info!("Attempting WebSocket upgrade to {}", ws_url_owned);
                match client_async(&ws_url_owned, stream).await {
                    Ok((ws_stream, response)) => {
                        info!("WebSocket upgrade successful: {:?}", response.status());
                        Some(ws_stream)
                    }
                    Err(e) => {
                        error!("WebSocket upgrade failed: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                error!("TLS handshake failed: {:?}", e);
                None
            }
        }
    }

    /// Parse incoming WebSocket message into VolatilityData
    #[allow(deprecated)]
    pub fn parse_message(text: &str) -> Option<Vec<VolatilityData>> {
        let notification: SubscriptionNotification<Vec<OptionMarkPrice>> =
            serde_json::from_str(text).ok()?;

        if notification.method != "subscription" {
            return None;
        }

        let vol_data = notification
            .params
            .data
            .into_iter()
            .filter_map(|mark_price| mark_price.to_volatility_data())
            .collect();

        Some(vol_data)
    }

    /// Subscribe to a channel and return data stream
    ///
    /// This method:
    /// 1. Registers the channel with the subscription manager
    /// 2. Adds the channel to subscribed_channels list
    /// 3. Starts connection if not already running
    /// 4. Sends a full resubscription to the server with ALL channels
    ///
    /// Subscription is decoupled from connection - each call sends
    /// a complete subscription list to ensure all channels are active.
    pub async fn subscribe(
        &self,
        channel: ChannelType,
    ) -> Result<mpsc::Receiver<ChannelData>, vol_core::VolError> {
        // Register subscriber and get receiver
        let rx = self.subscription_manager.register(channel.clone()).await;

        // Add to subscribed_channels if new
        let is_new = {
            let mut channels = self.subscribed_channels.lock().await;
            let is_new = !channels.contains(&channel);
            if is_new {
                channels.push(channel.clone());
            }
            is_new
        };

        // Start connection if not already running
        let should_start = {
            let state = self.state.lock().await;
            is_new && !state.connection_started
        };

        if should_start {
            self.start_connection().await;
        }

        // Send full subscription to server with all current channels
        // This will be queued if not connected, or sent immediately if already connected
        self.send_full_subscription().await;

        Ok(rx)
    }

    /// Send subscription message to server with all current channels
    async fn send_full_subscription(&self) {
        // Get all channels to subscribe to
        let channels: Vec<String> = {
            let channel_types = self.subscribed_channels.lock().await.clone();
            channel_types.iter().map(|c| c.channel_name()).collect()
        };

        // Get current subscriptions to check if we need to resubscribe
        let current_subs = {
            let state = self.state.lock().await;
            state.subscriptions.clone()
        };

        // Check if subscription already matches
        let current_set: std::collections::HashSet<_> = current_subs.iter().collect();
        let new_set: std::collections::HashSet<_> = channels.iter().collect();

        if current_set == new_set && !channels.is_empty() {
            debug!("Subscription unchanged, skipping: {:?}", channels);
            return;
        }

        // Get the WebSocket sender
        let sender_outer = self.ws_sender.lock().await;
        if let Some(write_arc) = sender_outer.as_ref() {
            let subscribe_msg = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "public/subscribe",
                "params": {
                    "channels": channels.iter().map(|s| s.as_str()).collect::<Vec<&str>>()
                }
            });

            let mut writer = write_arc.lock().await;
            match writer.send(Message::Text(subscribe_msg.to_string())).await {
                Ok(_) => {
                    info!("Sent subscription for channels: {:?}", channels);
                    // Update state after successful send
                    {
                        let mut state = self.state.lock().await;
                        state.subscriptions = channels.clone();
                    }
                }
                Err(e) => {
                    error!("Failed to send subscription: {}", e);
                }
            }
        } else {
            debug!("WebSocket not connected, subscription queued: {:?}", channels);
        }
    }

    /// Start the WebSocket connection and reader loop
    ///
    /// Connection is decoupled from subscription management:
    /// - This method only manages the WebSocket lifecycle
    /// - Subscriptions are sent after each connect using the full channel list
    /// - subscribe() can be called at any time to add channels
    async fn start_connection(&self) {
        // Mark connection as started before spawning
        {
            let mut state = self.state.lock().await;
            state.connection_started = true;
        }

        let ws_url = self.ws_url.clone();
        let proxy_url = self.proxy_url.clone();
        let manager = self.subscription_manager.clone();
        let channels = self.subscribed_channels.clone();
        let state = self.state.clone();
        let ws_sender = self.ws_sender.clone();

        tokio::spawn(async move {
            let mut retry_delay = Duration::from_secs(1);
            const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

            loop {
                info!("Connecting to Deribit WebSocket: {}", ws_url);
                if let Some(proxy) = &proxy_url {
                    info!("Using proxy: {}", proxy);
                }

                let connect_result = if let Some(proxy) = &proxy_url {
                    Self::connect_via_proxy(&ws_url, proxy).await
                } else {
                    connect_async(&ws_url).await.map(|(ws, _)| ws).ok()
                };

                match connect_result {
                    Some(ws_stream) => {
                        info!("Connected to Deribit");

                        let (mut write, mut read) = ws_stream.split();

                        // Wrap writer in Arc for sharing
                        let write_arc = Arc::new(Mutex::new(write));

                        // Get the FULL list of channels at connect time
                        let channel_types = channels.lock().await.clone();
                        let channel_names: Vec<String> = channel_types
                            .iter()
                            .map(|c| c.channel_name())
                            .collect();

                        // Send initial subscription using the wrapped writer
                        {
                            let mut writer = write_arc.lock().await;
                            let subscribe_msg = serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": 1,
                                "method": "public/subscribe",
                                "params": {
                                    "channels": channel_names.iter().map(|s| s.as_str()).collect::<Vec<&str>>()
                                }
                            });

                            if let Err(e) = writer.send(Message::Text(subscribe_msg.to_string())).await {
                                error!("Failed to send subscription: {}", e);
                                continue;
                            }
                        }

                        info!("Subscribed to channels: {:?}", channel_names);

                        // Update state
                        {
                            let mut s = state.lock().await;
                            s.connected = true;
                            s.subscriptions = channel_names.clone();
                        }

                        // Store the Arc<Mutex<write>> for dynamic subscriptions
                        {
                            let mut sender_guard = ws_sender.lock().await;
                            *sender_guard = Some(write_arc.clone());
                        }

                        // Read and dispatch
                        while let Some(msg_result) = read.next().await {
                            match msg_result {
                                Ok(Message::Text(text)) => {
                                    if let Some((channel_type, data)) = Self::parse_and_route(&text) {
                                        manager.dispatch(&channel_type, data).await;
                                    }
                                }
                                Ok(Message::Ping(data)) => {
                                    let mut writer = write_arc.lock().await;
                                    if let Err(e) = writer.send(Message::Pong(data)).await {
                                        warn!("Failed to send pong: {}", e);
                                    }
                                }
                                Ok(Message::Close(frame)) => {
                                    warn!("WebSocket closed: {:?}", frame);
                                    break;
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    error!("WebSocket error: {}", e);
                                    break;
                                }
                            }
                        }

                        // Connection lost - reset state
                        {
                            let mut s = state.lock().await;
                            s.connected = false;
                            s.connection_started = false;
                            s.subscriptions = Vec::new();
                        }

                        // Clear sender
                        {
                            let mut sender_guard = ws_sender.lock().await;
                            *sender_guard = None;
                        }
                    }
                    None => {
                        error!("Failed to connect to Deribit");
                    }
                }

                // Wait before reconnecting (exponential backoff)
                tokio::time::sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY);
        }
        });
    }

    /// Parse message and extract channel type and data.
    ///
    /// Uses a single JSON parse with an untagged enum to discriminate
    /// the notification type, which is more efficient than trying
    /// each type sequentially.
    fn parse_and_route(text: &str) -> Option<(ChannelType, ChannelData)> {
        let notification = serde_json::from_str::<DeribitNotification>(text).ok()?;

        match notification {
            DeribitNotification::Markprice(n) => {
                if n.method != "subscription" {
                    return None;
                }
                let index = n.params.channel.strip_prefix("markprice.options.")?.to_string();
                Some((ChannelType::MarkpriceOptions(index), ChannelData::OptionMarkPrice(n.params.data)))
            }
            DeribitNotification::PriceIndex(n) => {
                if n.method != "subscription" {
                    return None;
                }
                let index = n.params.channel.strip_prefix("deribit_price_index.")?.to_string();
                Some((ChannelType::PriceIndex(index), ChannelData::PriceIndex(n.params.data)))
            }
            DeribitNotification::Ticker(n) => {
                if n.method != "subscription" {
                    return None;
                }
                let base = n.params.channel.strip_prefix("ticker.")?.split('.').next()?.to_string();
                let ticker = n.params.data.into_iter().next()?;
                Some((ChannelType::Ticker(base), ChannelData::Ticker(ticker)))
            }
            DeribitNotification::Trade(n) => {
                if n.method != "subscription" {
                    return None;
                }
                let instrument = n.params.channel.strip_prefix("trades.")?.to_string();
                let trade = n.params.data.into_iter().next()?;
                Some((ChannelType::Trade(instrument), ChannelData::Trade(trade)))
            }
        }
    }
}

impl Clone for DeribitClient {
    fn clone(&self) -> Self {
        Self {
            ws_url: self.ws_url.clone(),
            state: self.state.clone(),
            proxy_url: self.proxy_url.clone(),
            subscription_manager: self.subscription_manager.clone(),
            subscribed_channels: self.subscribed_channels.clone(),
            ws_sender: self.ws_sender.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_multiple_subscriptions_share_connection() {
        let client = DeribitClient::new("wss://www.deribit.com/ws/api/v2");

        // Subscribe to multiple channels
        let _rx1 = client.subscribe(ChannelType::PriceIndex("btc_usd".to_string())).await;
        let _rx2 = client.subscribe(ChannelType::PriceIndex("eth_usd".to_string())).await;
        let _rx3 = client.subscribe(ChannelType::MarkpriceOptions("btc_usd".to_string())).await;

        // Give connection time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify all channels are tracked in subscribed_channels
        let subscribed = client.subscribed_channels.lock().await;
        assert_eq!(subscribed.len(), 3, "All 3 subscriptions should be tracked");

        // Verify channel types are correctly stored
        assert!(matches!(&subscribed[0], ChannelType::PriceIndex(idx) if idx == "btc_usd"));
        assert!(matches!(&subscribed[1], ChannelType::PriceIndex(idx) if idx == "eth_usd"));
        assert!(matches!(&subscribed[2], ChannelType::MarkpriceOptions(idx) if idx == "btc_usd"));
    }
}
