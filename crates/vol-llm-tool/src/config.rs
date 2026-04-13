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

impl ToolConfig {
    /// Create an empty tool configuration container.
    pub fn new() -> Self {
        Self::default()
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
}
