//! Advice Agent: AI-powered alert analysis and advice.

mod limiter;
mod prompt;
mod service;

pub use limiter::FrequencyLimiter;
pub use prompt::{build_user_prompt, get_threshold_from_alert, system_prompt};
pub use service::{AdviceAgent, AdviceAgentConfig};
