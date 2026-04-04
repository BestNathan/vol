//! Deribit WebSocket client implementation.
//!
//! Provides low-level WebSocket connection, subscription management,
//! and message parsing for Deribit API v2.

use futures_util::{SinkExt, StreamExt};
use rustls::{pki_types::ServerName, ClientConfig, RootCertStore};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info, warn};
use webpki_roots::TLS_SERVER_ROOTS;

use crate::positions::{Position, PortfolioSummary};
use crate::subscription_manager::SubscriptionManager;
use crate::{
    ChannelData, ChannelType, DeribitNotification, OptionMarkPrice, SubscriptionNotification,
};
use vol_core::VolatilityData;

/// Type alias for WebSocket write half
type WsWriter = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
type WsSplitSink = futures_util::stream::SplitSink<WsWriter, Message>;

/// Deribit WebSocket client state
#[derive(Debug, Clone)]
pub struct ClientState {
    pub connected: bool,
    pub subscriptions: Vec<String>,
    connection_started: bool, // Track if connection task has been started
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
    /// OAuth client ID
    client_id: Option<String>,
    /// OAuth client secret
    client_secret: Option<String>,
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
            client_id: None,
            client_secret: None,
        }
    }

    /// Configure HTTP proxy
    pub fn with_proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.proxy_url = Some(proxy_url.into());
        self
    }

    /// Configure OAuth authentication
    pub fn with_auth(mut self, client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self.client_secret = Some(client_secret.into());
        self
    }

    /// Check if authentication is configured
    pub fn has_auth(&self) -> bool {
        self.client_id.is_some() && self.client_secret.is_some()
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
    pub async fn run(&self, channels: Vec<String>, tx: mpsc::Sender<VolatilityData>) {
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
    ) -> Option<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    > {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpStream;
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
        let ws_host = ws_url
            .trim_start_matches("wss://")
            .trim_start_matches("ws://");
        let ws_host_parts: Vec<&str> = ws_host.split('/').collect();
        let target_host = format!("{}:443", ws_host_parts[0]);

        // Connect to proxy
        let mut proxy_stream = TcpStream::connect((proxy_host, proxy_port)).await.ok()?;

        // Send CONNECT request
        let connect_request = format!(
            "CONNECT {} HTTP/1.1\r\nHost: {}\r\nProxy-Connection: Keep-Alive\r\n\r\n",
            target_host, target_host
        );
        proxy_stream
            .write_all(connect_request.as_bytes())
            .await
            .ok()?;

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
        let server_name = ServerName::try_from(domain).ok()?.to_owned();
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
            debug!(
                "WebSocket not connected, subscription queued: {:?}",
                channels
            );
        }
    }

    /// Get OAuth access token via HTTP POST (static helper for use in spawned tasks)
    async fn get_access_token_inner(client_id: &str, client_secret: &str) -> Result<String, vol_core::VolError> {
        let client = reqwest::Client::new();
        let response = client
            .post("https://www.deribit.com/api/v2/public/auth")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "public/auth",
                "params": {
                    "grant_type": "client_credentials",
                    "client_id": client_id,
                    "client_secret": client_secret
                }
            }))
            .send()
            .await
            .map_err(|e| vol_core::VolError::Auth(format!("Token request failed: {}", e)))?;

        let result: serde_json::Value = response.json().await
            .map_err(|e| vol_core::VolError::Auth(format!("Token parse failed: {}", e)))?;

        result.get("result")
            .and_then(|r| r.get("access_token"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| vol_core::VolError::Auth("No access token in response".into()))
    }

    /// Get OAuth access token (public method for use in REST API calls)
    pub async fn get_access_token(&self) -> Result<String, vol_core::VolError> {
        let client_id = self.client_id.as_ref()
            .ok_or_else(|| vol_core::VolError::Auth("No client_id configured".into()))?;
        let client_secret = self.client_secret.as_ref()
            .ok_or_else(|| vol_core::VolError::Auth("No client_secret configured".into()))?;
        Self::get_access_token_inner(client_id, client_secret).await
    }

    /// Make a REST API request to Deribit
    async fn request(&self, method: &str, params: Option<serde_json::Map<String, serde_json::Value>>) -> Result<serde_json::Value, vol_core::VolError> {
        // Get access token
        let access_token = self.get_access_token().await?;

        let client = reqwest::Client::new();
        let mut payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
        });

        if let Some(p) = params {
            payload["params"] = serde_json::Value::Object(p);
        }

        let response = client
            .post("https://www.deribit.com/api/v2/")
            .header("Authorization", format!("Bearer {}", access_token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| vol_core::VolError::Connection(format!("REST request failed: {}", e)))?;

        let result: serde_json::Value = response.json().await
            .map_err(|e| vol_core::VolError::Parse(format!("REST response parse failed: {}", e)))?;

        // Check for error response
        if let Some(error) = result.get("error") {
            return Err(vol_core::VolError::Internal(format!("Deribit API error: {}", error)));
        }

        result.get("result")
            .cloned()
            .ok_or_else(|| vol_core::VolError::Parse("No result in response".into()))
    }

    /// Get account positions via REST API
    pub async fn get_positions(&self, currency: Option<&str>) -> Result<Vec<Position>, vol_core::VolError> {
        let mut params = serde_json::Map::new();
        if let Some(curr) = currency {
            params.insert("currency".to_string(), serde_json::Value::String(curr.to_string()));
        }

        let response = self.request("private/get_positions", Some(params)).await?;
        let positions: Vec<Position> = serde_json::from_value(response)
            .map_err(|e| vol_core::VolError::Parse(format!("Position parse failed: {}", e)))?;
        Ok(positions)
    }

    /// Get portfolio/account summary via REST API
    pub async fn get_portfolio(&self, currency: &str) -> Result<PortfolioSummary, vol_core::VolError> {
        let mut params = serde_json::Map::new();
        params.insert("currency".to_string(), serde_json::Value::String(currency.to_string()));

        let response = self.request("private/get_portfolio", Some(params)).await?;
        let summary: PortfolioSummary = serde_json::from_value(response)
            .map_err(|e| vol_core::VolError::Parse(format!("Portfolio parse failed: {}", e)))?;
        Ok(summary)
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
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();

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

                        let (write, mut read) = ws_stream.split();

                        // Wrap writer in Arc for sharing
                        let write_arc = Arc::new(Mutex::new(write));

                        // Get the FULL list of channels at connect time
                        let channel_types = channels.lock().await.clone();
                        let channel_names: Vec<String> =
                            channel_types.iter().map(|c| c.channel_name()).collect();

                        // Authenticate if credentials are present
                        let access_token = if let (Some(cid), Some(csecret)) = (&client_id, &client_secret) {
                            match Self::get_access_token_inner(cid, csecret).await {
                                Ok(token) => {
                                    info!("OAuth authentication successful");
                                    Some(token)
                                }
                                Err(e) => {
                                    error!("OAuth authentication failed: {}", e);
                                    None
                                }
                            }
                        } else {
                            None
                        };

                        // Send initial subscription using the wrapped writer
                        {
                            let mut writer = write_arc.lock().await;

                            // If we have auth, send auth message first
                            if let Some(token) = &access_token {
                                let auth_msg = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": 2,
                                    "method": "private/auth",
                                    "params": {
                                        "access_token": token
                                    }
                                });

                                if let Err(e) = writer.send(Message::Text(auth_msg.to_string())).await {
                                    error!("Failed to send auth message: {}", e);
                                }

                                // Wait for auth response
                                if let Some(Ok(msg)) = read.next().await {
                                    if let Message::Text(text) = msg {
                                        if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&text) {
                                            if let Some(error) = resp.get("error") {
                                                error!("Auth failed: {}", error);
                                            } else {
                                                info!("private/auth succeeded");
                                            }
                                        }
                                    }
                                }
                            }

                            // Then send subscription
                            let subscribe_msg = serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": 1,
                                "method": "public/subscribe",
                                "params": {
                                    "channels": channel_names.iter().map(|s| s.as_str()).collect::<Vec<&str>>()
                                }
                            });

                            if let Err(e) =
                                writer.send(Message::Text(subscribe_msg.to_string())).await
                            {
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
                                    if let Some((channel_type, data)) = Self::parse_and_route(&text)
                                    {
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
                let index = n
                    .params
                    .channel
                    .strip_prefix("markprice.options.")?
                    .to_string();
                Some((
                    ChannelType::MarkpriceOptions(index),
                    ChannelData::OptionMarkPrice(n.params.data),
                ))
            }
            DeribitNotification::PriceIndex(n) => {
                if n.method != "subscription" {
                    return None;
                }
                let index = n
                    .params
                    .channel
                    .strip_prefix("deribit_price_index.")?
                    .to_string();
                Some((
                    ChannelType::PriceIndex(index),
                    ChannelData::PriceIndex(n.params.data),
                ))
            }
            DeribitNotification::Ticker(n) => {
                if n.method != "subscription" {
                    return None;
                }
                let base = n
                    .params
                    .channel
                    .strip_prefix("ticker.")?
                    .split('.')
                    .next()?
                    .to_string();
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
            DeribitNotification::Portfolio(n) => {
                if n.method != "subscription" {
                    return None;
                }
                let currency = n
                    .params
                    .channel
                    .strip_prefix("user.portfolio.")?
                    .to_string();
                Some((
                    ChannelType::UserPortfolio(currency),
                    ChannelData::Portfolio(n.params.data),
                ))
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
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
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
        let _rx1 = client
            .subscribe(ChannelType::PriceIndex("btc_usd".to_string()))
            .await;
        let _rx2 = client
            .subscribe(ChannelType::PriceIndex("eth_usd".to_string()))
            .await;
        let _rx3 = client
            .subscribe(ChannelType::MarkpriceOptions("btc_usd".to_string()))
            .await;

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

    #[test]
    fn test_parse_and_route_markprice() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "markprice.options.btc_usd",
                "data": [
                    {
                        "instrument_name": "BTC-29MAR24-70000-C",
                        "mark_price": 1250.50,
                        "iv": 0.72,
                        "timestamp": 1743456789000,
                        "price": 95000.00
                    }
                ]
            }
        }"#;

        let result = DeribitClient::parse_and_route(json).unwrap();
        let (channel_type, channel_data) = result;

        // Verify channel type
        assert!(matches!(&channel_type, ChannelType::MarkpriceOptions(idx) if idx == "btc_usd"));

        // Verify channel data
        assert!(matches!(channel_data, ChannelData::OptionMarkPrice(_)));
        if let ChannelData::OptionMarkPrice(data) = channel_data {
            assert_eq!(data.len(), 1);
            assert_eq!(data[0].instrument_name, "BTC-29MAR24-70000-C");
            assert!((data[0].mark_price - 1250.50).abs() < f64::EPSILON);
            assert!(data[0].iv.unwrap() - 0.72 < f64::EPSILON);
        }
    }

    #[test]
    fn test_parse_and_route_price_index() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "deribit_price_index.btc_usd",
                "data": {
                    "index_name": "btc_usd",
                    "price": 95123.45,
                    "timestamp": 1743456789000
                }
            }
        }"#;

        let result = DeribitClient::parse_and_route(json).unwrap();
        let (channel_type, channel_data) = result;

        // Verify channel type
        assert!(matches!(&channel_type, ChannelType::PriceIndex(idx) if idx == "btc_usd"));

        // Verify channel data
        assert!(matches!(channel_data, ChannelData::PriceIndex(_)));
        if let ChannelData::PriceIndex(data) = channel_data {
            assert_eq!(data.index_name, "btc_usd");
            assert!((data.price - 95123.45).abs() < f64::EPSILON);
            assert_eq!(data.timestamp, 1743456789000);
        }
    }

    #[test]
    fn test_parse_and_route_ticker() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "ticker.BTC",
                "data": [
                    {
                        "instrument_name": "BTC-PERPETUAL",
                        "last": 95100.00,
                        "mark": 95123.45,
                        "index_price": 95123.45,
                        "mark_iv": 0.65,
                        "timestamp": 1743456789000,
                        "best_bid_price": 95099.00,
                        "best_ask_price": 95101.00,
                        "best_bid_amount": 15000,
                        "best_ask_amount": 12000,
                        "state": "open",
                        "volume": 1234567.89
                    }
                ]
            }
        }"#;

        let result = DeribitClient::parse_and_route(json).unwrap();
        let (channel_type, channel_data) = result;

        // Verify channel type
        assert!(matches!(&channel_type, ChannelType::Ticker(base) if base == "BTC"));

        // Verify channel data
        assert!(matches!(channel_data, ChannelData::Ticker(_)));
        if let ChannelData::Ticker(data) = channel_data {
            assert_eq!(data.instrument_name, "BTC-PERPETUAL");
            assert!(data.last.unwrap() - 95100.00 < f64::EPSILON);
            assert!(data.mark_iv.unwrap() - 0.65 < f64::EPSILON);
        }
    }

    #[test]
    fn test_parse_and_route_trade() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "trades.BTC-PERPETUAL",
                "data": [
                    {
                        "trade_id": "12345678",
                        "instrument_name": "BTC-PERPETUAL",
                        "price": 95050.00,
                        "amount": 1500,
                        "timestamp": 1743456789000,
                        "direction": "buy",
                        "liquidation": false,
                        "block_trade": false
                    }
                ]
            }
        }"#;

        let result = DeribitClient::parse_and_route(json).unwrap();
        let (channel_type, channel_data) = result;

        // Verify channel type
        assert!(
            matches!(&channel_type, ChannelType::Trade(instrument) if instrument == "BTC-PERPETUAL")
        );

        // Verify channel data
        assert!(matches!(channel_data, ChannelData::Trade(_)));
        if let ChannelData::Trade(data) = channel_data {
            assert_eq!(data.trade_id, "12345678");
            assert_eq!(data.instrument_name, "BTC-PERPETUAL");
            assert!((data.price - 95050.00).abs() < f64::EPSILON);
            assert_eq!(data.amount, 1500.0);
            assert_eq!(data.direction, "buy");
        }
    }

    #[test]
    fn test_parse_and_route_wrong_method() {
        // Test with a non-subscription method (e.g., a response or error)
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"status": "ok"},
            "usIn": 1743456789000,
            "usOut": 1743456789001,
            "usDiff": 1,
            "testnet": false
        }"#;

        // Should return None for non-subscription messages
        let result = DeribitClient::parse_and_route(json);
        assert!(
            result.is_none(),
            "Should return None for non-subscription method"
        );

        // Also test with subscription method but invalid channel prefix
        let json_invalid_channel = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "unknown.channel.test",
                "data": []
            }
        }"#;

        let result = DeribitClient::parse_and_route(json_invalid_channel);
        assert!(
            result.is_none(),
            "Should return None for unknown channel prefix"
        );
    }
}
