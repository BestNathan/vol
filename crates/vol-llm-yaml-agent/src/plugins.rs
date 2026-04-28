//! Plugin registration by name for YAML agent definitions.

use std::path::Path;
use vol_llm_agent::react::PluginRegistry;

/// Register a plugin by name.
///
/// Supported names: logger (writes JSONL to store_dir/logs/)
pub fn register_plugin_by_name(
    registry: &mut PluginRegistry,
    name: &str,
    working_dir: &Path,
) -> Result<(), crate::error::YamlAgentError> {
    use crate::error::YamlAgentError;

    match name {
        "logger" => {
            let logger = vol_llm_observability::LoggerPlugin::new(working_dir.to_path_buf());
            registry.register(logger);
        }
        _ => return Err(YamlAgentError::UnknownPlugin(name.to_string())),
    }

    Ok(())
}

/// Register multiple plugins by name.
pub fn register_plugins_by_name(
    registry: &mut PluginRegistry,
    names: &[String],
    working_dir: &Path,
) -> Result<(), crate::error::YamlAgentError> {
    for name in names {
        register_plugin_by_name(registry, name, working_dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_logger() {
        let mut registry = PluginRegistry::new();
        let temp = tempfile::tempdir().unwrap();
        register_plugin_by_name(&mut registry, "logger", temp.path()).unwrap();
    }

    #[test]
    fn test_register_unknown_plugin() {
        let mut registry = PluginRegistry::new();
        let temp = tempfile::tempdir().unwrap();
        let err = register_plugin_by_name(&mut registry, "magic", temp.path()).unwrap_err();
        assert!(err.to_string().contains("magic"));
    }
}
