//! Retry configuration and logic for web tools.
//!
//! Provides a `RetryConfig` that can be embedded in tool config structs,
//! and a `retry_async` helper for exponential backoff retry loops.

use serde::Deserialize;
use std::future::Future;
use std::time::Duration;

/// Default retry settings.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;
pub const DEFAULT_BASE_DELAY_MS: u64 = 1000; // 1 second

/// Retry configuration for web requests.
///
/// Can be embedded in any tool config struct to provide retry support.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the initial request).
    /// Default: 3
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,

    /// Base delay between retries in milliseconds.
    /// Each subsequent retry doubles the delay (exponential backoff).
    /// Default: 1000ms
    #[serde(default = "default_base_delay_ms")]
    pub base_delay_ms: u64,
}

fn default_max_attempts() -> u32 {
    DEFAULT_MAX_ATTEMPTS
}

fn default_base_delay_ms() -> u64 {
    DEFAULT_BASE_DELAY_MS
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            base_delay_ms: DEFAULT_BASE_DELAY_MS,
        }
    }
}

impl RetryConfig {
    /// Create a retry config with custom settings.
    pub fn new(max_attempts: u32, base_delay_ms: u64) -> Self {
        Self {
            max_attempts,
            base_delay_ms,
        }
    }

    /// Returns the delay for a given attempt (0-indexed).
    /// Attempt 0 = no delay (first request), attempt 1 = base_delay, attempt 2 = 2×base_delay, etc.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            Duration::ZERO
        } else {
            Duration::from_millis(self.base_delay_ms * 2u64.pow(attempt - 1))
        }
    }

    /// Returns `true` if the error should trigger a retry.
    ///
    /// Retries are appropriate for transient network errors and server-side
    /// errors (5xx). Client errors (4xx) are not retried.
    pub fn should_retry(error_msg: &str) -> bool {
        // Network-level errors from reqwest typically contain these patterns
        let lower = error_msg.to_lowercase();
        lower.contains("timeout")
            || lower.contains("connection")
            || lower.contains("dns")
            || lower.contains("reset")
            || lower.contains("refused")
            || lower.contains("tls")
            || lower.contains("eof")
            || lower.contains("broken pipe")
    }
}

/// Execute an async operation with retry logic.
///
/// The operation receives the current attempt number (0-indexed).
/// Returns the first successful result, or the last error if all attempts fail.
///
/// # Example
///
/// ```ignore
/// let result = retry_async(&RetryConfig::default(), |attempt| async {
///     if attempt > 0 {
///         // retry attempt
///     }
///     reqwest::get("https://example.com").await
/// }).await;
/// ```
pub async fn retry_async<F, Fut, T, E>(config: &RetryConfig, mut operation: F) -> Result<T, E>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        if attempt > 0 {
            let delay = config.delay_for_attempt(attempt);
            tracing::info!(
                attempt = attempt,
                delay_ms = delay.as_millis(),
                "Retrying after failure"
            );
            tokio::time::sleep(delay).await;
        }

        match operation(attempt).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let err_str = e.to_string();
                let can_retry =
                    attempt + 1 < config.max_attempts && RetryConfig::should_retry(&err_str);
                if can_retry {
                    tracing::warn!(
                        attempt = attempt,
                        error = %err_str,
                        "Request failed, will retry"
                    );
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    // All attempts exhausted with retryable errors.
    // `last_error` is always `Some` here because max_attempts >= 1
    // and we only reach this point if every iteration stored its error
    // in the `can_retry` branch (so `last_error = Some(e)` was executed).
    if let Some(err) = last_error {
        Err(err)
    } else {
        // Defensive fallback: run the operation one more time and return
        // whatever error it produces. This branch is unreachable in practice.
        Err(operation(0).await.err().unwrap_or_else(|| {
            panic!("retry_async: operation succeeded when it should have failed")
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── RetryConfig tests ───────────────────────────────────────────────

    #[test]
    fn test_default_config() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_attempts, 3);
        assert_eq!(cfg.base_delay_ms, 1000);
    }

    #[test]
    fn test_custom_config() {
        let cfg = RetryConfig::new(5, 500);
        assert_eq!(cfg.max_attempts, 5);
        assert_eq!(cfg.base_delay_ms, 500);
    }

    #[test]
    fn test_delay_for_attempt_zero() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.delay_for_attempt(0), Duration::ZERO);
    }

    #[test]
    fn test_delay_for_attempt_one() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.delay_for_attempt(1), Duration::from_millis(1000));
    }

    #[test]
    fn test_delay_for_attempt_two() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.delay_for_attempt(2), Duration::from_millis(2000));
    }

    #[test]
    fn test_delay_for_attempt_three() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.delay_for_attempt(3), Duration::from_millis(4000));
    }

    #[test]
    fn test_should_retry_timeout() {
        assert!(RetryConfig::should_retry("request timeout"));
    }

    #[test]
    fn test_should_retry_connection_refused() {
        assert!(RetryConfig::should_retry("connection refused"));
    }

    #[test]
    fn test_should_retry_dns_error() {
        assert!(RetryConfig::should_retry("dns resolution failed"));
    }

    #[test]
    fn test_should_retry_reset() {
        assert!(RetryConfig::should_retry("connection reset by peer"));
    }

    #[test]
    fn test_should_retry_tls_error() {
        assert!(RetryConfig::should_retry("tls handshake error"));
    }

    #[test]
    fn test_should_retry_eof() {
        assert!(RetryConfig::should_retry("unexpected eof"));
    }

    #[test]
    fn test_should_retry_broken_pipe() {
        assert!(RetryConfig::should_retry("broken pipe"));
    }

    #[test]
    fn test_should_not_retry_404() {
        assert!(!RetryConfig::should_retry("HTTP 404 Not Found"));
    }

    #[test]
    fn test_should_not_retry_unauthorized() {
        assert!(!RetryConfig::should_retry("HTTP 401 Unauthorized"));
    }

    #[test]
    fn test_deserialize_retry_config() {
        let toml_str = r#"
max_attempts = 5
base_delay_ms = 500
"#;
        let cfg: RetryConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.max_attempts, 5);
        assert_eq!(cfg.base_delay_ms, 500);
    }

    #[test]
    fn test_deserialize_retry_config_defaults() {
        let cfg: RetryConfig = toml::from_str("").unwrap_or_default();
        assert_eq!(cfg.max_attempts, 3);
        assert_eq!(cfg.base_delay_ms, 1000);
    }

    // ── retry_async tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_retry_async_succeeds_first_try() {
        let cfg = RetryConfig::default();
        let result = retry_async(&cfg, |attempt| async move {
            assert_eq!(attempt, 0);
            Ok::<&str, String>("ok")
        })
        .await;
        assert_eq!(result.unwrap(), "ok");
    }

    #[tokio::test]
    async fn test_retry_async_succeeds_after_retries() {
        let cfg = RetryConfig::new(3, 10); // fast retries for test
        let mut call_count = 0;
        let result = retry_async(&cfg, |_attempt| {
            call_count += 1;
            async move {
                if call_count < 3 {
                    Err::<&str, _>("connection timeout".to_string())
                } else {
                    Ok("finally ok")
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), "finally ok");
        assert_eq!(call_count, 3);
    }

    #[tokio::test]
    async fn test_retry_async_non_retryable_error_fails_immediately() {
        let cfg = RetryConfig::new(5, 10);
        let result = retry_async(&cfg, |_attempt| async move {
            Err::<&str, _>("HTTP 404 Not Found".to_string())
        })
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_async_exhausts_attempts() {
        let cfg = RetryConfig::new(3, 10);
        let mut call_count = 0;
        let result: Result<&str, String> = retry_async(&cfg, |_attempt| {
            call_count += 1;
            async move { Err("connection refused".to_string()) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(call_count, 3);
    }
}
