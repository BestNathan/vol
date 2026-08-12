//! Tool registration by name for YAML agent definitions.

use vol_llm_tool::ToolRegistry;

/// Register a tool by name to the registry.
///
/// Supported names: read, write, edit, glob, grep, bash, web_search, web_fetch
///
/// When `tool_config` is provided, it is consulted for per-tool configuration
/// (proxy, retry, etc.) when registering web tools.
pub fn register_tool_by_name(
    registry: &mut ToolRegistry,
    name: &str,
    tool_config: Option<&vol_llm_tool::ToolConfig>,
) -> Result<(), crate::error::YamlAgentError> {
    use crate::error::YamlAgentError;
    use vol_llm_tools_builtin::{
        BashTool, DefaultFetchProvider, EditTool, GlobTool, GrepTool, ReadTool,
        TavilySearchProvider, WebFetchTool, WebSearchTool, WriteTool,
    };

    match name {
        "read" => registry.register(ReadTool::new()),
        "write" => registry.register(WriteTool::new()),
        "edit" => registry.register(EditTool::new()),
        "glob" => registry.register(GlobTool::new()),
        "grep" => registry.register(GrepTool::new()),
        "bash" => registry.register(BashTool::new()),
        "web_search" => {
            // Build config from tool_config if available, otherwise use defaults
            let cfg = tool_config
                .and_then(|tc| tc.get::<vol_llm_tools_builtin::WebSearchConfig>("web_search"))
                .unwrap_or_default();

            if cfg.api_key.is_empty() {
                tracing::warn!("TAVILY_API_KEY not set, skipping web_search");
                return Ok(());
            }

            let proxy = cfg.proxy.clone();
            match TavilySearchProvider::from_config(
                &vol_llm_tools_builtin::web_search_tool::tavily::TavilyConfig {
                    api_key: cfg.api_key,
                    proxy: cfg.proxy,
                    retry: cfg.retry,
                },
            ) {
                Ok(provider) => {
                    registry.register(WebSearchTool::with_proxy(provider, proxy));
                }
                Err(e) => {
                    tracing::warn!("web_search provider init failed: {}", e);
                    return Ok(());
                }
            }
        }
        "web_fetch" => {
            let cfg = tool_config
                .and_then(|tc| tc.get::<vol_llm_tools_builtin::WebFetchConfig>("web_fetch"))
                .unwrap_or_default();

            let proxy = cfg.proxy.clone();
            let fetch_provider_cfg =
                vol_llm_tools_builtin::web_fetch_provider::FetchProviderConfig {
                    max_content_length: cfg.max_content_length,
                    proxy: cfg.proxy,
                    retry: cfg.retry,
                };
            match DefaultFetchProvider::from_config(&fetch_provider_cfg) {
                Ok(provider) => {
                    registry.register(WebFetchTool::with_proxy(provider, proxy));
                }
                Err(e) => {
                    tracing::warn!("web_fetch provider init failed: {}", e);
                    return Ok(());
                }
            }
        }
        _ => return Err(YamlAgentError::UnknownTool(name.to_string())),
    }

    Ok(())
}

/// Register multiple tools by name.
pub fn register_tools_by_name(
    registry: &mut ToolRegistry,
    names: &[String],
    tool_config: Option<&vol_llm_tool::ToolConfig>,
) -> Result<(), crate::error::YamlAgentError> {
    for name in names {
        register_tool_by_name(registry, name, tool_config)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_core_tools() {
        let mut registry = ToolRegistry::new();
        for name in &["read", "write", "edit", "glob", "grep", "bash"] {
            register_tool_by_name(&mut registry, name, None).unwrap();
        }
    }

    #[test]
    fn test_register_unknown_tool() {
        let mut registry = ToolRegistry::new();
        let err = register_tool_by_name(&mut registry, "quantum_tool", None).unwrap_err();
        assert!(err.to_string().contains("quantum_tool"));
    }

    #[test]
    fn test_register_multiple_tools() {
        let mut registry = ToolRegistry::new();
        let names = vec!["read".to_string(), "write".to_string()];
        register_tools_by_name(&mut registry, &names, None).unwrap();
    }
}
