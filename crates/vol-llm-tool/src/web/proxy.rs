//! Proxy configuration for web tools.
//!
//! A standalone, reusable proxy configuration that can be embedded in any tool's config struct.

use serde::Deserialize;

/// Proxy configuration for HTTP clients.
///
/// Can be embedded in any tool config struct to provide proxy support.
#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
pub struct ProxyConfig {
    /// Proxy URL (e.g., `http://proxy.example.com:8080` or `socks5://...`)
    pub proxy_url: Option<String>,
}
