//! Tavily API search provider.

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::web::search::{SearchError, SearchFn, SearchItem, SearchOptions, SearchResult};
use vol_llm_tool::{ProxyConfig, RetryConfig};

/// Configuration for Tavily search provider.
#[derive(Debug, Clone, Deserialize)]
pub struct TavilyConfig {
    pub api_key: String,
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub retry: RetryConfig,
}

#[derive(Debug, Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: Option<String>,
}

/// Tavily API search provider.
pub struct TavilySearchProvider {
    api_key: String,
    retry_config: RetryConfig,
}

impl TavilySearchProvider {
    /// Create a new Tavily provider with optional proxy URL.
    pub fn new(
        api_key: String,
        _proxy_url: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            api_key,
            retry_config: RetryConfig::default(),
        })
    }

    /// Create a new Tavily provider from configuration.
    pub fn from_config(
        config: &TavilyConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            api_key: config.api_key.clone(),
            retry_config: config.retry.clone(),
        })
    }
}

fn build_client(
    proxy_url: &Option<String>,
) -> Result<reqwest::Client, Box<dyn std::error::Error + Send + Sync>> {
    let mut builder = reqwest::Client::builder();
    if let Some(url) = proxy_url {
        let proxy = reqwest::Proxy::all(url)?;
        builder = builder.proxy(proxy);
    }
    Ok(builder.build()?)
}

#[async_trait]
impl SearchFn for TavilySearchProvider {
    async fn search(&self, query: &str, opts: SearchOptions) -> Result<SearchResult, SearchError> {
        let num_results = opts.num_results.unwrap_or(5);
        let api_key = self.api_key.clone();
        let retry_config = self.retry_config.clone();
        let query_owned = query.to_string();

        vol_llm_tool::web::retry::retry_async(&retry_config, |_attempt| {
            let api_key = api_key.clone();
            let query = query_owned.clone();
            let allowed_domains = opts.allowed_domains.clone();
            let blocked_domains = opts.blocked_domains.clone();
            let proxy_url = opts.proxy_url.clone();
            async move {
                // Build request body
                let mut body = serde_json::json!({
                    "query": &query,
                    "api_key": &api_key,
                    "max_results": num_results,
                });

                if let Some(ref domains) = allowed_domains {
                    body["include_domains"] = serde_json::Value::Array(
                        domains
                            .iter()
                            .map(|d| serde_json::Value::String(d.clone()))
                            .collect(),
                    );
                }
                if let Some(ref domains) = blocked_domains {
                    body["exclude_domains"] = serde_json::Value::Array(
                        domains
                            .iter()
                            .map(|d| serde_json::Value::String(d.clone()))
                            .collect(),
                    );
                }

                // Determine effective proxy for this attempt.
                // opts.proxy_url has been resolved by the tool layer already.
                let effective_proxy = proxy_url;

                let client = build_client(&effective_proxy)
                    .map_err(|e| SearchError::RequestFailed(e.to_string()))?;

                let response = client
                    .post("https://api.tavily.com/search")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| SearchError::RequestFailed(e.to_string()))?;

                if !response.status().is_success() {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    if status == reqwest::StatusCode::UNAUTHORIZED
                        || status == reqwest::StatusCode::FORBIDDEN
                    {
                        return Err(SearchError::InvalidApiKey);
                    }
                    return Err(SearchError::RequestFailed(format!(
                        "Tavily API error: {status} — {text}"
                    )));
                }

                let resp: TavilyResponse = response
                    .json()
                    .await
                    .map_err(|e| SearchError::ParseError(e.to_string()))?;

                let results = resp
                    .results
                    .into_iter()
                    .map(|r| SearchItem {
                        title: r.title,
                        url: r.url,
                        snippet: r.content,
                    })
                    .collect();

                Ok(SearchResult {
                    query: query.to_string(),
                    results,
                })
            }
        })
        .await
    }
}
