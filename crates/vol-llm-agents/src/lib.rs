//! vol-llm-agents: Business Agents for LLM-powered analysis.

pub mod advice;
pub mod ppt;
pub mod qa;
pub mod coding;
pub mod wiki;

pub use advice::system_prompt;
pub use advice::FrequencyLimiter;
pub use advice::{AdviceAgent, AdviceAgentConfig};
pub use qa::{QaAgent, QaAgentConfig, QaResponse};
pub use coding::{CodingAgent, CodingAgentConfig};
pub use wiki::{WikiAgent, WikiAgentConfig, WikiCompressResult, WikiLoader, WikiInjector};
