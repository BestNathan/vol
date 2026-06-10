//! Coding Agent: AI-powered code assistant.

mod agent;
mod channelled_observer;
mod compressor;
mod config;
mod error;
mod hitl;
mod html_reporter;
mod observer;
mod observer_plugin;
mod sandbox;

pub use agent::{CodingAgent, CodingAgentBuilder, CodingAgentResponse};
pub use channelled_observer::ChannelledEventObserver;
pub use compressor::{ConversationCompressor, SessionCompressor, ToolCallCompressor};
pub use config::CodingAgentConfig;
pub use error::{CodingAgentError, HITLError, ObserverError};
pub use hitl::{HITLDecision, HITLHandler};
pub use html_reporter::HTMLReporter;
pub use observer::EventObserver;
pub use observer_plugin::ObserverPlugin;
pub use sandbox::LocalSandbox;

// Re-export ToolConfig so users can configure web tools
pub use vol_llm_tool::ToolConfig;

#[cfg(test)]
mod tests;
