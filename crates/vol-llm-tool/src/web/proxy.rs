//! Proxy configuration for web tools.
//!
//! A standalone, reusable proxy configuration that can be embedded in any tool's config struct.
//! Supports a priority chain: tool parameter > agent config > environment variable.

use serde::Deserialize;

/// Proxy configuration for HTTP clients.
///
/// Can be embedded in any tool config struct to provide proxy support.
///
/// # Resolution priority (highest to lowest)
///
/// 1. Tool parameter (`proxy_url` passed by the LLM at call time)
/// 2. Agent config (`proxy.proxy_url` in TOML/YAML tool config)
/// 3. Environment variable (`HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY`)
#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
pub struct ProxyConfig {
    /// Proxy URL (e.g., `http://proxy.example.com:8080` or `socks5://...`)
    pub proxy_url: Option<String>,
}

impl ProxyConfig {
    /// Create a `ProxyConfig` from the standard environment variables.
    ///
    /// Checks `HTTPS_PROXY` first, then `HTTP_PROXY`, then `ALL_PROXY`.
    /// Returns `None`-proxy_url if none are set.
    pub fn from_env() -> Self {
        let url = std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .or_else(|_| std::env::var("HTTP_PROXY"))
            .or_else(|_| std::env::var("http_proxy"))
            .or_else(|_| std::env::var("ALL_PROXY"))
            .or_else(|_| std::env::var("all_proxy"))
            .ok();

        Self { proxy_url: url }
    }

    /// Resolve the effective proxy URL using the priority chain.
    ///
    /// Priority (highest to lowest):
    /// 1. `override_url` — typically the tool parameter from the LLM
    /// 2. `self.proxy_url` — the agent-level or tool config
    /// 3. Environment variable (`HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY`)
    ///
    /// Returns `None` if no proxy is configured at any level.
    pub fn resolve(&self, override_url: Option<&str>) -> Option<String> {
        // 1. Tool parameter (LLM-provided)
        if let Some(url) = override_url {
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }

        // 2. Agent/tool config
        if let Some(ref url) = self.proxy_url {
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }

        // 3. Environment variable
        Self::from_env().proxy_url
    }

    /// Returns `true` if a proxy URL is configured at any level (including env).
    pub fn is_configured(&self) -> bool {
        self.resolve(None).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── from_env tests ──────────────────────────────────────────────────

    #[test]
    fn test_from_env_no_vars_returns_none() {
        // Clear proxy env vars for this test
        let vars = [
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
        ];
        let saved: Vec<_> = vars.iter().map(|v| (v, std::env::var(v).ok())).collect();
        for v in &vars {
            std::env::remove_var(v);
        }

        let cfg = ProxyConfig::from_env();
        assert!(cfg.proxy_url.is_none());

        // Restore
        for (v, val) in saved {
            if let Some(val) = val {
                std::env::set_var(v, val);
            }
        }
    }

    #[test]
    fn test_from_env_prefers_https_proxy() {
        std::env::set_var("HTTPS_PROXY", "https://secure.proxy:443");
        std::env::set_var("HTTP_PROXY", "http://insecure.proxy:80");

        let cfg = ProxyConfig::from_env();
        assert_eq!(cfg.proxy_url.as_deref(), Some("https://secure.proxy:443"));

        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("HTTP_PROXY");
    }

    #[test]
    fn test_from_env_falls_back_to_all_proxy() {
        std::env::set_var("ALL_PROXY", "socks5://all.proxy:1080");

        let cfg = ProxyConfig::from_env();
        assert_eq!(cfg.proxy_url.as_deref(), Some("socks5://all.proxy:1080"));

        std::env::remove_var("ALL_PROXY");
    }

    // ── resolve tests ───────────────────────────────────────────────────

    #[test]
    fn test_resolve_override_wins_over_config() {
        let cfg = ProxyConfig {
            proxy_url: Some("http://config.proxy:8080".into()),
        };
        let result = cfg.resolve(Some("http://tool.proxy:3128"));
        assert_eq!(result.as_deref(), Some("http://tool.proxy:3128"));
    }

    #[test]
    fn test_resolve_config_wins_over_env() {
        std::env::set_var("HTTP_PROXY", "http://env.proxy:80");

        let cfg = ProxyConfig {
            proxy_url: Some("http://config.proxy:8080".into()),
        };
        let result = cfg.resolve(None);
        assert_eq!(result.as_deref(), Some("http://config.proxy:8080"));

        std::env::remove_var("HTTP_PROXY");
    }

    #[test]
    fn test_resolve_falls_back_to_env() {
        std::env::set_var("HTTPS_PROXY", "https://env.proxy:443");

        let cfg = ProxyConfig::default();
        let result = cfg.resolve(None);
        assert_eq!(result.as_deref(), Some("https://env.proxy:443"));

        std::env::remove_var("HTTPS_PROXY");
    }

    #[test]
    fn test_resolve_empty_override_skips_to_config() {
        let cfg = ProxyConfig {
            proxy_url: Some("http://config.proxy:8080".into()),
        };
        let result = cfg.resolve(Some(""));
        assert_eq!(result.as_deref(), Some("http://config.proxy:8080"));
    }

    #[test]
    fn test_resolve_none_when_nothing_configured() {
        let vars = [
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
        ];
        let saved: Vec<_> = vars.iter().map(|v| (v, std::env::var(v).ok())).collect();
        for v in &vars {
            std::env::remove_var(v);
        }

        let cfg = ProxyConfig::default();
        assert_eq!(cfg.resolve(None), None);

        for (v, val) in saved {
            if let Some(val) = val {
                std::env::set_var(v, val);
            }
        }
    }

    // ── is_configured tests ─────────────────────────────────────────────

    #[test]
    fn test_is_configured_true_with_config_set() {
        let cfg = ProxyConfig {
            proxy_url: Some("http://proxy:8080".into()),
        };
        assert!(cfg.is_configured());
    }

    #[test]
    fn test_is_configured_false_with_nothing_set() {
        let vars = [
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
        ];
        let saved: Vec<_> = vars.iter().map(|v| (v, std::env::var(v).ok())).collect();
        for v in &vars {
            std::env::remove_var(v);
        }

        let cfg = ProxyConfig::default();
        assert!(!cfg.is_configured());

        for (v, val) in saved {
            if let Some(val) = val {
                std::env::set_var(v, val);
            }
        }
    }

    // ── serde tests ─────────────────────────────────────────────────────

    #[test]
    fn test_deserialize_with_proxy_url() {
        let toml_str = r#"proxy_url = "http://proxy:8080""#;
        let cfg: ProxyConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.proxy_url.as_deref(), Some("http://proxy:8080"));
    }

    #[test]
    fn test_deserialize_default() {
        let cfg: ProxyConfig = toml::from_str("").unwrap_or_default();
        assert!(cfg.proxy_url.is_none());
    }
}
