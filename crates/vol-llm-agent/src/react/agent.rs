//! ReAct Agent implementation.

use super::{
    AgentInput, AgentResponse, AgentStreamEvent, PluginDecision, PluginRegistry, RunContext,
};
use crate::react::state::ToolCallRecord;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use vol_llm_context::{
    AttentionAnchor, ContextBuilder, ContextBuilderBuilder, ContextContributor, ContextError,
    ContextMessage, ContributorInfo,
};
use vol_llm_core::{
    ConversationRequest, ConversationResponse, LLMClient, Message, StreamEventData, StreamReceiver,
    ToolChoice,
};
use vol_llm_mcp::McpManager;
use vol_llm_sandbox::registry::SandboxRegistry;
use vol_llm_sandbox::SandboxRef;
use vol_llm_tool::{ToolConfig, ToolContext};
use vol_session::{InMemoryEntryStore, Session, SessionContributor};

/// Agent configuration — single source of truth for ReActAgent.
///
/// Clone is intentionally NOT derived. After construction, config is shared
/// via Arc and external code only gets &AgentConfig references.
pub struct AgentConfig {
    // === Declarative definition (optional) ===
    pub def: Option<crate::agent_def::AgentDef>,

    // === Runtime components ===
    pub llm: Arc<dyn vol_llm_core::LLMClient>,
    pub tools: Arc<vol_llm_tool::ToolRegistry>,
    /// Session handle with interior mutability. Read via agent.session(),
    /// write via agent.set_session() (gated by is_running).
    pub(crate) session: std::sync::RwLock<Arc<Session>>,
    pub sandbox: Option<SandboxRef>,
    pub sandbox_registry: Option<Arc<SandboxRegistry>>,
    pub default_sandbox: Option<String>,
    /// Per-tool configuration (includes sandbox overrides, tool-specific settings).
    pub tool_config: ToolConfig,

    // === Context and plugins ===
    pub(crate) context_builder: RwLock<ContextBuilder>,
    pub plugin_registry: PluginRegistry,

    // === MCP ===
    pub mcp_manager: Option<Arc<McpManager>>,

    // === Agent identity ===
    pub agent_id: String,
    /// Working directory. Log paths derive from `{working_dir}/logs/agents/{agent_id}/`.
    pub working_dir: PathBuf,
}

impl AgentConfig {
    /// Create a new builder for AgentConfig.
    pub fn builder() -> super::config_builder::AgentConfigBuilder {
        super::config_builder::AgentConfigBuilder::new()
    }

    /// Add a context contributor.
    pub fn add_contributor(&mut self, contributor: Box<dyn ContextContributor>) {
        self.context_builder
            .write()
            .unwrap()
            .add_contributor(contributor);
    }

    /// List contributor info (for RPC / UI queries).
    pub async fn contributor_infos(&self) -> Result<Vec<ContributorInfo>, ContextError> {
        let cb = self.context_builder.read().unwrap().clone();
        cb.contributor_infos().await
    }

    /// Get message snapshot from a specific contributor.
    pub async fn snapshot_by_name(&self, name: &str) -> Result<Vec<ContextMessage>, ContextError> {
        let cb = self.context_builder.read().unwrap().clone();
        cb.snapshot_by_name(name).await
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            def: None,
            llm: Arc::new(DefaultLlm),
            tools: Arc::new(vol_llm_tool::ToolRegistry::new()),
            session: std::sync::RwLock::new(Arc::new(Session::new(Arc::new(
                InMemoryEntryStore::new(),
            )))),
            sandbox: None,
            sandbox_registry: None,
            default_sandbox: None,
            tool_config: ToolConfig::new(),
            context_builder: RwLock::new(ContextBuilderBuilder::new(128_000).build()),
            plugin_registry: PluginRegistry::new(),
            mcp_manager: None,
            agent_id: generate_agent_id(),
            working_dir: PathBuf::from("."),
        }
    }
}

fn generate_agent_id() -> String {
    format!(
        "agent-{}",
        uuid::Uuid::new_v4().simple().to_string()[..8].to_string()
    )
}

/// Dummy LLM for Default impl (tests only — will panic if used).
struct DefaultLlm;
#[async_trait::async_trait]
impl LLMClient for DefaultLlm {
    fn provider(&self) -> vol_llm_core::LLMProvider {
        vol_llm_core::LLMProvider::Anthropic
    }
    fn model(&self) -> &str {
        "default"
    }
    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[]
    }
    async fn converse(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!(
            "DefaultLlm::converse called — AgentConfig::default() is for struct defaults only"
        )
    }
    async fn converse_stream(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<StreamReceiver> {
        let (_tx, rx) = tokio::sync::mpsc::channel(10);
        Ok(StreamReceiver::new(rx))
    }
}

/// Shared running state — exposed for external status queries.
pub struct RunningState {
    /// true while run_input() is executing.
    pub is_running: std::sync::atomic::AtomicBool,
    /// Current input text (for status display).
    pub current_input: std::sync::RwLock<Option<String>>,
    /// Current run_id (for status display).
    pub current_run_id: std::sync::RwLock<Option<String>>,
}

impl RunningState {
    fn new() -> Self {
        Self {
            is_running: std::sync::atomic::AtomicBool::new(false),
            current_input: std::sync::RwLock::new(None),
            current_run_id: std::sync::RwLock::new(None),
        }
    }
}

/// RAII guard that clears running state on drop (even on panic).
struct RunningGuard<'a> {
    run_state: &'a RunningState,
}

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.run_state
            .is_running
            .store(false, std::sync::atomic::Ordering::Release);
        *self.run_state.current_input.write().unwrap() = None;
        *self.run_state.current_run_id.write().unwrap() = None;
    }
}

/// ReAct Agent — owns config (Arc) and running state.
pub struct ReActAgent {
    config: Arc<AgentConfig>,
    run_state: Arc<RunningState>,
}

impl ReActAgent {
    /// Create a new ReActAgent from config.
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config: Arc::new(config),
            run_state: Arc::new(RunningState::new()),
        }
    }

    // ── Read-only access ──

    /// Immutable reference to config.
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    // ── Contributor API ──

    /// Add a context contributor at runtime.
    pub fn add_contributor(&mut self, contributor: Box<dyn ContextContributor>) {
        self.config
            .context_builder
            .write()
            .unwrap()
            .add_contributor(contributor);
    }

    /// List all contributors with metadata (SOT for external queries).
    pub async fn contributors(&self) -> Result<Vec<ContributorInfo>, ContextError> {
        self.config.contributor_infos().await
    }

    /// Get messages from a specific contributor by name.
    pub async fn snapshot_by_name(&self, name: &str) -> Result<Vec<ContextMessage>, ContextError> {
        self.config.snapshot_by_name(name).await
    }

    /// Cheap clone of the shared session handle.
    pub fn session(&self) -> Arc<Session> {
        self.config.session.read().unwrap().clone()
    }

    /// Whether agent is currently executing run_input().
    pub fn is_running(&self) -> bool {
        self.run_state
            .is_running
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Shared running state for external status queries.
    pub fn run_state(&self) -> &Arc<RunningState> {
        &self.run_state
    }

    // ── Mutation (gated by is_running) ──

    /// Replace the session. Rejected if agent is running.
    /// Also replaces the SessionContributor with a new one pointing to the new session.
    pub fn set_session(&self, session: Arc<Session>) -> Result<(), AgentBusyError> {
        if self.is_running() {
            return Err(AgentBusyError {
                agent_id: self.config.agent_id.clone(),
            });
        }
        let max_history = self
            .config
            .def
            .as_ref()
            .and_then(|d| d.max_history_messages)
            .unwrap_or(50);
        let session_contributor = Box::new(SessionContributor::new(
            Arc::new(tokio::sync::Mutex::new((*session).clone())),
            max_history,
            AttentionAnchor::Tail(0),
        ));
        *self.config.session.write().unwrap() = session;
        self.config
            .context_builder
            .write()
            .unwrap()
            .replace_contributor("session", session_contributor);
        Ok(())
    }

    // ── Builder-style (consuming self, initial setup only) ──

    /// Set the sandbox for tool execution (builder pattern, consumes self).
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        Arc::get_mut(&mut self.config)
            .expect("with_sandbox called after config was shared")
            .sandbox = Some(sandbox);
        self
    }

    // ── Execution ──

    /// Run ReAct loop and return the final response.
    pub async fn run(&self, user_input: &str) -> Result<AgentResponse, crate::AgentError> {
        self.run_input(AgentInput::text(user_input)).await
    }

    pub async fn run_input(&self, input: AgentInput) -> Result<AgentResponse, crate::AgentError> {
        // Re-entrancy guard
        if self
            .run_state
            .is_running
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return Err(crate::AgentError::AlreadyRunning);
        }

        let user_content = input
            .to_message_content()
            .map_err(|e| crate::AgentError::InvalidInput(e.to_string()))?;
        let user_input = input.display_text();
        let run_id = input
            .run_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());

        // Set status metadata
        *self.run_state.current_input.write().unwrap() = Some(user_input.clone());
        *self.run_state.current_run_id.write().unwrap() = Some(run_id.clone());

        // RAII guard: clears running state on drop (even on panic)
        let _guard = RunningGuard {
            run_state: &self.run_state,
        };

        let (run_ctx, plugin_rx) =
            RunContext::new(run_id.clone(), user_input.clone(), self.config.clone());

        for (key, value) in input.metadata {
            run_ctx.data.write().await.insert(key, value);
        }

        // Persist user message to session so it's available via SessionContributor.
        let user_msg = Message::user(user_content);
        run_ctx.add_message(user_msg).await.map_err(|e| {
            crate::AgentError::SessionError(format!("Failed to persist user message: {}", e))
        })?;

        // === Phase 2: Context is built per-iteration via get_context ===

        // === Phase 2.6: Spawn listener and interceptor tasks ===
        use super::plugin_stream::{run_interceptor_loop, spawn_listener_tasks};

        let plugins = self.config.plugin_registry.plugins().to_vec();
        let mut listener_set = spawn_listener_tasks(plugins, run_ctx.clone());

        let interceptor_plugins = self.config.plugin_registry.plugins().to_vec();
        let interceptor_ctx = run_ctx.without_plugin_event_tx();
        let interceptor_handle = tokio::spawn(async move {
            run_interceptor_loop(plugin_rx, interceptor_plugins, interceptor_ctx).await;
        });

        let mut shutdown_event_tx = run_ctx.event_tx.clone();

        // === Phase 3: Spawn agent loop task and await it ===
        let llm = self.config.llm.clone();
        let user_input = user_input.clone();
        let sandbox = self.config.sandbox.clone();
        let agent_def = self.config.def.clone();
        let agent_task = tokio::spawn(async move {
            let max_iterations = run_ctx.max_iterations();
            // === Emit and intercept AgentStart ===
            let start_event = AgentStreamEvent::agent_start(user_input.clone());
            run_ctx.emit(start_event.clone()).await;

            match run_ctx.intercept(&start_event).await {
                Ok(PluginDecision::Continue) => {
                    // Continue with normal flow
                }
                Ok(PluginDecision::Skip) => {
                    // Skip only affects the current event, not the entire run
                }
                Ok(PluginDecision::Abort(reason)) => {
                    run_ctx
                        .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                        .await;
                    return Err(crate::AgentError::Context(reason));
                }
                Err(e) => {
                    tracing::warn!(
                        "Plugin intercept error (plugins may not be wired up yet): {}",
                        e
                    );
                }
            }

            loop {
                // Increment iteration via ctx
                run_ctx.next_iteration();
                let iteration = run_ctx.current_iteration();

                if iteration > max_iterations {
                    run_ctx
                        .emit(AgentStreamEvent::max_iterations_reached(
                            iteration,
                            max_iterations,
                        ))
                        .await;

                    let reason = format!("Max iterations ({}) reached", max_iterations);
                    run_ctx
                        .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                        .await;
                    return Err(crate::AgentError::MaxIterationsReached {
                        max: max_iterations,
                    });
                }

                // Reason phase - call LLM with streaming
                let tools_defs = run_ctx.effective_tools();

                // Get messages from ctx (not local variable)
                let messages = run_ctx
                    .get_context()
                    .await
                    .map_err(|e| crate::AgentError::from(e))?;

                let request = ConversationRequest::with_history(None, messages)
                    .with_tools(tools_defs)
                    .with_tool_choice(ToolChoice::Auto);

                let llm_stream = match llm.converse_stream(request).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        run_ctx
                            .emit(AgentStreamEvent::agent_aborted(format!(
                                "LLM request failed: {}",
                                e
                            )))
                            .await;
                        return Err(crate::AgentError::Llm(e));
                    }
                };

                // Consume LLM stream — emits Thinking/Content streaming events internally
                let (thinking, tool_calls, content, _model, _usage) =
                    match consume_llm_stream(llm_stream, &run_ctx).await {
                        Ok(data) => data,
                        Err(e) => {
                            run_ctx
                                .emit(AgentStreamEvent::agent_aborted(format!(
                                    "LLM stream failed: {}",
                                    e
                                )))
                                .await;
                            return Err(e);
                        }
                    };

                // Record reasoning step
                if !thinking.is_empty() {
                    run_ctx.record_reasoning_step(thinking.clone(), None).await;
                }

                // Check if tool calls
                if !tool_calls.is_empty() {
                    // IMPORTANT: Add assistant message with tool calls to history
                    let assistant_message = {
                        let msg = if !content.is_empty() {
                            Message::assistant_with_tools(content.clone(), tool_calls.clone())
                        } else {
                            Message::assistant_with_tools(
                                "Calling tools to get information.".to_string(),
                                tool_calls.clone(),
                            )
                        };
                        if !thinking.is_empty() {
                            msg.with_thinking(thinking.clone())
                        } else {
                            msg
                        }
                    };
                    if let Err(e) = run_ctx.add_message(assistant_message).await {
                        return Err(crate::AgentError::from(e));
                    }

                    // Act phase - execute tools
                    for call in &tool_calls {
                        // === Emit and intercept ToolCallBegin ===
                        let tool_event = AgentStreamEvent::tool_call_begin(
                            call.id.clone(),
                            call.name.clone(),
                            call.arguments.clone(),
                        );
                        run_ctx.emit(tool_event.clone()).await;
                        let tool_begin = std::time::Instant::now();

                        let tool_decision = match run_ctx.intercept(&tool_event).await {
                            Ok(decision) => decision,
                            Err(e) => {
                                tracing::warn!("Plugin intercept error: {}", e);
                                PluginDecision::Continue
                            }
                        };

                        match tool_decision {
                            PluginDecision::Continue => {
                                // Execute tool directly — approval is handled by HitlPlugin via intercept()
                            }
                            PluginDecision::Skip => {
                                tracing::warn!("Plugin intercepted to skip tool: {}", call.name);
                                let duration_ms = tool_begin.elapsed().as_millis() as u64;

                                run_ctx
                                    .emit(AgentStreamEvent::tool_call_skipped(
                                        call.id.clone(),
                                        call.name.clone(),
                                        "Plugin skipped".to_string(),
                                        Some(duration_ms),
                                    ))
                                    .await;

                                continue;
                            }
                            PluginDecision::Abort(reason) => {
                                run_ctx
                                    .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                                    .await;
                                return Err(crate::AgentError::Context(reason));
                            }
                        }

                        // Resolve sandbox:
                        //   1. ToolConfig.get_sandbox(tool_name) — per-tool override
                        //   2. AgentDef.sandbox — agent default
                        //   3. Registry default ("local")
                        let sandbox_ref = if let Some(ref registry) =
                            run_ctx.config.sandbox_registry
                        {
                            let sandbox_name = run_ctx
                                .config
                                .tool_config
                                .get_sandbox(&call.name)
                                .or_else(|| run_ctx.config.default_sandbox.clone())
                                .unwrap_or_else(|| "local".to_string());
                            registry
                                .acquire(&sandbox_name)
                                .unwrap_or_else(|| registry.default())
                        } else {
                            match &sandbox {
                                Some(sb) => sb.clone(),
                                None => Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None)),
                            }
                        };
                        let mut tool_ctx = ToolContext::default().with_sandbox(sandbox_ref);
                        if let Some(ref def) = agent_def {
                            tool_ctx = tool_ctx.with_agent_def(def.clone());
                        }
                        let result = match run_ctx.execute_tool(call, &tool_ctx).await {
                            Ok(r) => r,
                            Err(e) => {
                                let duration_ms = tool_begin.elapsed().as_millis() as u64;
                                run_ctx
                                    .emit(AgentStreamEvent::tool_call_error(
                                        call.id.clone(),
                                        call.name.clone(),
                                        e.to_string(),
                                        Some(duration_ms),
                                    ))
                                    .await;

                                run_ctx
                                    .record_tool_call(ToolCallRecord {
                                        tool_name: call.name.clone(),
                                        arguments: call.arguments.clone(),
                                        result: format!("Error: {}", e),
                                        iteration,
                                        success: false,
                                    })
                                    .await;

                                // Add error message to session — LLM sees it on next turn
                                let error_content = format!("Tool '{}' error: {}", call.name, e);
                                if let Err(e) = run_ctx
                                    .add_message(Message::tool(error_content, call.id.clone()))
                                    .await
                                {
                                    return Err(crate::AgentError::from(e));
                                }

                                continue;
                            }
                        };

                        // Record tool call
                        run_ctx
                            .record_tool_call(ToolCallRecord {
                                tool_name: call.name.clone(),
                                arguments: call.arguments.clone(),
                                result: result.content.clone(),
                                iteration,
                                success: true,
                            })
                            .await;

                        // Emit ToolCallComplete
                        let duration_ms = tool_begin.elapsed().as_millis() as u64;
                        run_ctx
                            .emit(AgentStreamEvent::tool_call_complete(
                                call.id.clone(),
                                call.name.clone(),
                                result.content.clone(),
                                Some(duration_ms),
                            ))
                            .await;

                        // Add tool result to ctx
                        if let Err(e) = run_ctx
                            .add_message(Message::tool(result.content.clone(), call.id.clone()))
                            .await
                        {
                            return Err(crate::AgentError::from(e));
                        }

                        // Clear current tool calls for next iteration
                        run_ctx.clear_current_tool_calls().await;
                    }

                    // Emit IterationComplete
                    run_ctx
                        .emit(AgentStreamEvent::iteration_complete(
                            iteration,
                            tool_calls.clone(),
                            None,
                        ))
                        .await;

                    continue;
                }

                // No tool calls - we have final answer
                run_ctx
                    .emit(AgentStreamEvent::iteration_complete(
                        iteration,
                        Vec::new(),
                        Some(content.clone()),
                    ))
                    .await;

                // Save assistant response to session
                let mut final_msg = Message::assistant(content.clone());
                if !thinking.is_empty() {
                    final_msg = final_msg.with_thinking(thinking.clone());
                }
                if let Err(e) = run_ctx.add_message(final_msg).await {
                    return Err(crate::AgentError::from(e));
                }

                // Store final response data
                run_ctx.set_final_content(content.clone()).await;

                // === Emit AgentComplete with response data ===
                let response = run_ctx.finalize();
                let response_json = serde_json::json!({
                    "content": response.content,
                    "iterations": response.iterations,
                    "tool_calls": response.tool_calls.iter().map(|t| serde_json::json!({
                        "tool_name": t.tool_name,
                        "arguments": t.arguments,
                        "result": t.result,
                        "iteration": t.iteration,
                        "success": t.success,
                    })).collect::<Vec<_>>(),
                    "run_id": response.run_id,
                    "session_id": response.session_id,
                });
                run_ctx
                    .emit(AgentStreamEvent::agent_complete_with_response(
                        response_json,
                    ))
                    .await;

                return Ok(response);
            }
        });

        // Wait for agent loop to complete
        let agent_result = match agent_task.await {
            Ok(result) => result,
            Err(join_err) => {
                return Err(crate::AgentError::Context(format!(
                    "Agent task panicked: {}",
                    join_err
                )));
            }
        };

        if let Err(join_err) = interceptor_handle.await {
            tracing::warn!(%join_err, "Interceptor task panicked");
        }

        shutdown_event_tx.take();

        while let Some(result) = listener_set.join_next().await {
            if let Err(e) = result {
                tracing::warn!(%e, "Listener task panicked");
            }
        }

        // Disconnect MCP manager
        if let Some(ref mcp_manager) = self.config.mcp_manager {
            mcp_manager.disconnect().await.ok();
        }

        agent_result
    }
}

/// Returned when mutation is attempted while agent is running.
#[derive(Debug, thiserror::Error)]
#[error("agent {agent_id} is currently running — state mutation not allowed")]
pub struct AgentBusyError {
    pub agent_id: String,
}

/// Consume LLM stream response, emit streaming events, and accumulate into complete data.
///
/// Returns: (thinking, tool_calls, content, model, usage)
async fn consume_llm_stream(
    mut stream: StreamReceiver,
    run_ctx: &RunContext,
) -> Result<
    (
        String,
        Vec<vol_llm_core::ToolCall>,
        String,
        String,
        Option<vol_llm_core::TokenUsage>,
    ),
    crate::AgentError,
> {
    let mut thinking = String::new();
    let mut tool_calls = Vec::new();
    let mut content = String::new();
    let mut model = String::new();
    let mut last_usage: Option<vol_llm_core::TokenUsage> = None;

    let mut thinking_started = false;
    let mut content_started = false;

    while let Some(result) = stream.recv().await {
        let event = result.map_err(|e| crate::AgentError::Llm(e))?;

        match event.data {
            StreamEventData::ThinkingDelta { thinking: delta } => {
                if !thinking_started {
                    run_ctx.emit(AgentStreamEvent::thinking_start()).await;
                    thinking_started = true;
                }
                thinking.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::thinking_delta(delta)).await;
            }
            StreamEventData::ThinkingComplete { thinking: t } => {
                if !thinking_started {
                    run_ctx.emit(AgentStreamEvent::thinking_start()).await;
                    run_ctx
                        .emit(AgentStreamEvent::thinking_delta(t.clone()))
                        .await;
                }
                thinking = t;
                run_ctx
                    .emit(AgentStreamEvent::thinking_complete(thinking.clone()))
                    .await;
            }
            StreamEventData::ContentDelta { delta } => {
                if !content_started {
                    run_ctx.emit(AgentStreamEvent::content_start()).await;
                    content_started = true;
                }
                content.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::content_delta(delta)).await;
            }
            StreamEventData::ContentComplete { content: c } => {
                if !content_started {
                    run_ctx.emit(AgentStreamEvent::content_start()).await;
                    run_ctx
                        .emit(AgentStreamEvent::content_delta(c.clone()))
                        .await;
                }
                content = c;
                run_ctx
                    .emit(AgentStreamEvent::content_complete(content.clone()))
                    .await;
            }
            StreamEventData::ToolCallComplete { tool_call } => {
                tool_calls.push(tool_call);
            }
            StreamEventData::UsageUpdate { usage } => {
                last_usage = Some(usage);
            }
            StreamEventData::ResponseStart { model: m } => {
                model = m;
            }
            StreamEventData::ToolCallArgumentDelta {
                tool_call_id,
                tool_name,
                delta,
            } => {
                run_ctx
                    .emit(AgentStreamEvent::tool_call_argument_delta(
                        tool_call_id.clone(),
                        tool_name.clone(),
                        delta.clone(),
                    ))
                    .await;
            }
            StreamEventData::Error { code, message } => {
                tracing::warn!(%code, %message, "Stream error event received");
            }
            _ => {}
        }
    }

    Ok((thinking, tool_calls, content, model, last_usage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{
        ConversationResponse, FinishReason, Message as CoreMessage, StreamReceiver,
    };

    use crate::agent_def::AgentDef;
    
    use vol_llm_tool::ToolRegistry;
    use vol_session::InMemoryEntryStore;

    struct MockLlm;
    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> vol_llm_core::LLMProvider {
            vol_llm_core::LLMProvider::Anthropic
        }
        fn model(&self) -> &str {
            "mock"
        }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
            &[]
        }
        async fn converse(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<ConversationResponse> {
            Ok(ConversationResponse {
                message: CoreMessage::assistant("mock".to_string()),
                model: "mock".to_string(),
                usage: vol_llm_core::TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                raw: None,
            })
        }
        async fn converse_stream(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<StreamReceiver> {
            let (_tx, rx) = tokio::sync::mpsc::channel(10);
            Ok(StreamReceiver::new(rx))
        }
    }

    fn make_config() -> AgentConfig {
        AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_tools(Arc::new(ToolRegistry::new()))
            .with_session(Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))))
            .build()
            .expect("Test config build failed")
    }

    #[test]
    fn test_agent_config_default() {
        let config = make_config();
        assert!(config.def.is_none());
        assert_eq!(config.plugin_registry.plugins().len(), 0);
    }

    #[test]
    fn test_agent_config_with_def() {
        let def = AgentDef::new("test-agent", "You are a test agent.")
            .with_type("test-runner")
            .with_max_iterations(10)
            .with_max_history_messages(50);
        let config = AgentConfig::builder()
            .with_llm(Arc::new(MockLlm))
            .with_def(def)
            .build()
            .unwrap();
        assert_eq!(config.def.as_ref().unwrap().name, "test-agent");
        assert_eq!(config.def.as_ref().unwrap().max_iterations, Some(10));
        assert_eq!(config.def.as_ref().unwrap().max_history_messages, Some(50));
    }

    #[test]
    fn test_agent_config_fields() {
        let config = AgentConfig {
            agent_id: "test_agent".to_string(),
            working_dir: PathBuf::from("."),
            ..Default::default()
        };

        assert_eq!(config.agent_id, "test_agent");
        assert_eq!(config.working_dir, PathBuf::from("."));
    }
}
