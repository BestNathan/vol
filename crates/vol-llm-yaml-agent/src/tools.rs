//! Tool registration by name for YAML agent definitions.

use vol_llm_tool::ToolRegistry;

/// Register a tool by name to the registry.
///
/// Supported names: read, write, edit, glob, grep, bash, web_search, web_fetch
pub fn register_tool_by_name(
    registry: &mut ToolRegistry,
    name: &str,
) -> Result<(), crate::error::YamlAgentError> {
    use crate::error::YamlAgentError;
    use vol_llm_tools_builtin::{
        ReadTool, WriteTool, EditTool, GlobTool, GrepTool, BashTool,
        WebSearchTool, WebFetchTool, TavilySearchProvider, DefaultFetchProvider,
    };

    match name {
        "read" => registry.register(ReadTool::new()),
        "write" => registry.register(WriteTool::new()),
        "edit" => registry.register(EditTool::new()),
        "glob" => registry.register(GlobTool::new()),
        "grep" => registry.register(GrepTool::new()),
        "bash" => registry.register(BashTool::new()),
        "web_search" => {
            if let Ok(api_key) = std::env::var("TAVILY_API_KEY") {
                match TavilySearchProvider::new(api_key, None) {
                    Ok(provider) => registry.register(WebSearchTool::new(provider)),
                    Err(e) => {
                        tracing::warn!("web_search provider init failed: {}", e);
                        return Ok(());
                    }
                }
            } else {
                tracing::warn!("TAVILY_API_KEY not set, skipping web_search");
                return Ok(());
            }
        }
        "web_fetch" => {
            match DefaultFetchProvider::new(None) {
                Ok(provider) => registry.register(WebFetchTool::new(provider)),
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
) -> Result<(), crate::error::YamlAgentError> {
    for name in names {
        register_tool_by_name(registry, name)?;
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
            register_tool_by_name(&mut registry, name).unwrap();
        }
    }

    #[test]
    fn test_register_unknown_tool() {
        let mut registry = ToolRegistry::new();
        let err = register_tool_by_name(&mut registry, "quantum_tool").unwrap_err();
        assert!(err.to_string().contains("quantum_tool"));
    }

    #[test]
    fn test_register_multiple_tools() {
        let mut registry = ToolRegistry::new();
        let names = vec!["read".to_string(), "write".to_string()];
        register_tools_by_name(&mut registry, &names).unwrap();
    }
}
