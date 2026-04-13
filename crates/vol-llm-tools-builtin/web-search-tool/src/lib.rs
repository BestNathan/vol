//! vol-llm-tools-builtin-web-search: Web search and fetch tools.

pub mod tavily;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::web::fetch::{FetchFn, FetchOptions};
use vol_llm_tool::web::search::{SearchFn, SearchOptions};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};
use std::sync::Arc;

// ==================== WebSearchTool ====================

/// Parameters for web_search tool
#[derive(Debug, Deserialize)]
pub struct WebSearchParams {
    pub query: String,
    pub num_results: Option<usize>,
    pub allowed_domains: Option<Vec<String>>,
    pub blocked_domains: Option<Vec<String>>,
}

/// Web search tool — searches the web via a SearchFn provider
pub struct WebSearchTool {
    provider: Arc<dyn SearchFn>,
}

impl WebSearchTool {
    pub fn new(provider: impl SearchFn + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
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
        let params: WebSearchParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        let opts = SearchOptions {
            num_results: params.num_results,
            allowed_domains: params.allowed_domains,
            blocked_domains: params.blocked_domains,
            proxy_url: None, // Proxy configured at provider creation time
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
}

/// Web fetch tool — extracts content from a URL via a FetchFn provider
pub struct WebFetchTool {
    provider: Arc<dyn FetchFn>,
}

impl WebFetchTool {
    pub fn new(provider: impl FetchFn + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
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
        let params: WebFetchParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        let opts = FetchOptions {
            prompt: params.prompt,
            proxy_url: None, // Proxy configured at provider creation time
            max_length: None,
        };

        let result = self
            .provider
            .fetch(&params.url, opts)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let mut content = String::new();
        if let Some(title) = &result.title {
            content.push_str(&format!("Title: {}\n\n", title));
        }
        content.push_str(&format!("URL: {}\n\n", result.url));
        content.push_str(&result.content);

        Ok(ToolResult::success(content))
    }
}
