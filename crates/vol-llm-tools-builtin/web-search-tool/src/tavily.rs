//! Tavily API search provider.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use vol_llm_tool::web::search::{SearchError, SearchFn, SearchItem, SearchOptions, SearchResult};
use vol_llm_tool::ProxyConfig;

/// Configuration for Tavily search provider.
#[derive(Debug, Clone, Deserialize)]
pub struct TavilyConfig {
    pub api_key: String,
    #[serde(default)]
    pub proxy: ProxyConfig,
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
    client: Client,
}

impl TavilySearchProvider {
    /// Create a new Tavily provider with optional proxy URL
    pub fn new(
        api_key: String,
        proxy_url: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = build_client(&proxy_url)?;
        Ok(Self { api_key, client })
    }

    /// Create a new Tavily provider from configuration.
    pub fn from_config(config: &TavilyConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = build_client(&config.proxy.proxy_url)?;
        Ok(Self {
            api_key: config.api_key.clone(),
            client,
        })
    }
}

fn build_client(
    proxy_url: &Option<String>,
) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    let mut builder = Client::builder();
    if let Some(url) = proxy_url {
        let proxy = reqwest::Proxy::all(url)?;
        builder = builder.proxy(proxy);
    }
    Ok(builder.build()?)
}

#[async_trait]
impl SearchFn for TavilySearchProvider {
    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<SearchResult, SearchError> {
        let num_results = opts.num_results.unwrap_or(5);

        let mut body = serde_json::json!({
            "query": query,
            "api_key": &self.api_key,
            "max_results": num_results,
        });

        if let Some(ref domains) = opts.allowed_domains {
            body["include_domains"] = serde_json::Value::Array(
                domains
                    .iter()
                    .map(|d| serde_json::Value::String(d.clone()))
                    .collect(),
            );
        }
        if let Some(ref domains) = opts.blocked_domains {
            body["exclude_domains"] = serde_json::Value::Array(
                domains
                    .iter()
                    .map(|d| serde_json::Value::String(d.clone()))
                    .collect(),
            );
        }

        let response = self
            .client
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
                "Tavily API error: {} — {}",
                status, text
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
}
