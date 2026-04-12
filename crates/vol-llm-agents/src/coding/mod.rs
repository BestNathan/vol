//! Coding Agent: AI-powered code assistant.

mod agent;
mod config;
mod error;
mod hitl;
mod html_reporter;
mod observer;

pub use agent::{CodingAgent, CodingAgentBuilder};
pub use config::CodingAgentConfig;
pub use error::{CodingAgentError, ObserverError, HITLError};
pub use hitl::{HITLDecision, HITLHandler};
pub use html_reporter::HTMLReporter;
pub use observer::EventObserver;
