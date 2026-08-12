//! vol-llm-tools-builtin-web-search: Web search and fetch tools.

pub mod tavily;

use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use vol_llm_tool::web::fetch::{FetchFn, FetchOptions, FetchStatus};
use vol_llm_tool::web::search::{SearchFn, SearchOptions};
use vol_llm_tool::{
    ExecutableTool, ProxyConfig, ToolContext, ToolError, ToolResult, ToolResultType,
};

// ==================== WebSearchTool ====================

/// Parameters for web_search tool
#[derive(Debug, Deserialize)]
pub struct WebSearchParams {
    pub query: String,
    pub num_results: Option<usize>,
    pub allowed_domains: Option<Vec<String>>,
    pub blocked_domains: Option<Vec<String>>,
    /// Optional proxy URL for this specific request.
    /// Overrides agent config and environment variables.
    pub proxy_url: Option<String>,
}

/// Web search tool — searches the web via a SearchFn provider
pub struct WebSearchTool {
    provider: Arc<dyn SearchFn>,
    proxy_config: ProxyConfig,
}

impl WebSearchTool {
    pub fn new(provider: impl SearchFn + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
            proxy_config: ProxyConfig::default(),
        }
    }

    /// Create with a specific proxy configuration (from agent config).
    pub fn with_proxy(provider: impl SearchFn + 'static, proxy_config: ProxyConfig) -> Self {
        Self {
            provider: Arc::new(provider),
            proxy_config,
        }
    }
}

#[async_trait]
impl ExecutableTool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Search the web for up-to-date information. Returns search results with titles, URLs, and snippets. Use for accessing current events, documentation, or recent data."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5)",
                    "default": 5
                },
                "allowed_domains": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Only include results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Exclude results from these domains"
                },
                "proxy_url": {
                    "type": "string",
                    "description": "Optional proxy URL for this request. Overrides agent config and environment variables. Example: http://proxy:8080"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: WebSearchParams = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments(format!("Failed to parse arguments: {e}")))?;

        // Resolve proxy with priority: tool param > agent config > env var
        let effective_proxy = self.proxy_config.resolve(params.proxy_url.as_deref());

        let opts = SearchOptions {
            num_results: params.num_results,
            allowed_domains: params.allowed_domains,
            blocked_domains: params.blocked_domains,
            proxy_url: effective_proxy,
        };

        let result = self
            .provider
            .search(&params.query, opts)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        // Format results as readable text
        let mut content = String::new();
        content.push_str(&format!("Search results for: {}\n\n", result.query));
        for (i, item) in result.results.iter().enumerate() {
            content.push_str(&format!(
                "[{}] {}\n{}\n{}\n\n",
                i + 1,
                item.title,
                item.url,
                item.snippet.as_deref().unwrap_or("")
            ));
        }

        Ok(ToolResult::success(content))
    }
}

// ==================== WebFetchTool ====================

/// Parameters for web_fetch tool
#[derive(Debug, Deserialize)]
pub struct WebFetchParams {
    pub url: String,
    pub prompt: Option<String>,
    /// Optional proxy URL for this specific request.
    /// Overrides agent config and environment variables.
    pub proxy_url: Option<String>,
}

/// Web fetch tool — extracts content from a URL via a FetchFn provider
pub struct WebFetchTool {
    provider: Arc<dyn FetchFn>,
    proxy_config: ProxyConfig,
}

impl WebFetchTool {
    pub fn new(provider: impl FetchFn + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
            proxy_config: ProxyConfig::default(),
        }
    }

    /// Create with a specific proxy configuration (from agent config).
    pub fn with_proxy(provider: impl FetchFn + 'static, proxy_config: ProxyConfig) -> Self {
        Self {
            provider: Arc::new(provider),
            proxy_config,
        }
    }
}

#[async_trait]
impl ExecutableTool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }

    fn description(&self) -> &'static str {
        "Fetch and extract content from a URL. Converts HTML to readable text. Use for reading documentation, articles, or web content."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch content from"
                },
                "prompt": {
                    "type": "string",
                    "description": "Optional prompt to filter/extract specific content"
                },
                "proxy_url": {
                    "type": "string",
                    "description": "Optional proxy URL for this request. Overrides agent config and environment variables. Example: http://proxy:8080"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: WebFetchParams = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments(format!("Failed to parse arguments: {e}")))?;

        // Resolve proxy with priority: tool param > agent config > env var
        let effective_proxy = self.proxy_config.resolve(params.proxy_url.as_deref());

        let opts = FetchOptions {
            prompt: params.prompt,
            proxy_url: effective_proxy,
            max_length: None,
        };

        let result = self
            .provider
            .fetch(&params.url, opts)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let mut content = String::new();

        // Status tags (order: cache first, then status)
        if result.from_cache {
            content.push_str("<fetch from cache>\n");
        }
        match &result.status {
            FetchStatus::Success => {
                content.push_str("<fetch success>\n");
            }
            FetchStatus::SuccessTruncated {
                original_bytes,
                truncated_bytes,
            } => {
                content.push_str("<fetch success truncated>\n");
                content.push_str(&format!(
                    "Note: Content was truncated from {original_bytes} bytes to {truncated_bytes} bytes before extraction.\n"
                ));
            }
            FetchStatus::Redirect { target_url } => {
                content.push_str("<fetch redirect>\n");
                content.push_str(&format!("URL: {}\n", result.url));
                content.push_str(&format!("Redirect to: {target_url}\n\n"));
                content.push_str(&result.content);
                return Ok(ToolResult::success(content));
            }
            FetchStatus::Error { message: _ } => {
                content.push_str("<fetch error>\n");
                content.push_str(&format!("URL: {}\n", result.url));
                content.push_str(&format!("Error: {}\n", result.content));
                return Ok(ToolResult::success(content));
            }
        }

        // Normal output: title + URL + body
        if let Some(title) = &result.title {
            content.push_str(&format!("Title: {title}\n\n"));
        }
        content.push_str(&format!("URL: {}\n\n", result.url));
        content.push_str(&result.content);

        Ok(ToolResult::success(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_params_deserialize() {
        let p: WebSearchParams =
            serde_json::from_value(serde_json::json!({"query": "rust"})).unwrap();
        assert_eq!(p.query, "rust");
        assert!(p.num_results.is_none());
        assert!(p.proxy_url.is_none());
    }

    #[test]
    fn test_search_params_full() {
        let p: WebSearchParams = serde_json::from_value(serde_json::json!({
            "query": "rust", "num_results": 10,
            "allowed_domains": ["docs.rs"], "blocked_domains": ["spam.com"],
            "proxy_url": "http://proxy:8080"
        }))
        .unwrap();
        assert_eq!(p.num_results, Some(10));
        assert_eq!(p.allowed_domains.unwrap(), vec!["docs.rs"]);
        assert_eq!(p.proxy_url.as_deref(), Some("http://proxy:8080"));
    }

    #[test]
    fn test_web_fetch_params() {
        let p: WebFetchParams =
            serde_json::from_value(serde_json::json!({"url": "https://example.com"})).unwrap();
        assert_eq!(p.url, "https://example.com");
        assert!(p.prompt.is_none());
        assert!(p.proxy_url.is_none());
    }

    #[test]
    fn test_web_fetch_params_with_proxy() {
        let p: WebFetchParams = serde_json::from_value(serde_json::json!({
            "url": "https://example.com",
            "proxy_url": "socks5://proxy:1080"
        }))
        .unwrap();
        assert_eq!(p.url, "https://example.com");
        assert_eq!(p.proxy_url.as_deref(), Some("socks5://proxy:1080"));
    }

    #[test]
    fn test_web_search_tool_with_proxy_config() {
        // Verify that with_proxy stores the config
        struct DummySearch;
        #[async_trait]
        impl SearchFn for DummySearch {
            async fn search(
                &self,
                _query: &str,
                _opts: SearchOptions,
            ) -> Result<
                vol_llm_tool::web::search::SearchResult,
                vol_llm_tool::web::search::SearchError,
            > {
                Ok(vol_llm_tool::web::search::SearchResult {
                    query: "test".into(),
                    results: vec![],
                })
            }
        }

        let tool = WebSearchTool::with_proxy(
            DummySearch,
            ProxyConfig {
                proxy_url: Some("http://config.proxy:8080".into()),
            },
        );
        assert!(tool.proxy_config.proxy_url.is_some());
    }
}
