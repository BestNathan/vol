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
pub mod hitl;
pub mod plugin;
pub mod plugin_stream;
pub mod prompt;
pub mod response;
pub mod run_context;
pub mod state;
pub mod stream;

pub use agent::{AgentConfig, ReActAgent};
pub use builder::AgentBuilder;
pub use hitl::{
    run_cli_approval_loop, ApprovalChannel, ApprovalTrigger, ApprovalType, HitlConfig,
    TimeoutBehavior,
};
pub use plugin::{AgentPlugin, PluginDecision, PluginRegistry};
pub use plugin_stream::{
    create_shortcircuit_stream, create_skip_stream, run_interceptor_loop, spawn_listener_task,
};
pub use prompt::{default_system_prompt, SystemPromptBuilder};
pub use response::{AgentError, AgentResponse};
pub use run_context::{ApprovalRequest, ApprovalResponse, PluginContext, PluginRequest, RunContext};
pub use state::{ReasoningStep, ToolCallRecord};
pub use stream::{AgentStreamEvent, AgentStreamReceiver};

// Re-export prompt_context types for convenience
pub use crate::prompt_context::{
    FragmentType, MessageAssembler, PromptContext, PromptFragment, PromptTemplate,
};
