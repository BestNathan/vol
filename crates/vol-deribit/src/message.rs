//! Deribit JSON-RPC message types.
//!
//! Deribit uses JSON-RPC 2.0 for all WebSocket communication.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{DeribitTicker, OptionMarkPrice, PortfolioData, PriceIndex, Trade};

/// Channel type enum - type-safe channel binding at compile time
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ChannelType {
    /// Options mark price with IV (markprice.options.<INDEX>)
    MarkpriceOptions(String),
    /// Index price (deribit_price_index.<INDEX>)
    PriceIndex(String),
    /// Ticker data (ticker.<BASE>)
    Ticker(String),
    /// Trade executions (trades.<INSTRUMENT>)
    Trade(String),
    /// Portfolio data (user.portfolio.<CURRENCY>)
    UserPortfolio(String),
}

impl ChannelType {
    /// Get the channel name string for subscription
    pub fn channel_name(&self) -> String {
        match self {
            ChannelType::MarkpriceOptions(idx) => crate::subscription::markprice_options(idx),
            ChannelType::PriceIndex(idx) => crate::subscription::deribit_price_index(idx),
            ChannelType::Ticker(base) => crate::subscription::ticker_base(base),
            ChannelType::Trade(instrument) => crate::subscription::trades(instrument),
            ChannelType::UserPortfolio(currency) => {
                format!("user.portfolio.{}", currency)
            }
        }
    }
}

/// Unified channel data enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChannelData {
    OptionMarkPrice(Vec<OptionMarkPrice>),
    PriceIndex(PriceIndex),
    Ticker(DeribitTicker),
    Trade(Trade),
    Portfolio(PortfolioData),
}

impl ChannelData {
    /// Get channel name for logging
    pub fn channel_name(&self) -> &'static str {
        match self {
            ChannelData::OptionMarkPrice(_) => "markprice.options",
            ChannelData::PriceIndex(_) => "deribit_price_index",
            ChannelData::Ticker(_) => "ticker",
            ChannelData::Trade(_) => "trades",
            ChannelData::Portfolio(_) => "user.portfolio",
        }
    }
}

/// JSON-RPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest<T = Value> {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<T>,
}

/// JSON-RPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct JsonRpcResponse<T = Value> {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// Microsecond timestamp when request was received by server
    #[serde(default)]
    pub usIn: Option<u64>,
    /// Microsecond timestamp when response was sent by server
    #[serde(default)]
    pub usOut: Option<u64>,
    /// Difference between usOut and usIn (processing time in microseconds)
    #[serde(default)]
    pub usDiff: Option<u64>,
    /// Whether this is testnet
    #[serde(default)]
    pub testnet: Option<bool>,
}

/// JSON-RPC error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i64,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(default)]
    pub data: Option<Value>,
}

/// Subscription notification from Deribit
///
/// Format:
/// ```json
/// {
///   "jsonrpc": "2.0",
///   "method": "subscription",
///   "params": {
///     "channel": "ticker.BTC",
///     "data": [...]
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionNotification<T = Value> {
    pub jsonrpc: String,
    pub method: String,
    pub params: SubscriptionParams<T>,
}

/// Subscription notification parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionParams<T = Value> {
    /// Channel name (e.g., "ticker.BTC", "markprice.options.btc_usd")
    pub channel: String,
    /// Data payload - use Vec<T> for array responses, T for single object
    pub data: T,
}

/// Untagged enum for Deribit WebSocket notifications.
///
/// Serde tries variants in order, using fail-fast deserialization.
/// Order matters: more specific types (with required fields) come first.
/// Trade comes before Ticker because DeribitTicker has all optional fields.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DeribitNotification {
    Markprice(SubscriptionNotification<Vec<OptionMarkPrice>>),
    PriceIndex(SubscriptionNotification<PriceIndex>),
    Trade(SubscriptionNotification<Vec<Trade>>),
    Ticker(SubscriptionNotification<Vec<DeribitTicker>>),
    Portfolio(SubscriptionNotification<PortfolioData>),
}

/// Subscription request to public channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicSubscribeParams {
    pub channels: Vec<String>,
}

impl PublicSubscribeParams {
    pub fn new(channels: Vec<String>) -> Self {
        Self { channels }
    }
}

/// Subscription request to private channels (requires auth)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateSubscribeParams {
    pub channels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Ping request (keep-alive)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping: Option<String>,
}

/// Auth request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthParams {
    /// Client ID (API key)
    pub client_id: String,
    /// Request signature
    pub signature: String,
    /// Timestamp (milliseconds since Unix epoch)
    pub timestamp: String,
    /// Nonce
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

impl JsonRpcRequest<PublicSubscribeParams> {
    /// Create a new public subscription request
    pub fn subscribe(channels: Vec<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "public/subscribe".to_string(),
            params: Some(PublicSubscribeParams::new(channels)),
        }
    }
}

impl JsonRpcRequest<PingRequest> {
    /// Create a ping request
    pub fn ping() -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "public/ping".to_string(),
            params: Some(PingRequest { ping: None }),
        }
    }
}

impl JsonRpcRequest<AuthParams> {
    /// Create an auth request
    pub fn auth(client_id: String, signature: String, timestamp: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "public/auth".to_string(),
            params: Some(AuthParams {
                client_id,
                signature,
                timestamp,
                nonce: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_request() {
        let req = JsonRpcRequest::subscribe(vec!["ticker.BTC".to_string()]);
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "public/subscribe");
        assert_eq!(req.params.unwrap().channels.len(), 1);
    }

    #[test]
    fn test_notification_parse() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "markprice.options.btc_usd",
                "data": [
                    {"instrument_name": "BTC-29MAR24-70000-C", "mark_price": 1000, "iv": 0.7, "timestamp": 1234567890}
                ]
            }
        }"#;

        let notification: SubscriptionNotification<Vec<OptionMarkPrice>> =
            serde_json::from_str(json).unwrap();

        assert_eq!(notification.method, "subscription");
        assert_eq!(notification.params.channel, "markprice.options.btc_usd");
        assert_eq!(notification.params.data.len(), 1);
    }

    #[test]
    fn test_markprice_notification() {
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
                    },
                    {
                        "instrument_name": "BTC-29MAR24-70000-P",
                        "mark_price": 890.25,
                        "iv": 0.68,
                        "timestamp": 1743456789000,
                        "price": 95000.00
                    }
                ]
            }
        }"#;

        let notification: DeribitNotification = serde_json::from_str(json).unwrap();

        match notification {
            DeribitNotification::Markprice(n) => {
                assert_eq!(n.params.channel, "markprice.options.btc_usd");
                assert_eq!(n.params.data.len(), 2);
                assert_eq!(n.params.data[0].instrument_name, "BTC-29MAR24-70000-C");
                assert!((n.params.data[0].mark_price - 1250.50).abs() < f64::EPSILON);
                assert!(n.params.data[0].iv.unwrap() - 0.72 < f64::EPSILON);
            }
            _ => panic!("Expected Markprice variant"),
        }
    }

    #[test]
    fn test_price_index_notification() {
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

        let notification: DeribitNotification = serde_json::from_str(json).unwrap();

        match notification {
            DeribitNotification::PriceIndex(n) => {
                assert_eq!(n.params.channel, "deribit_price_index.btc_usd");
                assert_eq!(n.params.data.index_name, "btc_usd");
                assert!((n.params.data.price - 95123.45).abs() < f64::EPSILON);
                assert_eq!(n.params.data.timestamp, 1743456789000);
            }
            _ => panic!("Expected PriceIndex variant"),
        }
    }

    #[test]
    fn test_ticker_notification() {
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
                        "iv": 0.65,
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

        let notification: DeribitNotification = serde_json::from_str(json).unwrap();

        match notification {
            DeribitNotification::Ticker(n) => {
                assert_eq!(n.params.channel, "ticker.BTC");
                assert_eq!(n.params.data.len(), 1);
                assert_eq!(n.params.data[0].instrument_name, "BTC-PERPETUAL");
                assert!(n.params.data[0].last.unwrap() - 95100.00 < f64::EPSILON);
                assert!(n.params.data[0].mark_iv.unwrap() - 0.65 < f64::EPSILON);
            }
            _ => panic!("Expected Ticker variant"),
        }
    }

    #[test]
    fn test_trade_notification() {
        // Deribit trade data has specific fields that distinguish it from ticker data
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
                        "block_trade": false,
                        "tick_direction": 0,
                        "buyer_is_maker": false
                    }
                ]
            }
        }"#;

        let notification: DeribitNotification = serde_json::from_str(json).unwrap();

        match notification {
            DeribitNotification::Trade(n) => {
                assert_eq!(n.params.channel, "trades.BTC-PERPETUAL");
                assert_eq!(n.params.data.len(), 1);
                assert_eq!(n.params.data[0].trade_id, "12345678");
                assert_eq!(n.params.data[0].instrument_name, "BTC-PERPETUAL");
                assert!((n.params.data[0].price - 95050.00).abs() < f64::EPSILON);
                assert_eq!(n.params.data[0].amount, 1500.0);
                assert_eq!(n.params.data[0].direction, "buy");
            }
            _ => panic!("Expected Trade variant"),
        }
    }

    #[test]
    fn test_portfolio_notification() {
        // Test portfolio notification parsing - requires Portfolio variant
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "user.portfolio.BTC",
                "data": {
                    "currency": "BTC",
                    "equity": 1.5,
                    "balance": 1.2,
                    "available_funds": 0.8,
                    "margin_balance": 1.3,
                    "initial_margin": 0.5,
                    "maintenance_margin": 0.25,
                    "session_upl": 0.01,
                    "session_rpl": 0.005,
                    "total_pl": 0.15,
                    "delta_total": 0.25,
                    "options_delta": 0.2,
                    "options_gamma": 0.05,
                    "options_theta": -0.01,
                    "options_vega": 0.1,
                    "options_value": 0.8,
                    "options_pl": 0.08,
                    "futures_pl": 0.02,
                    "portfolio_margining_enabled": true,
                    "margin_model": "portfolio"
                }
            }
        }"#;

        let notification: DeribitNotification = serde_json::from_str(json).unwrap();

        match notification {
            DeribitNotification::Portfolio(n) => {
                assert_eq!(n.params.channel, "user.portfolio.BTC");
                assert_eq!(n.params.data.currency, "BTC");
                assert!((n.params.data.equity - 1.5).abs() < f64::EPSILON);
                assert!((n.params.data.available_funds - 0.8).abs() < f64::EPSILON);
                assert!((n.params.data.delta_total - 0.25).abs() < f64::EPSILON);
            }
            _ => panic!("Expected Portfolio variant"),
        }
    }
}
