//! Dynamic tool configuration container.
//!
//! Uses a `HashMap<String, toml::Value>` to store tool configs by name,
//! allowing new tools to be added without modifying this struct.
//!
//! Each tool defines its own config struct with `Deserialize` and reads
//! its section from the container via `get::<T>("tool_name")`.

use serde::de::DeserializeOwned;
use std::collections::HashMap;

/// Dynamic container for tool configurations.
///
/// Tools register their config by name and retrieve it via a typed getter.
/// No need to add fields for each new tool.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ToolConfig {
    #[serde(default)]
    tools: HashMap<String, toml::Value>,
}

/// Common configuration shared by all tools.
///
/// Every tool entry in `ToolConfig` may include an optional `sandbox` key
/// to route that tool's execution to a specific sandbox.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommonToolConfig {
    #[serde(default)]
    pub sandbox: Option<String>,
}

impl ToolConfig {
    /// Create an empty tool configuration container.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the sandbox name configured for a tool (if any).
    ///
    /// Reads the `sandbox` key from the tool's config table.
    /// Returns `None` if the tool is not configured or has no sandbox key.
    pub fn get_sandbox(&self, tool_name: &str) -> Option<String> {
        self.get::<CommonToolConfig>(tool_name)
            .and_then(|c| c.sandbox)
    }

    /// Get a typed configuration for the tool with the given name.
    ///
    /// Returns `None` if the tool is not configured or the config
    /// cannot be deserialized into the target type.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config: WebSearchConfig = tool_config.get("web_search")?;
    /// ```
    pub fn get<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        self.tools
            .get(name)
            .and_then(|v| v.clone().try_into().ok())
    }

    /// Set a configuration for the tool with the given name.
    ///
    /// # Example
    ///
    /// ```ignore
    /// tool_config.set("web_search", WebSearchConfig { ... });
    /// ```
    pub fn set<T: serde::Serialize>(&mut self, name: &str, config: T) {
        if let Ok(value) = toml::Value::try_from(config) {
            self.tools.insert(name.to_string(), value);
        }
    }

    /// Populate tool configurations from an `AgentDef.tool_config` map.
    ///
    /// Each entry is inserted directly into the tool config store by name.
    pub fn populate_from_agent_def(&mut self, tool_config_map: &HashMap<String, toml::Value>) {
        for (name, value) in tool_config_map {
            self.tools.insert(name.clone(), value.clone());
        }
    }
}
