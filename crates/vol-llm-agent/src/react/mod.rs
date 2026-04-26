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

pub use agent::{AgentConfig, ReActAgent, SkillsConfig};
pub use builder::AgentBuilder;
pub use hitl::{
    run_cli_approval_loop, spawn_custom_approval_handler, ApprovalChannel, ApprovalHandler,
    ApprovalTrigger, ApprovalType, BoxedApprovalHandler, HitlConfig, TimeoutBehavior,
};
pub use plugin::{AgentPlugin, PluginContext, PluginDecision, PluginRegistry, PluginId};
pub use plugin_stream::{
    create_shortcircuit_stream, create_skip_stream, run_interceptor_loop, spawn_listener_task,
};
pub use prompt::{default_system_prompt, SystemPromptBuilder};
pub use response::{AgentError, AgentResponse};
pub use run_context::{ApprovalRequest, ApprovalResponse, PluginRequest, RunContext, plugin_context_from_run_ctx};
pub use state::{ReasoningStep, ToolCallRecord};
pub use stream::{AgentStreamEvent, AgentStreamReceiver};

// Re-export vol-llm-context types for convenience
pub use vol_llm_context::{
    AttentionAnchor, ContextBlock, ContextBuilder, ContextBuilderBuilder, ContextContributor,
    ContextError,
};

#[cfg(test)]
mod tests;
