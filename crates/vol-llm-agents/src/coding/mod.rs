//! Coding Agent: AI-powered code assistant.

mod agent;
mod channelled_observer;
mod config;
mod error;
mod hitl;
mod html_reporter;
mod observer;
mod observer_plugin;

pub use agent::{CodingAgent, CodingAgentBuilder};
pub use channelled_observer::ChannelledEventObserver;
pub use config::CodingAgentConfig;
pub use error::{CodingAgentError, ObserverError, HITLError};
pub use hitl::{HITLDecision, HITLHandler};
pub use html_reporter::HTMLReporter;
pub use observer::EventObserver;
pub use observer_plugin::ObserverPlugin;
