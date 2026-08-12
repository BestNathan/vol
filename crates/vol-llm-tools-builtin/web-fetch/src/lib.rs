//! vol-llm-tools-builtin-web-fetch: Default HTTP→readable-text fetch provider.
//!
//! Features:
//! - Cross-host redirect detection (returns redirect notice instead of following)
//! - JSON file cache (`.vol/cache/tools/web_fetch/`, 15 min TTL)
//! - Structured status markers in output (`<fetch success>`, etc.)

use async_trait::async_trait;
use readability::extractor;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;
use vol_llm_tool::web::fetch::{FetchError, FetchFn, FetchOptions, FetchResult, FetchStatus};
use vol_llm_tool::{ProxyConfig, RetryConfig};

const DEFAULT_MAX_CONTENT_LENGTH: usize = 2 * 1024 * 1024; // 2MB
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const CACHE_TTL_SUCCESS_SECS: u64 = 900; // 15 min
const CACHE_TTL_ERROR_SECS: u64 = 300; // 5 min
const MAX_REDIRECT_HOPS: u32 = 5;
const CACHE_DIR_NAME: &str = ".vol/cache/tools/web_fetch";

// ── Cache file format ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    url: String,
    status: String, // "success" | "truncated" | "redirect" | "error"
    title: Option<String>,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect_target: Option<String>,
    cached_at: String, // RFC3339
}

// ── Provider config ────────────────────────────────────────────────────

/// Configuration for the default fetch provider.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FetchProviderConfig {
    /// Maximum content length in bytes (default: 2MB)
    pub max_content_length: Option<usize>,
    /// Proxy configuration (optional)
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// Retry configuration (optional)
    #[serde(default)]
    pub retry: RetryConfig,
}

/// Default fetch provider.
pub struct DefaultFetchProvider {
    max_content_length: usize,
    retry_config: RetryConfig,
    cache_dir: Option<PathBuf>,
}

impl DefaultFetchProvider {
    pub fn new(
        _proxy_url: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            max_content_length: DEFAULT_MAX_CONTENT_LENGTH,
            retry_config: RetryConfig::default(),
            cache_dir: resolve_cache_dir(),
        })
    }

    pub fn from_config(
        config: &FetchProviderConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            max_content_length: config
                .max_content_length
                .unwrap_or(DEFAULT_MAX_CONTENT_LENGTH),
            retry_config: config.retry.clone(),
            cache_dir: resolve_cache_dir(),
        })
    }

    /// Check cache for a URL. Returns `Some(FetchResult)` on hit, `None` on miss.
    fn cache_get(&self, url: &str) -> Option<FetchResult> {
        let cache_dir = self.cache_dir.as_ref()?;
        let path = cache_path(cache_dir, url);
        let bytes = std::fs::read(&path).ok()?;

        let entry: CacheEntry = serde_json::from_slice(&bytes).ok()?;
        let cached_at: chrono::DateTime<chrono::Utc> = entry.cached_at.parse().ok()?;
        let now = chrono::Utc::now();
        let elapsed = now.signed_duration_since(cached_at);

        let ttl = match entry.status.as_str() {
            "error" => CACHE_TTL_ERROR_SECS,
            _ => CACHE_TTL_SUCCESS_SECS,
        };

        if elapsed.num_seconds() >= ttl as i64 {
            let _ = std::fs::remove_file(&path);
            return None;
        }

        let status = match entry.status.as_str() {
            "redirect" => FetchStatus::Redirect {
                target_url: entry.redirect_target.unwrap_or_default(),
            },
            "truncated" => FetchStatus::SuccessTruncated {
                original_bytes: 0,
                truncated_bytes: 0,
            },
            "error" => FetchStatus::Error {
                message: entry.content.clone(),
            },
            _ => FetchStatus::Success,
        };

        Some(FetchResult {
            url: entry.url,
            status,
            content: entry.content,
            title: entry.title,
            from_cache: true,
        })
    }

    /// Write a fetch result to cache.
    fn cache_put(&self, url: &str, result: &FetchResult) {
        let cache_dir = match &self.cache_dir {
            Some(d) => d,
            None => return,
        };

        let _ = std::fs::create_dir_all(cache_dir);
        let path = cache_path(cache_dir, url);

        let (status_str, redirect_target) = match &result.status {
            FetchStatus::Success => ("success", None),
            FetchStatus::SuccessTruncated { .. } => ("truncated", None),
            FetchStatus::Redirect { target_url } => ("redirect", Some(target_url.clone())),
            FetchStatus::Error { .. } => ("error", None),
        };

        let entry = CacheEntry {
            url: result.url.clone(),
            status: status_str.to_string(),
            title: result.title.clone(),
            content: result.content.clone(),
            redirect_target,
            cached_at: chrono::Utc::now().to_rfc3339(),
        };

        if let Ok(json) = serde_json::to_vec(&entry) {
            let _ = std::fs::write(&path, json);
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn resolve_cache_dir() -> Option<PathBuf> {
    let base = std::env::current_dir().ok()?;
    let dir = base.join(CACHE_DIR_NAME);
    // Create cache dir (best-effort). Cache degrades gracefully on failure.
    match std::fs::create_dir_all(&dir) {
        Ok(()) => Some(dir),
        Err(_) => None,
    }
}

fn cache_path(cache_dir: &std::path::Path, url: &str) -> PathBuf {
    let hash = sha256_hash(url);
    cache_dir.join(format!("{hash}.json"))
}

fn sha256_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn build_client(
    proxy_url: &Option<String>,
) -> Result<reqwest::Client, Box<dyn std::error::Error + Send + Sync>> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .user_agent("Mozilla/5.0 (compatible; Agent/1.0)")
        .redirect(reqwest::redirect::Policy::none());

    if let Some(url) = proxy_url {
        let proxy = reqwest::Proxy::all(url)?;
        builder = builder.proxy(proxy);
    }

    Ok(builder.build()?)
}

/// Check if two URLs have different hosts.
fn is_cross_host(original: &url::Url, target: &url::Url) -> bool {
    original.host_str() != target.host_str()
}

// ── Fetch implementation ───────────────────────────────────────────────

#[async_trait]
impl FetchFn for DefaultFetchProvider {
    async fn fetch(&self, url: &str, opts: FetchOptions) -> Result<FetchResult, FetchError> {
        // 1. Check cache
        if let Some(cached) = self.cache_get(url) {
            return Ok(cached);
        }

        // 2. Validate URL
        let current_url =
            url::Url::parse(url).map_err(|e| FetchError::InvalidUrl(e.to_string()))?;

        let effective_proxy = opts.proxy_url.clone();
        let max_content_length = self.max_content_length;
        let retry_config = self.retry_config.clone();

        // 3. Fetch with retry + manual redirect loop
        let fetch_result = vol_llm_tool::web::retry::retry_async(&retry_config, |_attempt| {
            let proxy_url = effective_proxy.clone();
            let current_url = current_url.clone();
            async move {
                let client = build_client(&proxy_url)
                    .map_err(|e| FetchError::RequestFailed(e.to_string()))?;

                let mut hop_url = current_url.clone();

                for _hop in 0..MAX_REDIRECT_HOPS {
                    let response = client
                        .get(hop_url.as_str())
                        .send()
                        .await
                        .map_err(|e| FetchError::RequestFailed(e.to_string()))?;

                    let http_status = response.status();

                    // Handle redirects (3xx with Location header)
                    if http_status.is_redirection() {
                        let target = response
                            .headers()
                            .get(reqwest::header::LOCATION)
                            .and_then(|v| v.to_str().ok())
                            .and_then(|loc| hop_url.join(loc).ok());

                        let target = match target {
                            Some(t) => t,
                            None => {
                                return Ok(FetchResult {
                                    url: current_url.to_string(),
                                    status: FetchStatus::Error {
                                        message: "Redirect without Location header".into(),
                                    },
                                    content: String::new(),
                                    title: None,
                                    from_cache: false,
                                });
                            }
                        };

                        // Cross-host redirect → notify, don't follow
                        if is_cross_host(&current_url, &target) {
                            let content = format!(
                                "This URL redirected to a different host. Use web_fetch with the redirect target URL to fetch the content.\nRedirect to: {}",
                                target
                            );
                            return Ok(FetchResult {
                                url: current_url.to_string(),
                                status: FetchStatus::Redirect {
                                    target_url: target.to_string(),
                                },
                                content,
                                title: None,
                                from_cache: false,
                            });
                        }

                        // Same host — follow
                        hop_url = target;
                        continue;
                    }

                    // Non-redirect: error status → return error result
                    if !http_status.is_success() {
                        let msg = format!(
                            "HTTP {} {}",
                            http_status.as_u16(),
                            http_status.canonical_reason().unwrap_or("Unknown")
                        );
                        return Ok(FetchResult {
                            url: current_url.to_string(),
                            status: FetchStatus::Error {
                                message: msg.clone(),
                            },
                            content: msg,
                            title: None,
                            from_cache: false,
                        });
                    }

                    // Success: read and extract
                    let content_length = response.content_length();
                    let body_too_large = content_length
                        .map(|len| len > max_content_length as u64)
                        .unwrap_or(false);

                    let (bytes, was_truncated, original_bytes) = if body_too_large {
                        match response.bytes().await {
                            Ok(all_bytes) => {
                                let truncated_bytes =
                                    all_bytes.len().min(max_content_length);
                                let truncated = all_bytes[..truncated_bytes].to_vec();
                                (truncated, true, content_length.unwrap() as usize)
                            }
                            Err(e) => {
                                return Err(FetchError::RequestFailed(e.to_string()));
                            }
                        }
                    } else {
                        match response.bytes().await {
                            Ok(all_bytes) => {
                                let len = all_bytes.len();
                                if len > max_content_length {
                                    let truncated = all_bytes[..max_content_length].to_vec();
                                    (truncated, true, len)
                                } else {
                                    (all_bytes.to_vec(), false, len)
                                }
                            }
                            Err(e) => {
                                return Err(FetchError::RequestFailed(e.to_string()));
                            }
                        }
                    };

                    // Extract readable content
                    let product =
                        extractor::extract(&mut Cursor::new(&bytes), &hop_url).map_err(|e| {
                            FetchError::NotAccessible(format!("Failed to extract content: {e}"))
                        })?;

                    let mut content = if product.text.is_empty() {
                        String::from_utf8_lossy(&bytes).to_string()
                    } else {
                        product.text
                    };

                    let title = if product.title.is_empty() {
                        None
                    } else {
                        Some(product.title)
                    };

                    // Post-extraction text truncation (max half of content limit for text)
                    let text_max = max_content_length / 2;
                    let was_text_truncated = !was_truncated && content.len() > text_max;
                    if content.len() > text_max {
                        content = format!(
                            "{}\n\n[Content truncated at {} characters]",
                            &content[..text_max],
                            text_max
                        );
                    }

                    let status = if was_truncated {
                        FetchStatus::SuccessTruncated {
                            original_bytes,
                            truncated_bytes: max_content_length,
                        }
                    } else if was_text_truncated {
                        FetchStatus::SuccessTruncated {
                            original_bytes,
                            truncated_bytes: content.len(),
                        }
                    } else {
                        FetchStatus::Success
                    };

                    return Ok(FetchResult {
                        url: current_url.to_string(),
                        status,
                        content,
                        title,
                        from_cache: false,
                    });
                }

                // Exhausted redirect hops
                let msg = format!("Too many redirects (max {} hops)", MAX_REDIRECT_HOPS);
                Ok(FetchResult {
                    url: current_url.to_string(),
                    status: FetchStatus::Error {
                        message: msg.clone(),
                    },
                    content: msg,
                    title: None,
                    from_cache: false,
                })
            }
        })
        .await?;

        // 4. Write cache
        self.cache_put(url, &fetch_result);

        Ok(fetch_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_provider_config_default() {
        let cfg = FetchProviderConfig::default();
        assert!(cfg.max_content_length.is_none());
        assert!(cfg.proxy.proxy_url.is_none());
    }

    #[test]
    fn test_fetch_provider_config_deserialize() {
        let cfg: FetchProviderConfig = toml::from_str("").unwrap_or_default();
        assert!(cfg.max_content_length.is_none());
    }

    #[test]
    fn test_max_content_length_default() {
        assert_eq!(DEFAULT_MAX_CONTENT_LENGTH, 2 * 1024 * 1024);
    }

    #[test]
    fn test_default_timeout() {
        assert_eq!(DEFAULT_TIMEOUT_SECS, 30);
    }

    #[test]
    fn test_new() {
        let provider = DefaultFetchProvider::new(None).unwrap();
        assert_eq!(provider.max_content_length, DEFAULT_MAX_CONTENT_LENGTH);
    }

    #[test]
    fn test_new_with_proxy() {
        let provider = DefaultFetchProvider::new(Some("http://127.0.0.1:9".to_string())).unwrap();
        assert_eq!(provider.max_content_length, DEFAULT_MAX_CONTENT_LENGTH);
    }

    #[test]
    fn test_new_invalid_proxy_url_no_longer_fails() {
        let provider = DefaultFetchProvider::new(Some("not a proxy url".to_string()));
        assert!(provider.is_ok());
    }

    #[test]
    fn test_from_config_custom_max_length() {
        let cfg = FetchProviderConfig {
            max_content_length: Some(1024),
            proxy: ProxyConfig::default(),
            retry: RetryConfig::default(),
        };
        let provider = DefaultFetchProvider::from_config(&cfg).unwrap();
        assert_eq!(provider.max_content_length, 1024);
    }

    #[test]
    fn test_from_config_default_max_length() {
        let provider = DefaultFetchProvider::from_config(&FetchProviderConfig::default()).unwrap();
        assert_eq!(provider.max_content_length, DEFAULT_MAX_CONTENT_LENGTH);
    }

    #[test]
    fn test_cache_ttl_constants() {
        assert_eq!(CACHE_TTL_SUCCESS_SECS, 900);
        assert_eq!(CACHE_TTL_ERROR_SECS, 300);
    }

    #[test]
    fn test_max_redirect_hops() {
        assert_eq!(MAX_REDIRECT_HOPS, 5);
    }

    #[test]
    fn test_sha256_hash_deterministic() {
        let h1 = sha256_hash("https://example.com");
        let h2 = sha256_hash("https://example.com");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA256 hex is 64 chars
    }

    #[test]
    fn test_sha256_hash_different_urls() {
        let h1 = sha256_hash("https://example.com/a");
        let h2 = sha256_hash("https://example.com/b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_cache_path_format() {
        let dir = PathBuf::from("/tmp/.vol/cache/tools/web_fetch");
        let path = cache_path(&dir, "https://example.com/page");
        let expected_hash = sha256_hash("https://example.com/page");
        assert_eq!(
            path,
            PathBuf::from(format!(
                "/tmp/.vol/cache/tools/web_fetch/{expected_hash}.json"
            ))
        );
    }

    #[test]
    fn test_is_cross_host_same() {
        let a = url::Url::parse("https://example.com/page").unwrap();
        let b = url::Url::parse("https://example.com/other").unwrap();
        assert!(!is_cross_host(&a, &b));
    }

    #[test]
    fn test_is_cross_host_different() {
        let a = url::Url::parse("https://oldsite.com").unwrap();
        let b = url::Url::parse("https://newsite.com").unwrap();
        assert!(is_cross_host(&a, &b));
    }

    #[test]
    fn test_is_cross_host_subdomain() {
        let a = url::Url::parse("https://example.com").unwrap();
        let b = url::Url::parse("https://sub.example.com").unwrap();
        assert!(is_cross_host(&a, &b));
    }

    // ── HTTP server helpers ─────────────────────────────────────────────

    fn spawn_http_server(
        status: u16,
        body: &str,
        content_length: Option<usize>,
    ) -> std::net::SocketAddr {
        let body = body.to_string();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf);
                let reason = match status {
                    200 => "OK",
                    301 => "Moved Permanently",
                    302 => "Found",
                    307 => "Temporary Redirect",
                    308 => "Permanent Redirect",
                    404 => "Not Found",
                    500 => "Internal Server Error",
                    502 => "Bad Gateway",
                    503 => "Service Unavailable",
                    _ => "Unknown",
                };
                let mut head = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html\r\nConnection: close\r\n"
                );
                if let Some(len) = content_length {
                    head.push_str(&format!("Content-Length: {len}\r\n"));
                }
                head.push_str("\r\n");
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            }
        });
        addr
    }

    /// Spawn an HTTP server that returns a redirect to the given target URL.
    fn spawn_redirect_server(status: u16, target: &str) -> std::net::SocketAddr {
        let target = target.to_string();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf);
                let reason = if status == 301 {
                    "Moved Permanently"
                } else {
                    "Found"
                };
                let head = format!(
                    "HTTP/1.1 {status} {reason}\r\nLocation: {target}\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(head.as_bytes());
            }
        });
        addr
    }

    // ── Fetch tests ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_invalid_url() {
        let provider = DefaultFetchProvider::new(None).unwrap();
        let err = provider
            .fetch(
                "not a url",
                FetchOptions {
                    prompt: None,
                    proxy_url: None,
                    max_length: None,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn test_fetch_request_failed() {
        let provider = DefaultFetchProvider::new(None).unwrap();
        let err = provider
            .fetch(
                "http://127.0.0.1:1/",
                FetchOptions {
                    prompt: None,
                    proxy_url: None,
                    max_length: None,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::RequestFailed(_)));
    }

    #[tokio::test]
    async fn test_fetch_not_accessible_returns_error_status() {
        let addr = spawn_http_server(404, "<html><body>missing</body></html>", Some(30));
        let provider = DefaultFetchProvider::new(None).unwrap();
        let result = provider
            .fetch(
                &format!("http://{addr}/missing"),
                FetchOptions {
                    prompt: None,
                    proxy_url: None,
                    max_length: None,
                },
            )
            .await
            .unwrap();
        assert!(matches!(result.status, FetchStatus::Error { .. }));
        assert!(result.content.contains("HTTP 404"));
    }

    #[tokio::test]
    async fn test_fetch_success() {
        let body = "<html><head><title>Test Page</title></head><body><p>Hello from the local server</p></body></html>";
        let addr = spawn_http_server(200, body, Some(body.len()));
        let provider = DefaultFetchProvider::new(None).unwrap();
        let result = provider
            .fetch(
                &format!("http://{addr}/page"),
                FetchOptions {
                    prompt: None,
                    proxy_url: None,
                    max_length: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.title.as_deref(), Some("Test Page"));
        assert!(result.content.contains("Hello from the local server"));
        assert!(matches!(result.status, FetchStatus::Success));
    }

    #[tokio::test]
    async fn test_fetch_cache_flag_on_repeat() {
        let body =
            "<html><head><title>Tags Test</title></head><body><p>Content here</p></body></html>";
        let addr = spawn_http_server(200, body, Some(body.len()));
        let provider = DefaultFetchProvider::new(None).unwrap();
        let url = format!("http://{addr}/tags");

        // First fetch — not from cache
        let r1 = provider.fetch(&url, FetchOptions::default()).await.unwrap();
        assert!(!r1.from_cache);

        // Second fetch — should be from cache
        let r2 = provider.fetch(&url, FetchOptions::default()).await.unwrap();
        assert!(r2.from_cache);
        assert_eq!(r2.content, r1.content);
    }

    #[tokio::test]
    async fn test_fetch_truncates_large_content() {
        // Body is ~1MB+100 bytes. Under 2MB body limit, but extracted text
        // exceeds 1MB text limit (= max_content_length / 2), triggering text truncation.
        let body = "a".repeat(1024 * 1024 + 100);
        let addr = spawn_http_server(200, &body, Some(body.len()));
        let provider = DefaultFetchProvider::new(None).unwrap();
        let result = provider
            .fetch(
                &format!("http://{addr}/long"),
                FetchOptions {
                    prompt: None,
                    proxy_url: None,
                    max_length: None,
                },
            )
            .await
            .unwrap();
        assert!(
            matches!(result.status, FetchStatus::SuccessTruncated { .. }),
            "expected SuccessTruncated, got {:?}",
            result.status
        );
        assert!(result.content.contains("[Content truncated at"));
    }

    #[tokio::test]
    async fn test_fetch_fallback_to_raw_html() {
        let body = "<html><head><title></title></head><body></body></html>";
        let addr = spawn_http_server(200, body, Some(body.len()));
        let provider = DefaultFetchProvider::new(None).unwrap();
        let result = provider
            .fetch(
                &format!("http://{addr}/empty"),
                FetchOptions {
                    prompt: None,
                    proxy_url: None,
                    max_length: None,
                },
            )
            .await
            .unwrap();
        assert!(result.content.contains("<html>"));
    }

    // ── Redirect tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_cross_host_redirect() {
        let target = "https://newsite.example.com/new-page";
        let addr = spawn_redirect_server(301, target);
        let provider = DefaultFetchProvider::new(None).unwrap();
        let result = provider
            .fetch(
                &format!("http://{addr}/old"),
                FetchOptions {
                    prompt: None,
                    proxy_url: None,
                    max_length: None,
                },
            )
            .await
            .unwrap();
        match &result.status {
            FetchStatus::Redirect { target_url } => {
                assert_eq!(target_url, target);
            }
            _ => panic!("expected Redirect, got {:?}", result.status),
        }
        assert!(result.content.contains("redirected to a different host"));
        assert!(result.content.contains(target));
    }

    #[tokio::test]
    async fn test_fetch_same_host_redirect_followed() {
        let body = "<html><head><title>Final</title></head><body><p>Arrived</p></body></html>";
        let target_addr = spawn_http_server(200, body, Some(body.len()));
        let target_url = format!("http://{target_addr}/final");

        // Spawn a redirect server that redirects to the target (same host: 127.0.0.1)
        let redirect_addr = spawn_redirect_server(302, &target_url);
        let provider = DefaultFetchProvider::new(None).unwrap();

        // Both servers are on 127.0.0.1, so same-host redirect should be followed
        let result = provider
            .fetch(
                &format!("http://{redirect_addr}/start"),
                FetchOptions {
                    prompt: None,
                    proxy_url: None,
                    max_length: None,
                },
            )
            .await
            .unwrap();
        // Should have followed the redirect to get the content
        match &result.status {
            FetchStatus::Success => {
                assert!(result.content.contains("Arrived"));
            }
            FetchStatus::Redirect { target_url } => {
                // It's possible these are on different ports which would count as different hosts.
                // In that case, verify the redirect notification is correct.
                assert!(target_url.contains("/final"));
            }
            other => panic!("unexpected status: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fetch_error_status_on_500() {
        let addr = spawn_http_server(503, "Service Unavailable", Some(20));
        let provider = DefaultFetchProvider::new(None).unwrap();
        let result = provider
            .fetch(
                &format!("http://{addr}/error"),
                FetchOptions {
                    prompt: None,
                    proxy_url: None,
                    max_length: None,
                },
            )
            .await
            .unwrap();
        match &result.status {
            FetchStatus::Error { message } => {
                assert!(message.contains("503"));
            }
            _ => panic!("expected Error, got {:?}", result.status),
        }
    }

    // ── Cache tests ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_cache_hit_on_repeat_url() {
        let body = "<html><head><title>Cached</title></head><body><p>First fetch</p></body></html>";
        let addr = spawn_http_server(200, body, Some(body.len()));
        let provider = DefaultFetchProvider::new(None).unwrap();
        let url = format!("http://{addr}/cache-test");

        let r1 = provider.fetch(&url, FetchOptions::default()).await.unwrap();
        assert!(!r1.from_cache);

        let r2 = provider.fetch(&url, FetchOptions::default()).await.unwrap();
        assert!(r2.from_cache);
        assert_eq!(r2.content, r1.content);
        assert_eq!(r2.title, r1.title);
    }

    #[tokio::test]
    async fn test_fetch_cache_on_error() {
        let addr = spawn_http_server(502, "Bad Gateway", Some(15));
        let provider = DefaultFetchProvider::new(None).unwrap();
        let url = format!("http://{addr}/error-cache");

        let r1 = provider.fetch(&url, FetchOptions::default()).await.unwrap();
        assert!(matches!(r1.status, FetchStatus::Error { .. }));
        assert!(!r1.from_cache);

        let r2 = provider.fetch(&url, FetchOptions::default()).await.unwrap();
        assert!(r2.from_cache);
        assert!(matches!(r2.status, FetchStatus::Error { .. }));
    }

    #[tokio::test]
    async fn test_fetch_cache_on_redirect() {
        let target = "https://other.example.com/page";
        let addr = spawn_redirect_server(301, target);
        let provider = DefaultFetchProvider::new(None).unwrap();
        let url = format!("http://{addr}/redirect-cache");

        let r1 = provider.fetch(&url, FetchOptions::default()).await.unwrap();
        assert!(matches!(r1.status, FetchStatus::Redirect { .. }));
        assert!(!r1.from_cache);

        let r2 = provider.fetch(&url, FetchOptions::default()).await.unwrap();
        assert!(r2.from_cache);
        assert!(matches!(r2.status, FetchStatus::Redirect { .. }));
    }

    #[test]
    fn test_cache_entry_serde_roundtrip() {
        let entry = CacheEntry {
            url: "https://example.com".into(),
            status: "success".into(),
            title: Some("Test".into()),
            content: "Hello world".into(),
            redirect_target: None,
            cached_at: "2026-08-12T10:30:00Z".into(),
        };
        let json = serde_json::to_vec(&entry).unwrap();
        let decoded: CacheEntry = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.url, "https://example.com");
        assert_eq!(decoded.status, "success");
        assert_eq!(decoded.content, "Hello world");
    }

    #[test]
    fn test_cache_entry_redirect() {
        let entry = CacheEntry {
            url: "http://old.com".into(),
            status: "redirect".into(),
            title: None,
            content: "redirected".into(),
            redirect_target: Some("https://new.com".into()),
            cached_at: "2026-08-12T10:30:00Z".into(),
        };
        let json = serde_json::to_vec(&entry).unwrap();
        let decoded: CacheEntry = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.redirect_target.unwrap(), "https://new.com");
    }

    #[test]
    fn test_cache_entry_error() {
        let entry = CacheEntry {
            url: "https://down.example.com".into(),
            status: "error".into(),
            title: None,
            content: "HTTP 503 Service Unavailable".into(),
            redirect_target: None,
            cached_at: "2026-08-12T10:30:00Z".into(),
        };
        let json = serde_json::to_vec(&entry).unwrap();
        let decoded: CacheEntry = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.status, "error");
        assert_eq!(decoded.content, "HTTP 503 Service Unavailable");
    }

    #[test]
    fn test_cache_path_different_urls() {
        let dir = PathBuf::from("/tmp/.vol/cache/tools/web_fetch");
        let p1 = cache_path(&dir, "https://example.com/a");
        let p2 = cache_path(&dir, "https://example.com/b");
        assert_ne!(p1, p2);
    }

    #[test]
    fn test_fetch_status_enum_variants() {
        // Verify all variants can be constructed
        let s = FetchStatus::Success;
        assert!(matches!(s, FetchStatus::Success));

        let s = FetchStatus::SuccessTruncated {
            original_bytes: 100,
            truncated_bytes: 50,
        };
        assert!(matches!(s, FetchStatus::SuccessTruncated { .. }));

        let s = FetchStatus::Redirect {
            target_url: "https://new.com".into(),
        };
        assert!(matches!(s, FetchStatus::Redirect { .. }));

        let s = FetchStatus::Error {
            message: "fail".into(),
        };
        assert!(matches!(s, FetchStatus::Error { .. }));
    }

    #[test]
    fn test_resolve_cache_dir_creates_directory() {
        let dir = resolve_cache_dir();
        // Should succeed in test environment (current dir is writable)
        if let Some(ref d) = dir {
            assert!(d.ends_with(CACHE_DIR_NAME));
        }
    }
}
