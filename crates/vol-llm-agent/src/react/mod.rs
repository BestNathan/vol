//! ReAct Agent module.
//!
//! Provides `ReActAgent` for reasoning and acting with tool integration.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    ReActAgent                               │
//! │                                                             │
//! │  - llm: Arc<dyn LLMClient>                                 │
//! │  - tools: Arc<ToolRegistry>                                │
//! │  - config: AgentConfig                                     │
//! │                                                             │
//! │  + run(user_input, context) -> AgentStreamReceiver         │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use vol_llm_agent::react::{ReActAgent, AgentBuilder};
//!
//! // Create agent using builder
//! // let agent = AgentBuilder::new()
//! //     .with_llm(llm)
//! //     .with_tool(my_tool)
//! //     .with_max_iterations(5)
//! //     .build()
//! //     .unwrap();
//! ```

pub mod agent;
pub mod builder;
pub mod response;
pub mod stream;
pub mod prompt;
pub mod plugin;
pub mod plugin_stream;
pub mod hitl;

pub use agent::{ReActAgent, AgentConfig};
pub use builder::AgentBuilder;
pub use response::{AgentResponse, AgentError};
pub use stream::{AgentStreamEvent, AgentStreamReceiver};
pub use prompt::{default_system_prompt, SystemPromptBuilder};
pub use plugin::{AgentPlugin, PluginContext, PluginAction, PluginRegistry};
pub use plugin_stream::{PluginStream, create_shortcircuit_stream, create_skip_stream};
pub use hitl::{ApprovalChannel, ApprovalRequest, ApprovalResponse, ApprovalType, HitlConfig, ApprovalTrigger, TimeoutBehavior};
