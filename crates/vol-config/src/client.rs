//! Client configuration types.
//!
//! Transport-layer configuration for different client types (Deribit, Binance, etc.).
//! Each client type is configured globally under [clients] section.

use serde::{Deserialize, Serialize};

/// Deribit authentication configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeribitAuthConfig {
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
}

impl DeribitAuthConfig {
    /// Get client ID, checking environment variables first.
    pub fn client_id(&self) -> Option<String> {
        std::env::var("DERIBIT_CLIENT_ID")
            .ok()
            .or_else(|| self.client_id.clone())
    }

    /// Get client secret, checking environment variables first.
    pub fn client_secret(&self) -> Option<String> {
        std::env::var("DERIBIT_CLIENT_SECRET")
            .ok()
            .or_else(|| self.client_secret.clone())
    }
}

/// Deribit client configuration - transport layer settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitClientConfig {
    pub ws_url: String,
    #[serde(default)]
    pub auth: Option<DeribitAuthConfig>,
}

/// Binance client configuration - transport layer settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceClientConfig {
    pub api_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

/// Global client configurations - all available clients
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientConfigs {
    #[serde(default)]
    pub deribit: Option<DeribitClientConfig>,
    #[serde(default)]
    pub binance: Option<BinanceClientConfig>,
}
