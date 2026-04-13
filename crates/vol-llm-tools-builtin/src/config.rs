//! Tool-specific configuration structs.
//!
//! Each tool defines its own config type that is deserialized from the
//! dynamic `ToolConfig` container. The config struct includes the tool's
//! own settings plus a `ProxyConfig` for proxy support.

use serde::Deserialize;
use vol_llm_tool::ProxyConfig;

/// Configuration for the web search tool.
///
/// Deserialized from TOML under the `web_search` key:
/// ```toml
/// [tools.web_search]
/// provider = "tavily"
/// api_key = "${TAVILY_API_KEY}"
///
/// [tools.web_search.proxy]
/// proxy_url = "http://proxy:8080"
/// ```
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct WebSearchConfig {
    /// Search provider name (e.g., "tavily")
    pub provider: String,
    /// API key for the search provider (supports `${ENV_VAR}` expansion)
    pub api_key: String,
    /// Proxy configuration (optional)
    #[serde(default)]
    pub proxy: ProxyConfig,
}

/// Configuration for the web fetch tool.
///
/// Deserialized from TOML under the `web_fetch` key:
/// ```toml
/// [tools.web_fetch]
/// max_content_length = 1048576
///
/// [tools.web_fetch.proxy]
/// proxy_url = "http://proxy:8080"
/// ```
#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
pub struct WebFetchConfig {
    /// Maximum content length in bytes (default: 2MB)
    pub max_content_length: Option<usize>,
    /// Proxy configuration (optional)
    #[serde(default)]
    pub proxy: ProxyConfig,
}
