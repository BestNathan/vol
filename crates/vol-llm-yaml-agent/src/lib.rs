//! vol-llm-yaml-agent: Declarative agent definitions via YAML.
//!
//! Parse a YAML file into a fully configured ReActAgent.
//!
//! # Example
//!
//! ```ignore
//! let agent = YamlAgentBuilder::from_file(".agents/agents/coding.yaml")?
//!     .build()?;
//! let response = agent.run("Hello!").await?;
//! ```

mod builder;
mod config;
mod discovery;
mod error;
mod plugins;
mod tools;

pub use builder::YamlAgentBuilder;
pub use config::YamlAgentConfig;
pub use discovery::{discover_agents, discover_from_workdir};
pub use error::YamlAgentError;
