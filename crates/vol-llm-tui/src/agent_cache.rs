//! Pre-built agent configuration cache to avoid per-run reconstruction.

use std::path::PathBuf;
use vol_llm_tool::ToolConfig;

pub struct AgentCache {
    pub working_dir: PathBuf,
    pub store_dir: PathBuf,
    pub tool_config: ToolConfig,
}

impl AgentCache {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        let mut tool_config = ToolConfig::new();

        if let Ok(tavily_key) = std::env::var("TAVILY_API_KEY") {
            tool_config.set(
                "web_search",
                vol_llm_tools_builtin::WebSearchConfig {
                    provider: "tavily".to_string(),
                    api_key: tavily_key,
                    proxy: vol_llm_tool::ProxyConfig::default(),
                },
            );
        }

        if let Ok(max_len) = std::env::var("WEB_FETCH_MAX_LENGTH") {
            tool_config.set(
                "web_fetch",
                vol_llm_tools_builtin::WebFetchConfig {
                    max_content_length: max_len.parse().ok(),
                    proxy: vol_llm_tool::ProxyConfig::default(),
                },
            );
        }

        Self {
            working_dir,
            store_dir,
            tool_config,
        }
    }
}
