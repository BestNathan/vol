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

impl DeribitClientConfig {
    /// Get client ID, checking config first then environment variables.
    /// This works even when `auth` section is not present in config.
    pub fn client_id(&self) -> Option<String> {
        // Try config first
        if let Some(ref auth) = self.auth {
            if let Some(ref id) = auth.client_id {
                return Some(id.clone());
            }
        }
        // Fallback to environment
        std::env::var("DERIBIT_CLIENT_ID").ok()
    }

    /// Get client secret, checking config first then environment variables.
    /// This works even when `auth` section is not present in config.
    pub fn client_secret(&self) -> Option<String> {
        // Try config first
        if let Some(ref auth) = self.auth {
            if let Some(ref secret) = auth.client_secret {
                return Some(secret.clone());
            }
        }
        // Fallback to environment
        std::env::var("DERIBIT_CLIENT_SECRET").ok()
    }

    /// Check if both client ID and secret are available (from config or env).
    pub fn has_auth(&self) -> bool {
        self.client_id().is_some() && self.client_secret().is_some()
    }
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
