//! Deribit JSON-RPC message types.
//!
//! Deribit uses JSON-RPC 2.0 for all WebSocket communication.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// Array of data items
    pub data: Vec<T>,
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

        let notification: SubscriptionNotification<crate::market_data::OptionMarkPrice> =
            serde_json::from_str(json).unwrap();

        assert_eq!(notification.method, "subscription");
        assert_eq!(notification.params.channel, "markprice.options.btc_usd");
        assert_eq!(notification.params.data.len(), 1);
    }
}
