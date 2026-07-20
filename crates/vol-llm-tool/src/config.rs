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
        self.tools.get(name).and_then(|v| v.clone().try_into().ok())
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
    /// Converts `serde_json::Value` entries (from YAML frontmatter) into
    /// `toml::Value` for internal storage.
    pub fn populate_from_agent_def(
        &mut self,
        tool_config_map: &HashMap<String, serde_json::Value>,
    ) {
        for (name, value) in tool_config_map {
            // Convert serde_json::Value → toml::Value via serde deserialization
            if let Ok(toml_val) = serde_json::from_value::<toml::Value>(value.clone()) {
                self.tools.insert(name.clone(), toml_val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    struct TestToolConfig {
        api_key: String,
        #[serde(default)]
        timeout: u64,
    }

    #[test]
    fn test_tool_config_new_is_empty() {
        let config = ToolConfig::new();
        let result: Option<TestToolConfig> = config.get("any_tool");
        assert!(result.is_none());
    }

    #[test]
    fn test_tool_config_set_and_get_roundtrip() {
        let mut config = ToolConfig::new();
        let tc = TestToolConfig {
            api_key: "sk-test".into(),
            timeout: 30,
        };
        config.set("my_tool", &tc);

        let retrieved: TestToolConfig = config.get("my_tool").expect("should retrieve config");
        assert_eq!(retrieved.api_key, "sk-test");
        assert_eq!(retrieved.timeout, 30);
    }

    #[test]
    fn test_tool_config_get_nonexistent() {
        let config = ToolConfig::new();
        let result: Option<TestToolConfig> = config.get("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_tool_config_get_wrong_type_returns_none() {
        let mut config = ToolConfig::new();
        #[derive(serde::Deserialize)]
        struct OtherConfig {
            field: String,
        }
        config.set(
            "my_tool",
            &TestToolConfig {
                api_key: "sk".into(),
                timeout: 5,
            },
        );

        // Try to deserialize with wrong shape
        let result: Option<OtherConfig> = config.get("my_tool");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_sandbox_returns_sandbox_name() {
        let mut config = ToolConfig::new();
        let mut map = HashMap::new();
        map.insert(
            "browser".to_string(),
            serde_json::json!({"sandbox": "docker-sandbox"}),
        );
        config.populate_from_agent_def(&map);

        let sandbox = config.get_sandbox("browser");
        assert_eq!(sandbox, Some("docker-sandbox".to_string()));
    }

    #[test]
    fn test_get_sandbox_returns_none_when_not_configured() {
        let config = ToolConfig::new();
        assert_eq!(config.get_sandbox("nonexistent"), None);
    }

    #[test]
    fn test_get_sandbox_returns_none_when_no_sandbox_key() {
        let mut config = ToolConfig::new();
        let mut map = HashMap::new();
        map.insert(
            "simple_tool".to_string(),
            serde_json::json!({"timeout": 30}),
        );
        config.populate_from_agent_def(&map);

        assert_eq!(config.get_sandbox("simple_tool"), None);
    }

    #[test]
    fn test_populate_from_agent_def_multiple_tools() {
        let mut config = ToolConfig::new();
        let mut map = HashMap::new();
        map.insert(
            "tool_a".to_string(),
            serde_json::json!({"api_key": "key-a"}),
        );
        map.insert(
            "tool_b".to_string(),
            serde_json::json!({"api_key": "key-b", "timeout": 60}),
        );
        config.populate_from_agent_def(&map);

        let a: TestToolConfig = config.get("tool_a").expect("tool_a should exist");
        assert_eq!(a.api_key, "key-a");
        assert_eq!(a.timeout, 0); // default

        let b: TestToolConfig = config.get("tool_b").expect("tool_b should exist");
        assert_eq!(b.api_key, "key-b");
        assert_eq!(b.timeout, 60);
    }

    #[test]
    fn test_populate_from_agent_def_overwrites_existing() {
        let mut config = ToolConfig::new();
        let old = TestToolConfig {
            api_key: "old".into(),
            timeout: 1,
        };
        config.set("tool_x", &old);

        let mut map = HashMap::new();
        map.insert(
            "tool_x".to_string(),
            serde_json::json!({"api_key": "new", "timeout": 99}),
        );
        config.populate_from_agent_def(&map);

        let val: TestToolConfig = config.get("tool_x").unwrap();
        assert_eq!(val.api_key, "new");
        assert_eq!(val.timeout, 99);
    }

    #[test]
    fn test_common_tool_config_deserialization() {
        let toml_str = r#"
sandbox = "my-sandbox"
"#;
        let cfg: CommonToolConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.sandbox, Some("my-sandbox".to_string()));
    }

    #[test]
    fn test_common_tool_config_no_sandbox() {
        let cfg: CommonToolConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.sandbox, None);
    }
}
