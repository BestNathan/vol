//! vol-llm-agents: Business Agents for LLM-powered analysis.

pub mod coding;
pub mod ppt;
pub mod qa;
pub mod wiki;

pub use coding::{CodingAgent, CodingAgentConfig};
pub use qa::{QaAgent, QaAgentConfig, QaResponse};
pub use wiki::{WikiAgent, WikiAgentConfig, WikiCompressResult, WikiInjector, WikiLoader};
