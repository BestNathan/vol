//! vol-llm-tools-builtin: Built-in tools for LLM Agent.
//!
//! Each tool is a separate sub-crate for optional dependencies.
//! Use `register_all()` to register all tools at once.
//!
//! # Configuration
//!
//! Tools read their configuration from a dynamic `ToolConfig` container.
//! Each tool defines its own config struct and reads from a named section.
//!
//! Example TOML:
//! ```toml
//! [tools.web_search]
//! provider = "tavily"
//! api_key = "${TAVILY_API_KEY}"
//!
//! [tools.web_search.proxy]
//! proxy_url = "http://proxy:8080"
//!
//! [tools.web_fetch]
//! max_content_length = 1048576
//!
//! [tools.web_fetch.proxy]
//! proxy_url = "http://proxy:8080"
//! ```

pub mod config;
pub mod read_tool {
    pub use vol_llm_tools_builtin_read::*;
}

pub mod write_tool {
    pub use vol_llm_tools_builtin_write::*;
}

pub mod edit_tool {
    pub use vol_llm_tools_builtin_edit::*;
}

pub mod glob_tool {
    pub use vol_llm_tools_builtin_glob::*;
}

pub mod grep_tool {
    pub use vol_llm_tools_builtin_grep::*;
}

pub mod bash_tool {
    pub use vol_llm_tools_builtin_bash::*;
}

pub mod web_search_tool {
    pub use vol_llm_tools_builtin_web_search::*;
}

pub mod web_fetch_provider {
    pub use vol_llm_tools_builtin_web_fetch::*;
}

// Re-export all tools for convenience
pub use read_tool::ReadTool;
pub use write_tool::WriteTool;
pub use edit_tool::EditTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
pub use bash_tool::BashTool;
pub use web_search_tool::{WebFetchTool, WebSearchTool};
pub use web_search_tool::tavily::TavilySearchProvider;
pub use web_fetch_provider::DefaultFetchProvider;
pub use config::{WebFetchConfig, WebSearchConfig};

// Re-export error type
pub use read_tool::BuiltinToolError;

// Re-export config types from vol-llm-tool
pub use vol_llm_tool::{ProxyConfig, ToolConfig};

/// Register all built-in tools to a ToolRegistry
pub fn register_all(registry: &mut vol_llm_tool::ToolRegistry) {
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GlobTool::new());
    registry.register(GrepTool::new());
    registry.register(BashTool::new());
}

/// Register web tools to a ToolRegistry using dynamic configuration.
///
/// Reads tool configurations from the `ToolConfig` container.
/// Tools that are not configured are silently skipped.
pub fn register_web_all(
    registry: &mut vol_llm_tool::ToolRegistry,
    tool_config: &ToolConfig,
) {
    // Register web search if configured
    if let Some(search_cfg) = tool_config.get::<WebSearchConfig>("web_search") {
        match TavilySearchProvider::from_config(&vol_llm_tools_builtin_web_search::tavily::TavilyConfig {
            api_key: search_cfg.api_key,
            proxy: search_cfg.proxy,
        }) {
            Ok(provider) => {
                registry.register(WebSearchTool::new(provider));
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create web search provider, skipping");
            }
        }
    }

    // Register web fetch if configured
    if let Some(fetch_cfg) = tool_config.get::<WebFetchConfig>("web_fetch") {
        let fetch_provider_cfg = vol_llm_tools_builtin_web_fetch::FetchProviderConfig {
            max_content_length: fetch_cfg.max_content_length,
            proxy: fetch_cfg.proxy,
        };
        match DefaultFetchProvider::from_config(&fetch_provider_cfg) {
            Ok(provider) => {
                registry.register(WebFetchTool::new(provider));
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create web fetch provider, skipping");
            }
        }
    }
}
