//! vol-llm-agents: Business Agents for LLM-powered analysis.

pub mod advice;

pub use advice::{AgentAdviceService, AgentAdviceConfig};
pub use advice::FrequencyLimiter;
pub use advice::system_prompt;
