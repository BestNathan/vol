//! TOML config schema for a CLI tool.
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CliToolConfig {
    pub name: String,
    #[serde(default)]
    pub _placeholder: (),
}
