//! Deribit WebSocket client implementation.
//!
//! Provides low-level WebSocket connection, subscription management,
//! and message parsing for Deribit API v2.

use std::sync::Arc;
use std::time::Duration;
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{info, warn, error};
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
};
use tokio_rustls::TlsConnector;
use rustls::{RootCertStore, ClientConfig, pki_types::ServerName};
use webpki_roots::TLS_SERVER_ROOTS;

use crate::{ChannelType, ChannelData, OptionMarkPrice, PriceIndex, DeribitTicker, Trade, SubscriptionNotification};
use crate::subscription_manager::SubscriptionManager;
use vol_core::VolatilityData;

/// Deribit WebSocket client state
#[derive(Debug, Clone)]
pub struct ClientState {
    pub connected: bool,
    pub subscriptions: Vec<String>,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            connected: false,
            subscriptions: Vec::new(),
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
    pub async fn subscribe(
        &self,
        channel: ChannelType,
    ) -> Result<mpsc::Receiver<ChannelData>, vol_core::VolError> {
        let (tx, rx) = mpsc::channel(1024);

        // Get channel name string for subscription
        let channel_name = channel.channel_name();

        // Store channel type for runtime dispatch
        let channel_type = channel.clone();

        // Spawn WebSocket message processing loop
        let ws_url = self.ws_url.clone();
        let proxy_url = self.proxy_url.clone();

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

                        // Send subscription message
                        let subscribe_msg = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "public/subscribe",
                            "params": {
                                "channels": [&channel_name]
                            }
                        });

                        if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                            error!("Failed to send subscription: {}", e);
                            continue;
                        }

                        info!("Subscribed to channel: {}", channel_name);

                        // Read messages
                        while let Some(msg_result) = read.next().await {
                            match msg_result {
                                Ok(Message::Text(text)) => {
                                    if let Some(data) = Self::parse_channel_message(&text, &channel_type) {
                                        if let Err(e) = tx.send(data).await {
                                            warn!("Failed to send channel data: {}", e);
                                            break;
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

        Ok(rx)
    }

    /// Parse channel message based on channel type
    fn parse_channel_message(text: &str, channel_type: &ChannelType) -> Option<ChannelData> {
        match channel_type {
            ChannelType::MarkpriceOptions(_) => {
                let notification: SubscriptionNotification<Vec<OptionMarkPrice>> = serde_json::from_str(text).ok()?;
                Some(ChannelData::OptionMarkPrice(notification.params.data))
            }
            ChannelType::PriceIndex(_) => {
                let notification: SubscriptionNotification<PriceIndex> = serde_json::from_str(text).ok()?;
                Some(ChannelData::PriceIndex(notification.params.data))
            }
            ChannelType::Ticker(_) => {
                let notification: SubscriptionNotification<Vec<DeribitTicker>> = serde_json::from_str(text).ok()?;
                Some(ChannelData::Ticker(notification.params.data.into_iter().next()?))
            }
            ChannelType::Trade(_) => {
                let notification: SubscriptionNotification<Vec<Trade>> = serde_json::from_str(text).ok()?;
                Some(ChannelData::Trade(notification.params.data.into_iter().next()?))
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
        }
    }
}
