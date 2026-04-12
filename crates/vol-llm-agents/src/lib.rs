//! vol-llm-agents: Business Agents for LLM-powered analysis.

pub mod advice;
pub mod ppt;
pub mod qa;

pub use advice::system_prompt;
pub use advice::FrequencyLimiter;
pub use advice::{AdviceAgent, AdviceAgentConfig};
pub use qa::{QaAgent, QaAgentConfig, QaResponse};
