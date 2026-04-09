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
pub mod run_context;

pub use agent::{ReActAgent, AgentConfig};
pub use builder::AgentBuilder;
pub use response::{AgentResponse, AgentError};
pub use stream::{AgentStreamEvent, AgentStreamReceiver};
pub use prompt::{default_system_prompt, SystemPromptBuilder};
pub use plugin::{AgentPlugin, PluginDecision, PluginRegistry};
pub use plugin_stream::{PluginStream, create_shortcircuit_stream, create_skip_stream, run_interceptor_loop};
pub use run_context::{RunContext, PluginRequest};
pub use hitl::{ApprovalChannel, ApprovalRequest, ApprovalResponse, ApprovalType, HitlConfig, ApprovalTrigger, TimeoutBehavior};

// Re-export prompt_context types for convenience
pub use crate::prompt_context::{PromptContext, PromptTemplate, PromptFragment, FragmentType, MessageAssembler};
