//! vol-llm-yaml-agent: Declarative agent definitions via YAML.
//!
//! Parse a YAML file into a fully configured ReActAgent.
//!
//! # Example
//!
//! ```ignore
//! let agent = YamlAgentBuilder::from_file(".agent/agents/coding.yaml")?
//!     .build()?;
//! let response = agent.run("Hello!").await?;
//! ```

mod config;
mod error;
mod builder;
mod tools;
mod plugins;
mod discovery;

pub use config::YamlAgentConfig;
pub use error::YamlAgentError;
pub use builder::YamlAgentBuilder;
pub use discovery::{discover_agents, discover_from_workdir};
