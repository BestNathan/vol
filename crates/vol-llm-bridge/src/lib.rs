//! vol-llm-bridge: AI-powered alert analysis and advice service.
//!
//! Subscribes to alert broadcast, queries historical data from TDengine,
//! generates analysis advice using ReAct Agent, and sends to Feishu.

pub mod limiter;
pub mod service;
pub mod prompt;

pub use limiter::FrequencyLimiter;
pub use service::AgentAdviceService;
pub use prompt::system_prompt;
