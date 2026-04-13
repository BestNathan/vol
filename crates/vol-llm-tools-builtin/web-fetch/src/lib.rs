//! vol-llm-tools-builtin-web-fetch: Default HTTP→readable-text fetch provider.

use async_trait::async_trait;
use readability::extractor;
use reqwest::Client;
use std::io::Cursor;
use vol_llm_tool::web::fetch::{FetchError, FetchFn, FetchOptions, FetchResult};

const MAX_CONTENT_LENGTH: usize = 2 * 1024 * 1024; // 2MB
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Default fetch provider that fetches URLs and extracts readable content
pub struct DefaultFetchProvider {
    client: Client,
}

impl DefaultFetchProvider {
    /// Create a new fetch provider with optional proxy URL
    pub fn new(
        proxy_url: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent("Mozilla/5.0 (compatible; Agent/1.0)");

        if let Some(url) = proxy_url {
            let proxy = reqwest::Proxy::all(url)?;
            builder = builder.proxy(proxy);
        }

        let client = builder.build()?;
        Ok(Self { client })
    }
}

#[async_trait]
impl FetchFn for DefaultFetchProvider {
    async fn fetch(&self, url: &str, opts: FetchOptions) -> Result<FetchResult, FetchError> {
        // Validate URL
        let parsed =
            url::Url::parse(url).map_err(|e| FetchError::InvalidUrl(e.to_string()))?;

        // Fetch URL
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| FetchError::RequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FetchError::NotAccessible(format!(
                "HTTP {}",
                response.status()
            )));
        }

        // Check content length
        if let Some(len) = response.content_length() {
            if len > MAX_CONTENT_LENGTH as u64 {
                return Err(FetchError::TooLarge {
                    max: MAX_CONTENT_LENGTH,
                    actual: len as usize,
                });
            }
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| FetchError::RequestFailed(e.to_string()))?;

        if bytes.len() > MAX_CONTENT_LENGTH {
            return Err(FetchError::TooLarge {
                max: MAX_CONTENT_LENGTH,
                actual: bytes.len(),
            });
        }

        // Extract readable content using readability extractor
        let product = extractor::extract(&mut Cursor::new(&bytes), &parsed)
            .map_err(|e| FetchError::NotAccessible(format!(
                "Failed to extract content: {}", e
            )))?;

        // Use extracted text (plain text version)
        let content = if product.text.is_empty() {
            // Fallback to HTML content if readability found nothing
            String::from_utf8_lossy(&bytes).to_string()
        } else {
            product.text
        };

        // Truncate if needed
        let max_length = opts.max_length.unwrap_or(MAX_CONTENT_LENGTH / 2);
        let content = if content.len() > max_length {
            format!(
                "{}\n\n[Content truncated at {} characters]",
                &content[..max_length], max_length
            )
        } else {
            content
        };

        let title = if product.title.is_empty() {
            None
        } else {
            Some(product.title)
        };

        Ok(FetchResult {
            url: url.to_string(),
            content,
            title,
        })
    }
}
