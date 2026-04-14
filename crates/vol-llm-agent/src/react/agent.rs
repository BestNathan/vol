//! ReAct Agent implementation.

use super::{
    AgentResponse, AgentStreamEvent, PluginDecision, PluginRegistry, RunContext,
    plugin_context_from_run_ctx,
};
use crate::prompt_context::PromptContext;
use crate::react::state::ToolCallRecord;
use crate::session::Session;
use std::path::PathBuf;
use std::sync::Arc;
use vol_llm_core::{
    ConversationRequest, LLMClient, Message, SandboxRef, StreamEventData, StreamReceiver,
    ToolChoice,
};
use vol_llm_tool::{ToolContext, ToolSensitivity};

/// Agent configuration
#[derive(Clone)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub prompt_context: PromptContext,
    pub verbose: bool,
    pub plugin_registry: PluginRegistry,

    // Observability fields
    pub agent_id: String,
    pub log_base_path: PathBuf,
}

/// Generate a short random agent ID if not provided
fn generate_agent_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("agent_{:x}", timestamp % 0xFFFFFF)
}

impl Default for AgentConfig {
    fn default() -> Self {
        use crate::prompt_context::PromptTemplate;

        let template = PromptTemplate::new("default", "You are a helpful assistant.");
        let prompt_context = PromptContext::new(template);

        Self {
            max_iterations: 5,
            max_history_messages: 20,
            prompt_context,
            verbose: false,
            plugin_registry: PluginRegistry::new(),
            agent_id: generate_agent_id(),
            log_base_path: PathBuf::from("logs/agents"),
        }
    }
}

/// ReAct Agent
pub struct ReActAgent {
    llm: Arc<dyn LLMClient>,
    tools: Arc<vol_llm_tool::ToolRegistry>,
    config: AgentConfig,
    session: Arc<Session>,
    sandbox: Option<SandboxRef>,
}

impl ReActAgent {
    /// Create agent builder
    pub fn builder() -> super::AgentBuilder {
        super::AgentBuilder::new()
    }

    pub fn new(
        llm: Arc<dyn LLMClient>,
        tools: Arc<vol_llm_tool::ToolRegistry>,
        config: AgentConfig,
        session: Arc<Session>,
    ) -> Self {
        Self {
            llm,
            tools,
            config,
            session,
            sandbox: None,
        }
    }

    /// Set the sandbox for tool execution
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// Create agent with new session
    pub fn with_new_session(&self, session_id: String) -> Self {
        use crate::session::{InMemoryMessageStore, InMemorySessionStore};

        let new_session = Arc::new(Session::new(
            session_id,
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryMessageStore::new()),
        ));
        Self {
            session: new_session,
            llm: self.llm.clone(),
            tools: self.tools.clone(),
            config: self.config.clone(),
            sandbox: self.sandbox.clone(),
        }
    }

    /// Run ReAct loop and return the final response.
    ///
    /// All events are emitted via RunContext event bus.
    /// The returned AgentResponse contains the complete execution context including:
    /// - Final answer content and complete reasoning chain
    /// - run_id and session_id for correlation
    /// - All tool calls made during execution
    /// - Error information if any tool call failed
    pub async fn run(&self, user_input: &str) -> Result<AgentResponse, crate::AgentError> {
        // === Phase 1: Generate run_id and create RunContext ===
        let run_id = uuid::Uuid::new_v4().simple().to_string();

        let tools = self.tools.clone();
        let config = self.config.clone();
        let session = self.session.clone();

        let (run_ctx, plugin_rx, approval_rx) = RunContext::new(
            run_id.clone(),
            user_input.to_string(),
            self.session.id.clone(),
            session.clone(),
            tools,
            config.clone(),
        );

        // === Phase 1.5: Run log cleanup (best effort, non-blocking) ===
        let log_base_path = self.config.log_base_path.clone();
        let agent_id = self.config.agent_id.clone();
        tokio::spawn(async move {
            let agent_path = log_base_path.join(&agent_id);
            if let Err(e) = crate::observability::cleanup_old_logs(&agent_path).await {
                tracing::warn!(agent_id = %agent_id, error = %e, "Log cleanup failed");
            }
        });

        // === Phase 1.6: Spawn CLI approval handler for HITL ===
        super::run_cli_approval_loop(approval_rx);

        // === Phase 2: Initialize messages (call once before loop) ===
        run_ctx.init_messages().await?;

        // === Phase 2.5: Spawn SessionListener for session recording ===
        use crate::session::{FileMessageStore, SessionListener};

        let mut session_listener = SessionListener::new(
            run_ctx.event_tx.subscribe(),
            Arc::new(FileMessageStore::new(
                config.log_base_path.join(&config.agent_id),
                &session.id,
            )),
            session.id.clone(),
        );
        let session_listener_handle = tokio::spawn(async move {
            let _ = session_listener.run().await;
        });

        // === Phase 2.6: Spawn listener and interceptor tasks ===
        use super::plugin_stream::{run_interceptor_loop, spawn_listener_task};

        // Spawn listener task - subscribes to event broadcast channel
        // Handle stored for graceful shutdown wait
        // Note: We create plugin_ctx and subscribe here to avoid cloning RunContext
        // (which would clone senders and prevent channel close)
        let listener_event_rx = run_ctx.event_tx.subscribe();
        let plugin_ctx = plugin_context_from_run_ctx(&run_ctx);
        let listener_handle = spawn_listener_task(
            self.config.plugin_registry.plugins().to_vec(),
            plugin_ctx,
            listener_event_rx,
        );

        // Spawn interceptor loop task - receives from plugin_rx channel
        // When plugin_rx is closed (agent drops run_ctx), interceptor exits
        let interceptor_event_tx = run_ctx.event_tx.clone();
        let interceptor_plugins = self.config.plugin_registry.plugins().to_vec();
        let interceptor_plugin_ctx = plugin_context_from_run_ctx(&run_ctx);
        let interceptor_handle = tokio::spawn(async move {
            run_interceptor_loop(
                plugin_rx,
                interceptor_plugins,
                interceptor_event_tx,
                interceptor_plugin_ctx,
            )
            .await;
        });

        // === Phase 3: Spawn agent loop task and await it ===
        let llm = self.llm.clone();
        let tools = self.tools.clone();
        let config = self.config.clone();
        let _session_id = self.session.id.clone();
        let _session = self.session.clone();
        let user_input = user_input.to_string();
        let _run_id_clone = run_id.clone();
        let sandbox = self.sandbox.clone();

        let agent_task = tokio::spawn(async move {
            // === Emit and intercept AgentStart ===
            let start_event = AgentStreamEvent::agent_start(user_input.clone());
            run_ctx.emit(start_event.clone()).await;

            match run_ctx.intercept(&start_event).await {
                Ok(PluginDecision::Continue) => {
                    // Continue with normal flow
                }
                Ok(PluginDecision::Skip) => {
                    // Skip AgentStart event but continue with normal flow
                    // Skip only affects the current event, not the entire run
                }
                Ok(PluginDecision::Abort(reason)) => {
                    run_ctx
                        .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                        .await;
                    return Err(crate::AgentError::Context(reason));
                }
                Err(e) => {
                    // Plugin channel error - log and continue (plugins are optional)
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

                if iteration > config.max_iterations {
                    // Emit max iterations reached event
                    let reason = format!("Max iterations ({}) reached", config.max_iterations);
                    run_ctx
                        .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                        .await;

                    return Err(crate::AgentError::MaxIterationsReached {
                        max: config.max_iterations,
                    });
                }

                // Reason phase - call LLM with streaming
                let tools_defs = tools.definitions();

                // Get messages from ctx (not local variable)
                let messages = run_ctx.get_messages().await;

                let request = ConversationRequest::with_history(None, messages)
                    .with_tools(tools_defs)
                    .with_tool_choice(ToolChoice::Auto);

                // Emit LLMCallStart with full message history
                let messages = run_ctx.get_messages().await;
                run_ctx.emit(AgentStreamEvent::llm_call_start(iteration, messages)).await;

                let llm_stream = match llm.converse_stream(request).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        run_ctx.emit(AgentStreamEvent::llm_call_error(e.to_string())).await;
                        run_ctx.emit(AgentStreamEvent::agent_aborted(format!("LLM request failed: {}", e))).await;
                        return Err(crate::AgentError::Llm(e));
                    }
                };

                // Consume LLM stream — emits Thinking/Content streaming events internally
                let (thinking, tool_calls, content, model, usage) = match consume_llm_stream(llm_stream, &run_ctx).await {
                    Ok(data) => data,
                    Err(e) => {
                        run_ctx.emit(AgentStreamEvent::llm_call_error(e.to_string())).await;
                        run_ctx.emit(AgentStreamEvent::agent_aborted(format!("LLM stream failed: {}", e))).await;
                        return Err(e);
                    }
                };

                // Record reasoning step
                if !thinking.is_empty() {
                    run_ctx.record_reasoning_step(thinking.clone(), None).await;
                }

                // Emit LLMCallComplete with real model and usage
                run_ctx.emit(AgentStreamEvent::llm_call_complete(model.clone(), usage)).await;

                // Check if tool calls
                if !tool_calls.is_empty() {

                    // IMPORTANT: Add assistant message with tool calls to history
                    // This tells the LLM what tools it decided to call in the next iteration
                    let assistant_message = if !content.is_empty() {
                        Message::assistant_with_tools(content.clone(), tool_calls.clone())
                    } else {
                        // If no content, still need to record the tool call decision
                        Message::assistant_with_tools(
                            "Calling tools to get information.".to_string(),
                            tool_calls.clone(),
                        )
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
                                // Check tool sensitivity before execution
                                let args: serde_json::Value = serde_json::from_str(&call.arguments)
                                    .unwrap_or(serde_json::json!({}));
                                let sensitivity = tools.tool_sensitivity(&call.name, &args);

                                match sensitivity {
                                    ToolSensitivity::RequiresApproval { reason } => {
                                        let metadata = serde_json::json!({
                                            "tool_call_id": call.id,
                                            "arguments": call.arguments
                                        });
                                        match run_ctx.request_tool_approval(&call.name, &reason, metadata).await {
                                            Ok(approval) if !approval.approved => {
                                                let duration_ms = tool_begin.elapsed().as_millis() as u64;
                                                // Emit ToolCallSkipped
                                                run_ctx.emit(AgentStreamEvent::tool_call_skipped(
                                                    call.id.clone(),
                                                    call.name.clone(),
                                                    "User rejected".to_string(),
                                                    Some(duration_ms),
                                                )).await;

                                                // Add rejection message to history
                                                if let Err(e) = run_ctx.add_message(Message::tool(
                                                    "Execution rejected: permission denied".to_string(),
                                                    call.id.clone(),
                                                )).await {
                                                    return Err(crate::AgentError::from(e));
                                                }
                                                run_ctx.clear_current_tool_calls().await;
                                                continue;
                                            }
                                            Ok(_) => {
                                                // Approved, proceed
                                            }
                                            Err(e) => {
                                                tracing::warn!("HITL approval error: {}", e);
                                                // Fail open — proceed without approval
                                            }
                                        }
                                    }
                                    ToolSensitivity::Safe => {
                                        // Safe tool, proceed directly
                                    }
                                }
                            }
                            PluginDecision::Skip => {
                                tracing::warn!("Plugin intercepted to skip tool: {}", call.name);
                                let duration_ms = tool_begin.elapsed().as_millis() as u64;

                                run_ctx.emit(AgentStreamEvent::tool_call_skipped(
                                    call.id.clone(),
                                    call.name.clone(),
                                    "Plugin skipped".to_string(),
                                    Some(duration_ms),
                                )).await;

                                continue;
                            }
                            PluginDecision::Abort(reason) => {
                                run_ctx
                                    .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                                    .await;
                                return Err(crate::AgentError::Context(reason));
                            }
                        }

                        // Execute tool
                        let tool_ctx = match &sandbox {
                            Some(sandbox) => ToolContext::default().with_sandbox(sandbox.clone()),
                            None => ToolContext::default(),
                        };
                        let result = match tools.execute(call, &tool_ctx).await {
                            Ok(r) => r,
                            Err(e) => {
                                let duration_ms = tool_begin.elapsed().as_millis() as u64;
                                // Emit ToolCallError
                                run_ctx.emit(AgentStreamEvent::tool_call_error(
                                    call.id.clone(),
                                    call.name.clone(),
                                    e.to_string(),
                                    Some(duration_ms),
                                )).await;

                                // Record failed tool call
                                run_ctx.record_tool_call(ToolCallRecord {
                                    tool_name: call.name.clone(),
                                    arguments: call.arguments.clone(),
                                    result: format!("Error: {}", e),
                                    iteration,
                                    success: false,
                                }).await;

                                // Emit AgentAborted (terminal event)
                                let reason = format!("Tool execution failed: {}", e);
                                run_ctx.emit(AgentStreamEvent::agent_aborted(reason)).await;

                                // Set error in RunContext
                                run_ctx.set_error(format!("Tool execution failed: {}", e)).await;

                                return Err(crate::AgentError::ToolExecution {
                                    tool: call.name.clone(),
                                    error: e.to_string(),
                                });
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

                        // Add tool result to ctx (syncs to session automatically)
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

                    // Continue to next iteration
                    continue;
                }

                // No tool calls - we have final answer
                // Emit IterationComplete with final answer
                run_ctx
                    .emit(AgentStreamEvent::iteration_complete(
                        iteration,
                        Vec::new(),
                        Some(content.clone()),
                    ))
                    .await;

                // Save assistant response to session via ctx (user input already saved in init_messages)
                if let Err(e) = run_ctx
                    .add_message(Message::assistant(content.clone()))
                    .await
                {
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
                    .emit(AgentStreamEvent::agent_complete_with_response(response_json))
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

        // Wait for interceptor to finish with timeout
        // Interceptor exits when plugin_rx is closed (happens when agent_task drops run_ctx)
        let interceptor_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), interceptor_handle).await;

        // Wait for listener to finish with timeout
        // Listener exits when event_tx broadcast channel is closed (all senders dropped)
        let listener_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), listener_handle).await;

        // Handle interceptor result (log but don't fail - plugins are optional)
        match interceptor_result {
            Ok(Ok(())) => {
                if config.verbose {
                    tracing::info!("Interceptor task completed gracefully");
                }
            }
            Ok(Err(join_err)) => {
                tracing::warn!(%join_err, "Interceptor task panicked");
            }
            Err(_timeout) => {
                tracing::warn!(
                    "Interceptor task timeout after 5s - task may be hanging, proceeding anyway"
                );
            }
        }

        // Handle listener result (log but don't fail - plugins are optional)
        match listener_result {
            Ok(Ok(())) => {
                if config.verbose {
                    tracing::info!("Listener task completed gracefully");
                }
            }
            Ok(Err(join_err)) => {
                tracing::warn!(%join_err, "Listener task panicked");
            }
            Err(_timeout) => {
                tracing::warn!(
                    "Listener task timeout after 5s - task may be hanging, proceeding anyway"
                );
            }
        }

        // Wait for SessionListener to finish with timeout
        // SessionListener exits when event_tx broadcast channel is closed
        let session_listener_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), session_listener_handle).await;

        match session_listener_result {
            Ok(Ok(())) => {
                if config.verbose {
                    tracing::info!("SessionListener task completed gracefully");
                }
            }
            Ok(Err(join_err)) => {
                tracing::warn!(%join_err, "SessionListener task panicked");
            }
            Err(_timeout) => {
                tracing::warn!(
                    "SessionListener task timeout after 5s - task may be hanging, proceeding anyway"
                );
            }
        }

        agent_result
    }
}

/// Consume LLM stream response, emit streaming events, and accumulate into complete data.
///
/// Emits ThinkingStart/Delta/Complete and ContentStart/Delta/Complete events
/// as tokens arrive from the LLM.
///
/// Returns: (thinking, tool_calls, content, model, usage)
async fn consume_llm_stream(
    mut stream: StreamReceiver,
    run_ctx: &RunContext,
) -> Result<(String, Vec<vol_llm_core::ToolCall>, String, String, Option<vol_llm_core::TokenUsage>), crate::AgentError> {
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
            StreamEventData::ResponseComplete { .. } => {
                // Model info comes from ResponseStart
            }
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
                    run_ctx.emit(AgentStreamEvent::thinking_delta(t.clone())).await;
                }
                thinking = t;
                run_ctx.emit(AgentStreamEvent::thinking_complete(thinking.clone())).await;
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
                    run_ctx.emit(AgentStreamEvent::content_delta(c.clone())).await;
                }
                content = c;
                run_ctx.emit(AgentStreamEvent::content_complete(content.clone())).await;
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
            _ => {}
        }
    }

    Ok((thinking, tool_calls, content, model, last_usage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, 5);
        assert_eq!(config.max_history_messages, 20);
        assert_eq!(config.verbose, false);
        assert_eq!(config.plugin_registry.plugins().len(), 0);
    }

    #[test]
    fn test_agent_config_custom() {
        use crate::prompt_context::{PromptContext, PromptTemplate};

        let template = PromptTemplate::new("test", "test prompt");
        let prompt_context = PromptContext::new(template);

        let config = AgentConfig {
            max_iterations: 10,
            max_history_messages: 50,
            prompt_context,
            verbose: true,
            plugin_registry: PluginRegistry::new(),
            agent_id: "custom_agent".to_string(),
            log_base_path: PathBuf::from("custom/logs"),
        };
        assert_eq!(config.max_history_messages, 50);
        assert_eq!(config.agent_id, "custom_agent");
        assert_eq!(config.log_base_path, PathBuf::from("custom/logs"));
    }

    #[test]
    fn test_agent_config_with_observability() {
        use std::path::PathBuf;

        let config = AgentConfig {
            agent_id: "test_agent".to_string(),
            log_base_path: PathBuf::from("logs/agents"),
            ..Default::default()
        };

        assert_eq!(config.agent_id, "test_agent");
        assert_eq!(config.log_base_path, PathBuf::from("logs/agents"));
    }
}
