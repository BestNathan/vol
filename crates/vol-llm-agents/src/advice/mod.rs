//! Advice Agent: AI-powered alert analysis and advice.

mod service;
mod limiter;
mod prompt;

pub use service::{AgentAdviceService, AgentAdviceConfig};
pub use limiter::FrequencyLimiter;
pub use prompt::{system_prompt, build_user_prompt, get_threshold_from_alert};
